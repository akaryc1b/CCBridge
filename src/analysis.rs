//! Read/search-only Claude launches. Authentication remains owned by Claude.
use crate::common::*;
use anyhow::{Context, Result};
use serde_json::{Value, json};
use std::{
    fs::File,
    io::Read,
    path::Path,
    process::{Command, Stdio},
    time::{Duration, Instant},
};
pub fn settings() -> Value {
    json!({"disableAllHooks":true,"autoMemoryEnabled":false,
        "permissions":{"blockReadsOutsideWorkingDirectories":true}})
}
pub fn arguments() -> Vec<String> {
    [
        "--tools",
        "Read,Glob,Grep",
        "--safe-mode",
        "--setting-sources",
        "user",
        "--settings",
        &settings().to_string(),
        "--disable-slash-commands",
        "--disallowedTools",
        "mcp__*",
        "--permission-mode",
        "dontAsk",
        "--permission-prompts",
        "none",
    ]
    .iter()
    .map(|s| s.to_string())
    .collect()
}
fn unsupported(message: &str) -> anyhow::Error {
    coded(
        "analysis_unsupported",
        format!("无法启用只读分析：{message}"),
    )
}
fn managed_file(path: &Path) -> Result<()> {
    if !path.exists() {
        return Ok(());
    }
    let f = File::open(path).context("无法检查受管设置")?;
    let mut s = String::new();
    f.take(2 * 1024 * 1024 + 1).read_to_string(&mut s)?;
    if s.len() > 2 * 1024 * 1024 {
        return Err(unsupported("受管设置过大，未启动任务"));
    }
    let v: Value =
        serde_json::from_str(&s).map_err(|_| unsupported("受管设置无法解析，未启动任务"))?;
    fn has_content(v: &Value) -> bool {
        match v {
            Value::Null => false,
            Value::String(s) => !s.trim().is_empty(),
            Value::Object(o) => o.values().any(has_content),
            Value::Array(a) => a.iter().any(has_content),
            _ => true,
        }
    }
    fn has_script(v: &Value) -> bool {
        match v {
            Value::Object(o) => o.iter().any(|(key, value)| {
                (["hooks", "policyHelper"].contains(&key.as_str()) && has_content(value))
                    || has_script(value)
            }),
            Value::Array(a) => a.iter().any(has_script),
            _ => false,
        }
    }
    if (!v.is_object() && !v.is_null()) || has_script(&v) {
        return Err(unsupported(
            "存在受管启动脚本，不能保证仅分析；请联系管理员",
        ));
    }
    Ok(())
}
fn supported_version(s: &str) -> bool {
    let Some(v) = s
        .split_whitespace()
        .find(|v| v.chars().next().is_some_and(|c| c.is_ascii_digit()))
    else {
        return false;
    };
    let parts: Vec<_> = v.split('.').collect();
    parts.len() == 3
        && parts[0] == "2"
        && parts[1] == "1"
        && parts[2].parse::<u32>().is_ok_and(|p| p >= 308)
}
fn cli_text(claude: &str, flag: &str) -> Result<String> {
    let path = runner_dir()?.join(format!("analysis-probe-{}", uuid::Uuid::new_v4()));
    let output = File::create(&path)?;
    let mut child = Command::new(claude)
        .arg(flag)
        .current_dir(home()?)
        .stdin(Stdio::null())
        .stdout(output)
        .stderr(Stdio::null())
        .spawn()
        .context("无法检查 Claude 分析参数")?;
    let deadline = Instant::now() + Duration::from_secs(10);
    let status = loop {
        if let Some(s) = child.try_wait()? {
            break s;
        }
        if Instant::now() >= deadline {
            let _ = child.kill();
            let _ = child.wait();
            let _ = std::fs::remove_file(&path);
            return Err(unsupported("Claude 参数检测超时"));
        }
        std::thread::sleep(Duration::from_millis(50));
    };
    let result = (|| {
        let mut s = String::new();
        File::open(&path)?
            .take(1024 * 1024 + 1)
            .read_to_string(&mut s)?;
        if !status.success() || s.len() > 1024 * 1024 {
            return Err(unsupported("Claude 参数检测失败"));
        }
        Ok(s)
    })();
    let _ = std::fs::remove_file(path);
    result
}
pub fn verify_runtime() -> Result<()> {
    let config = std::env::var_os("CLAUDE_CONFIG_DIR")
        .map(std::path::PathBuf::from)
        .unwrap_or(home()?.join(".claude"));
    // A cache is normal on managed accounts; reject script-bearing content, not its mere presence.
    managed_file(&config.join("remote-settings.json"))
}
pub fn preflight(claude: &str, workspace: &Path) -> Result<()> {
    // Excluding project settings must not silently switch a project-only provider to a different login.
    for name in ["settings.json", "settings.local.json"] {
        let p = workspace.join(".claude").join(name);
        if p.exists() {
            let v: Value =
                read_json(&p).map_err(|_| unsupported("项目设置无法核实，未启动分析"))?;
            if v.get("apiKeyHelper").is_some()
                || v["env"].as_object().is_some_and(|e| {
                    e.keys().any(|k| {
                        k.starts_with("ANTHROPIC_")
                            || k.starts_with("CLAUDE_CODE_USE_")
                            || k == "CLAUDE_CODE_OAUTH_TOKEN"
                    })
                })
            {
                return Err(unsupported(
                    "模型认证配置在项目级，无法安全沿用；请先使用用户级 Claude 登录设置",
                ));
            }
        }
    }
    if !supported_version(&cli_text(claude, "--version")?) {
        return Err(unsupported(
            "需要已审核的 Claude 2.1.308 及以上 2.1 系列版本",
        ));
    }
    let config = std::env::var_os("CLAUDE_CONFIG_DIR")
        .map(std::path::PathBuf::from)
        .unwrap_or(home()?.join(".claude"));
    // Recheck at init/completion too: remote-delivered settings can change after this preflight.
    managed_file(&config.join("remote-settings.json"))?;
    let system = if cfg!(target_os = "macos") {
        Path::new("/Library/Application Support/ClaudeCode")
    } else {
        Path::new("/etc/claude-code")
    };
    managed_file(&system.join("managed-settings.json"))?;
    let dropins = system.join("managed-settings.d");
    if dropins.exists() {
        for e in std::fs::read_dir(dropins)? {
            let p = e?.path();
            if p.extension().is_some_and(|e| e == "json") {
                managed_file(&p)?;
            }
        }
    }
    if cfg!(target_os = "macos")
        && Path::new("/Library/Managed Preferences/com.anthropic.claudecode.plist").exists()
    {
        return Err(unsupported("存在系统受管配置，请先核实启动脚本"));
    }
    // The official CLI reference says --help omits some supported flags.
    // Use the reviewed version contract plus the actual process/init checks, not help-text heuristics.
    Ok(())
}
pub fn verify_init(v: &Value) -> Result<()> {
    let tools = v["tools"]
        .as_array()
        .ok_or_else(|| unsupported("Claude 没有报告工具集合"))?;
    if tools.iter().any(|t| {
        t.as_str()
            .is_none_or(|s| !["Read", "Glob", "Grep", "EndConversation"].contains(&s))
    }) {
        return Err(unsupported("Claude 返回了未允许的工具，任务已停止"));
    }
    if v["permissionMode"] != "dontAsk" || !v["mcp_servers"].is_array() {
        return Err(unsupported("Claude 没有确认受控权限或 MCP 状态"));
    }
    if !v["mcp_servers"].as_array().unwrap().is_empty() {
        return Err(unsupported("Claude 仍连接了 MCP 服务，任务已停止"));
    }
    if let Some(plugins) = v.get("plugins") {
        let builtin = [
            "cc-plugin-agents-md",
            "cc-plugin-telemetry",
            "cc-plugin-plugin-authoring",
        ];
        if plugins.as_array().is_none_or(|a| {
            a.iter().any(|p| {
                let name = p["name"].as_str().unwrap_or("");
                !builtin.contains(&name)
                    || p["path"] != "builtin"
                    || p["source"] != format!("{name}@builtin")
            })
        }) {
            return Err(unsupported("Claude 报告了外部插件，任务已停止"));
        }
    }
    // CLI command names are descriptive metadata, not an AI tool grant. Safe mode and
    // --disable-slash-commands remain on; the tools whitelist above excludes Skill/Agent.
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn managed_cache_accepts_plain_settings_but_rejects_nested_scripts() {
        let path = std::env::temp_dir().join(format!("ccbridge-managed-{}", uuid::Uuid::new_v4()));
        std::fs::write(
            &path,
            r#"{"settings":{"permissions":{"defaultMode":"default"}}}"#,
        )
        .unwrap();
        assert!(managed_file(&path).is_ok());
        std::fs::write(&path, "null").unwrap();
        assert!(managed_file(&path).is_ok());
        std::fs::write(
            &path,
            r#"{"settings":{"hooks":{"SessionStart":[]},"policyHelper":null}}"#,
        )
        .unwrap();
        assert!(managed_file(&path).is_ok());
        std::fs::write(&path, r#"{"settings":{"hooks":{"SessionStart":[{"hooks":[{"type":"command","command":"echo managed"}]}]}}}"#).unwrap();
        assert!(managed_file(&path).is_err());
        std::fs::remove_file(path).unwrap();
    }
    #[test]
    fn builtin_plugin_metadata_is_not_an_extra_tool_grant() {
        let mut init = json!({"tools":["Read","Glob","Grep"],"permissionMode":"dontAsk","mcp_servers":[],"plugins":[{"name":"cc-plugin-telemetry","path":"builtin","source":"cc-plugin-telemetry@builtin"}],"slash_commands":["compact","help"]});
        assert!(verify_init(&init).is_ok());
        init["plugins"][0]["path"] = json!("/tmp/custom");
        assert!(verify_init(&init).is_err());
        init["plugins"] = json!([]);
        init["tools"] = json!(["Read", "Skill"]);
        assert!(verify_init(&init).is_err());
    }
    #[test]
    fn version_floor_is_explicit() {
        assert!(supported_version("2.1.308 (Claude Code)"));
        assert!(!supported_version("2.1.256"));
        assert!(!supported_version("2.2.0"));
    }
    #[test]
    fn rejects_write_tools_and_extra_servers() {
        assert!(
            verify_init(&json!({"tools":["Read","Grep","EndConversation"],"mcp_servers":[],"permissionMode":"dontAsk"}))
                .is_ok()
        );
        assert!(verify_init(&json!({"tools":["Read","Bash"]})).is_err());
        assert!(verify_init(&json!({"tools":["Read"],"mcp_servers":[{"name":"db"}]})).is_err());
        assert!(verify_init(&json!({})).is_err());
    }
    #[test]
    fn analysis_arguments_preserve_login_not_bare_or_bypass() {
        let a = arguments();
        assert!(
            !a.iter()
                .any(|s| s == "--bare" || s == "--dangerously-skip-permissions")
        );
        assert!(a.contains(&"Read,Glob,Grep".into()));
        assert!(a.contains(&"dontAsk".into()));
    }
}
