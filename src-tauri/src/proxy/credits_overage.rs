// AI Credits Overages 工具模块
//
// 用于在账号开启 "AI Credits Overage" 时，向 v1internal 请求体注入
// `enabledCreditTypes: ["GOOGLE_ONE_AI"]` 字段。
//
// 该字段与 Antigravity 官方 IDE 在用户打开该开关时向 cloudcode-pa
// 上游发送的字段一致：Google 会先消耗免费配额，配额耗尽后自动消耗
// 账号（或家庭组）的 AI Credits。
//
// 注入是在上游调用之前完成的，不涉及 429 重试或 body 消费，因此
// 不会破坏现有的"模拟真实客户端"逻辑。

use serde_json::Value;

/// 在请求体 JSON 中追加 `enabledCreditTypes: ["GOOGLE_ONE_AI"]`。
///
/// 仅在 body 本身是 JSON Object 时生效；若调用方已显式设置该字段，
/// 本函数不会覆盖。
pub fn inject_enabled_credit_types(body: &mut Value) {
    if let Some(obj) = body.as_object_mut() {
        if !obj.contains_key("enabledCreditTypes") {
            obj.insert(
                "enabledCreditTypes".to_string(),
                Value::Array(vec![Value::String("GOOGLE_ONE_AI".to_string())]),
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn injects_enabled_credit_types() {
        let mut body = json!({"model":"gemini","contents":[]});
        inject_enabled_credit_types(&mut body);
        let arr = body
            .get("enabledCreditTypes")
            .and_then(|v| v.as_array())
            .expect("field should be inserted");
        assert_eq!(arr[0].as_str().unwrap(), "GOOGLE_ONE_AI");
    }

    #[test]
    fn inject_is_idempotent() {
        let mut body = json!({"enabledCreditTypes":["PAID"]});
        inject_enabled_credit_types(&mut body);
        let arr = body
            .get("enabledCreditTypes")
            .and_then(|v| v.as_array())
            .unwrap();
        assert_eq!(arr.len(), 1);
        assert_eq!(arr[0].as_str().unwrap(), "PAID");
    }

    #[test]
    fn inject_no_op_on_non_object() {
        let mut body = json!([1, 2, 3]);
        inject_enabled_credit_types(&mut body);
        assert!(body.get("enabledCreditTypes").is_none());
    }
}
