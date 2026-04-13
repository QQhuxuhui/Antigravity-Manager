//! LLM pricing table (LiteLLM format) for converting token usage into USD cost.
//!
//! On startup we try to refresh `model_prices_and_context_window.json` from
//! LiteLLM's upstream repo; on failure we fall back to whatever is already
//! cached on disk. Costs are only ever computed on-demand when the UI asks
//! for per-account USD — no per-request writes.

use once_cell::sync::Lazy;
use parking_lot::RwLock;
use serde::Deserialize;
use std::collections::HashMap;
use std::path::PathBuf;

const UPSTREAM_URL: &str =
    "https://raw.githubusercontent.com/BerriAI/litellm/main/model_prices_and_context_window.json";

const CACHE_FILE: &str = "pricing.json";

/// Per-model pricing entry (subset of LiteLLM schema we care about).
#[derive(Debug, Clone, Default, Deserialize)]
pub struct ModelPricing {
    #[serde(default)]
    pub input_cost_per_token: f64,
    #[serde(default)]
    pub output_cost_per_token: f64,
    #[serde(default)]
    pub cache_read_input_token_cost: Option<f64>,
}

static PRICING: Lazy<RwLock<HashMap<String, ModelPricing>>> =
    Lazy::new(|| RwLock::new(HashMap::new()));

fn cache_path() -> Result<PathBuf, String> {
    let dir = crate::modules::account::get_data_dir()?;
    Ok(dir.join(CACHE_FILE))
}

/// Parse raw LiteLLM JSON; the first entry `sample_spec` is a schema sample
/// and has no numeric prices — any entry without a model name is ignored.
fn parse_json(raw: &str) -> Result<HashMap<String, ModelPricing>, String> {
    let value: serde_json::Value = serde_json::from_str(raw).map_err(|e| e.to_string())?;
    let obj = value
        .as_object()
        .ok_or_else(|| "pricing JSON root is not an object".to_string())?;

    let mut out = HashMap::with_capacity(obj.len());
    for (k, v) in obj {
        if k == "sample_spec" {
            continue;
        }
        if let Ok(entry) = serde_json::from_value::<ModelPricing>(v.clone()) {
            out.insert(k.to_lowercase(), entry);
        }
    }
    Ok(out)
}

/// Synchronously load the cached pricing file (if any) into memory. Call at
/// startup before the tokio runtime is available.
pub fn load_cache() {
    if let Ok(path) = cache_path() {
        if let Ok(raw) = std::fs::read_to_string(&path) {
            if let Ok(map) = parse_json(&raw) {
                let count = map.len();
                *PRICING.write() = map;
                crate::modules::logger::log_info(&format!(
                    "💰 Pricing loaded from cache: {} models",
                    count
                ));
            }
        }
    }
}

/// Async refresh from the LiteLLM upstream JSON. Meant to be spawned once the
/// tokio runtime is live (e.g. from tauri `setup`).
pub async fn refresh() {
    match refresh_from_upstream().await {
        Ok(count) => crate::modules::logger::log_info(&format!(
            "💰 Pricing refreshed from GitHub: {} models",
            count
        )),
        Err(e) => crate::modules::logger::log_warn(&format!(
            "💰 Pricing refresh failed, using cache: {}",
            e
        )),
    }
}

/// Fetch the LiteLLM pricing JSON from GitHub and replace in-memory table +
/// cached file.
async fn refresh_from_upstream() -> Result<usize, String> {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(20))
        .build()
        .map_err(|e| e.to_string())?;

    let resp = client
        .get(UPSTREAM_URL)
        .send()
        .await
        .map_err(|e| e.to_string())?;

    if !resp.status().is_success() {
        return Err(format!("HTTP {}", resp.status()));
    }

    let raw = resp.text().await.map_err(|e| e.to_string())?;
    let map = parse_json(&raw)?;
    let count = map.len();

    *PRICING.write() = map;

    if let Ok(path) = cache_path() {
        let _ = std::fs::write(&path, raw);
    }

    Ok(count)
}

/// Look up a model's pricing. Tries exact match, lowercased, and a few
/// common normalizations (strip `models/` prefix, strip vendor prefixes).
pub fn lookup(model: &str) -> Option<ModelPricing> {
    let table = PRICING.read();
    let lower = model.to_lowercase();

    if let Some(p) = table.get(&lower) {
        return Some(p.clone());
    }

    // Google often sends `models/gemini-...`; LiteLLM keys omit that prefix.
    if let Some(stripped) = lower.strip_prefix("models/") {
        if let Some(p) = table.get(stripped) {
            return Some(p.clone());
        }
    }

    // LiteLLM sometimes keys as `vertex_ai/<model>` or `gemini/<model>`.
    for prefix in ["gemini/", "vertex_ai/", "anthropic/"] {
        let key = format!("{}{}", prefix, lower);
        if let Some(p) = table.get(&key) {
            return Some(p.clone());
        }
    }

    None
}

/// Compute USD cost for a (model, input_tokens, output_tokens) tuple. Returns
/// 0.0 if the model is unknown so callers don't need to handle None.
pub fn cost_usd(model: &str, input_tokens: u64, output_tokens: u64) -> f64 {
    match lookup(model) {
        Some(p) => {
            (input_tokens as f64) * p.input_cost_per_token
                + (output_tokens as f64) * p.output_cost_per_token
        }
        None => 0.0,
    }
}
