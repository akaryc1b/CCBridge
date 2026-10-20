use anyhow::{Result, bail};
use serde_json::{Value, json};
pub fn validate(model: &str) -> Result<()> {
    if model.starts_with('-') || model.chars().any(char::is_control) || model.len() > 256 {
        bail!("模型名称不能以 - 开头、包含控制字符或超过 256 字节")
    }
    Ok(())
}
pub fn selection(request: &Value, default: &str, previous: Option<&Value>) -> Result<Value> {
    let policy =
        request["model_policy"]
            .as_str()
            .unwrap_or(if request["model"].as_str().is_some() {
                "explicit"
            } else {
                "inherit"
            });
    let (model, source) = match policy {
        "explicit" => {
            let m = request["model"].as_str().unwrap_or("").trim();
            if m.is_empty() {
                bail!("指定模型不能为空")
            }
            (m.to_string(), "explicit")
        }
        "remote_default" => (String::new(), "remote_default"),
        "inherit" => match previous {
            Some(p) => (
                p["model_requested"]
                    .as_str()
                    .filter(|m| !m.is_empty())
                    .or(p["model"].as_str())
                    .unwrap_or("")
                    .into(),
                "inherit",
            ),
            None => (
                default.into(),
                if default.is_empty() {
                    "remote_default"
                } else {
                    "server_default"
                },
            ),
        },
        _ => bail!("model_policy 必须是 inherit、remote_default 或 explicit"),
    };
    validate(&model)?;
    Ok(json!({"model":model,"model_requested":model,"model_source":source,"model_policy":policy}))
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn inheritance_and_validation() {
        let old = json!({"model_requested":"sonnet"});
        assert_eq!(
            selection(&json!({}), "opus", Some(&old)).unwrap()["model"],
            "sonnet"
        );
        assert_eq!(
            selection(
                &json!({"model_policy":"remote_default"}),
                "opus",
                Some(&old)
            )
            .unwrap()["model"],
            ""
        );
        assert_eq!(
            selection(&json!({"model":"custom"}), "opus", None).unwrap()["model"],
            "custom"
        );
        assert!(validate("--other").is_err());
        assert!(validate("x\ny").is_err());
    }
}
