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
    if !v.is_object() || v.get("hooks").is_some() || v.get("policyHelper").is_some() {
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
    if config.join("remote-settings.json").exists() {
        return Err(unsupported(
            "发现组织受管设置，可能已执行管理员脚本；此账号暂不支持分析保护",
        ));
    }
    Ok(())
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
    // Remote-delivered policies can change after startup; don't certify their hooks from a stale cache.
    if config.join("remote-settings.json").exists() {
        return Err(unsupported("此账号使用动态受管设置，暂不支持只读分析"));
    }
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
    let help = cli_text(claude, "--help")?;
    if [
        "--tools",
        "--safe-mode",
        "--setting-sources",
        "--settings",
        "--strict-mcp-config",
        "--disable-slash-commands",
        "--permission-prompts",
        "--disallowedTools",
        "dontAsk",
    ]
    .iter()
    .any(|s| !help.contains(s))
    {
        return Err(unsupported(
            "Claude 版本缺少受控分析参数，请更新 Claude 后重试",
        ));
    }
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
    for name in ["mcp_servers", "plugins", "slash_commands"] {
        if v.get(name)
            .is_some_and(|a| a.as_array().is_none_or(|a| !a.is_empty()))
        {
            return Err(unsupported("Claude 加载了额外工具或自定义内容，任务已停止"));
        }
    }
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
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
