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

/// Compute USD cost for a (model, input, output, cache_read) tuple. Returns
/// 0.0 if the model is unknown so callers don't need to handle None.
pub fn cost_usd(
    model: &str,
    input_tokens: u64,
    output_tokens: u64,
    cache_read_tokens: u64,
) -> f64 {
    match lookup(model) {
        Some(p) => compute_cost(&p, input_tokens, output_tokens, cache_read_tokens),
        None => 0.0,
    }
}

/// Pure cost computation — extracted so it can be unit-tested without seeding
/// the global pricing table. If the model has no explicit cache-read price,
/// fall back to the input price (conservative: no worse than pre-fix).
pub(crate) fn compute_cost(
    p: &ModelPricing,
    input_tokens: u64,
    output_tokens: u64,
    cache_read_tokens: u64,
) -> f64 {
    let cache_price = p
        .cache_read_input_token_cost
        .unwrap_or(p.input_cost_per_token);
    (input_tokens as f64) * p.input_cost_per_token
        + (output_tokens as f64) * p.output_cost_per_token
        + (cache_read_tokens as f64) * cache_price
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compute_cost_applies_cache_read_discount() {
        // Gemini 2.5 Pro-ish: $1.25/MTok input, $10/MTok output, cache read 25% of input.
        let p = ModelPricing {
            input_cost_per_token: 1.25e-6,
            output_cost_per_token: 10.0e-6,
            cache_read_input_token_cost: Some(0.3125e-6),
        };
        let cost = compute_cost(&p, 1_000, 400, 8_000);
        let expected = 1_000.0 * 1.25e-6 + 400.0 * 10.0e-6 + 8_000.0 * 0.3125e-6;
        assert!((cost - expected).abs() < 1e-12, "got {} expected {}", cost, expected);
    }

    #[test]
    fn compute_cost_falls_back_to_input_price_when_cache_price_missing() {
        // If a model in the pricing table lacks a cache-read price, we don't
        // silently drop the cached tokens — we bill them at the input price.
        // That's the conservative choice: no worse than pre-fix behavior.
        let p = ModelPricing {
            input_cost_per_token: 2.0e-6,
            output_cost_per_token: 8.0e-6,
            cache_read_input_token_cost: None,
        };
        let cost = compute_cost(&p, 100, 200, 500);
        let expected = 100.0 * 2.0e-6 + 200.0 * 8.0e-6 + 500.0 * 2.0e-6;
        assert!((cost - expected).abs() < 1e-12, "got {} expected {}", cost, expected);
    }

    #[test]
    fn compute_cost_no_cache_matches_old_behavior() {
        // When cache_read_tokens = 0, cost must equal the pre-fix formula.
        let p = ModelPricing {
            input_cost_per_token: 3.0e-6,
            output_cost_per_token: 15.0e-6,
            cache_read_input_token_cost: Some(0.3e-6),
        };
        let cost = compute_cost(&p, 1_000, 500, 0);
        let expected = 1_000.0 * 3.0e-6 + 500.0 * 15.0e-6;
        assert!((cost - expected).abs() < 1e-12);
    }
}
