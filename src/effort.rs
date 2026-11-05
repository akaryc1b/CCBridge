//! Per-turn effort; never changes Claude's global configuration.
use anyhow::{Result, bail};
use serde_json::Value;
pub fn selection(request: &Value, previous: Option<&Value>) -> Result<Option<String>> {
    let policy = request["effort_policy"]
        .as_str()
        .unwrap_or(if request["effort"].is_string() {
            "explicit"
        } else {
            "inherit"
        });
    let value = match policy {
        "remote_default" => None,
        "inherit" => previous
            .and_then(|p| p["effort"].as_str())
            .map(str::to_owned),
        "explicit" => Some(
            request["effort"]
                .as_str()
                .filter(|s| !s.is_empty())
                .ok_or_else(|| anyhow::anyhow!("请选择思考强度"))?
                .to_owned(),
        ),
        _ => bail!("思考强度设置无效"),
    };
    if value
        .as_deref()
        .is_some_and(|s| !["low", "medium", "high", "xhigh", "max"].contains(&s))
    {
        bail!("思考强度不受支持")
    }
    Ok(value)
}
pub fn validate_model(effort: &str, catalog: &Value, model: &str) -> Result<()> {
    let rows = catalog["models"]
        .as_array()
        .ok_or_else(|| anyhow::anyhow!("请刷新远端模型列表"))?;
    let key = if model.is_empty() { "default" } else { model };
    let row = rows
        .iter()
        .find(|r| r["value"] == key || r["resolved"] == key)
        .ok_or_else(|| anyhow::anyhow!("当前模型未返回思考强度信息"))?;
    if !row["effort_levels"]
        .as_array()
        .is_some_and(|levels| levels.iter().any(|v| v == effort))
    {
        bail!("当前模型不支持此思考强度，请重新选择或跟随远端默认")
    }
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    #[test]
    fn per_turn_override_and_inheritance() {
        let old = json!({"effort":"high"});
        assert_eq!(
            selection(&json!({}), Some(&old)).unwrap().as_deref(),
            Some("high")
        );
        assert_eq!(
            selection(&json!({"effort":"low"}), Some(&old))
                .unwrap()
                .as_deref(),
            Some("low")
        );
        assert!(
            selection(&json!({"effort_policy":"remote_default"}), Some(&old))
                .unwrap()
                .is_none()
        );
        assert!(selection(&json!({"effort":"--bad"}), None).is_err());
    }
    #[test]
    fn only_remote_supported_choices() {
        let c = json!({"models":[{"value":"opus","effort_levels":["low","high"]}]});
        assert!(validate_model("high", &c, "opus").is_ok());
        assert!(validate_model("max", &c, "opus").is_err());
        assert!(validate_model("high", &json!({"models":[{"value":"opus"}]}), "opus").is_err());
    }
}
