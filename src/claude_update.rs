//! Explicit, account-scoped Claude maintenance. Never installs a new CLI or elevates privileges.
use crate::common::*;
use anyhow::{Context, Result, bail};
use serde_json::{Value, json};
use std::{
    io::Read,
    path::Path,
    process::{Command, Stdio},
    time::{Duration, Instant},
};

pub fn version(text: &str) -> Result<String> {
    let v = text.trim().split_whitespace().next().unwrap_or("");
    let pieces = v.split('.').collect::<Vec<_>>();
    if pieces.len() != 3
        || pieces
            .iter()
            .any(|p| p.is_empty() || p.len() > 8 || !p.chars().all(|c| c.is_ascii_digit()))
    {
        bail!("Claude 未返回有效版本号")
    }
    Ok(v.to_owned())
}
fn newer(candidate: &str, current: &str) -> bool {
    let parts = |v: &str| {
        v.split('.')
            .map(|n| n.parse::<u64>().unwrap_or(0))
            .collect::<Vec<_>>()
    };
    parts(candidate) > parts(current)
}
pub fn install_kind(path: &Path, root: &Path) -> Result<(&'static str, bool)> {
    use std::os::unix::fs::MetadataExt;
    let actual = path.canonicalize().context("Claude 程序路径不存在")?;
    let root = root.canonicalize()?;
    if actual.to_string_lossy().contains("/Caskroom/")
        || actual.to_string_lossy().contains("/Cellar/")
    {
        return Ok(("homebrew", false));
    }
    let own = actual.starts_with(&root)
        && std::fs::metadata(&actual)?.uid() == unsafe { libc::geteuid() };
    if actual
        .to_string_lossy()
        .contains("/node_modules/@anthropic-ai/claude-code/")
    {
        return Ok(("npm", own));
    }
    if actual.starts_with(root.join(".local/share/claude/versions")) {
        return Ok(("native", own));
    }
    Ok((if own { "custom" } else { "shared" }, false))
}
// Private process group; bounded readers keep draining even after output is capped.
fn run(path: &str, args: &[&str], seconds: u64) -> Result<String> {
    use std::os::unix::process::CommandExt;
    let mut child = Command::new(path)
        .args(args)
        .current_dir(home()?)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .env("DISABLE_AUTOUPDATER", "1")
        .process_group(0)
        .spawn()
        .context("无法启动 Claude 版本维护命令")?;
    let group = child.id() as i32;
    fn reader(mut pipe: impl Read + Send + 'static) -> std::thread::JoinHandle<Vec<u8>> {
        std::thread::spawn(move || {
            let mut kept = Vec::new();
            let mut b = [0u8; 4096];
            while let Ok(n) = pipe.read(&mut b) {
                if n == 0 {
                    break;
                }
                let room = 65536usize.saturating_sub(kept.len());
                kept.extend_from_slice(&b[..n.min(room)]);
            }
            kept
        })
    }
    let out = reader(child.stdout.take().unwrap());
    let err = reader(child.stderr.take().unwrap());
    let end = Instant::now() + Duration::from_secs(seconds);
    let status = loop {
        if let Some(s) = child.try_wait()? {
            break Some(s);
        }
        if Instant::now() >= end {
            break None;
        }
        std::thread::sleep(Duration::from_millis(50));
    };
    unsafe {
        libc::kill(-group, libc::SIGKILL);
    }
    let _ = child.wait();
    let stdout = out.join().unwrap_or_default();
    let stderr = err.join().unwrap_or_default();
    let Some(status) = status else {
        bail!("Claude 维护命令超时，结果不确定；请重新检测版本，不要重复更新")
    };
    if !status.success() {
        let lower = String::from_utf8_lossy(&stderr).to_lowercase();
        if lower.contains("permission") || lower.contains("eacces") {
            bail!("当前账号没有更新权限，请由管理员维护此 Claude 安装")
        };
        bail!("Claude 维护命令失败，请检查安装方式和服务器网络后重新检测")
    }
    Ok(String::from_utf8_lossy(&stdout).into_owned())
}
fn installed(claude: &str) -> Result<String> {
    version(&run(claude, &["--version"], 10)?)
}
fn channel() -> Result<String> {
    // Read only the channel setting, never auth files or settings values in diagnostics.
    let root = std::env::var_os("CLAUDE_CONFIG_DIR")
        .map(std::path::PathBuf::from)
        .unwrap_or(home()?.join(".claude"));
    let files = [
        root.join("settings.json"),
        Path::new("/etc/claude-code/managed-settings.json").to_path_buf(),
        #[cfg(target_os = "macos")]
        Path::new("/Library/Application Support/ClaudeCode/managed-settings.json").to_path_buf(),
    ];
    let mut channel = "latest".to_string();
    for p in files {
        if !p.exists() {
            continue;
        }
        let data = std::fs::read(&p).context("无法读取 Claude 更新通道，请检查配置权限")?;
        if data.len() > 1024 * 1024 {
            bail!("Claude 设置文件过大，无法确认更新通道")
        };
        let v: Value =
            serde_json::from_slice(&data).context("Claude 设置格式无效，无法确认更新通道")?;
        if let Some(c) = v["autoUpdatesChannel"].as_str() {
            if !["latest", "stable"].contains(&c) {
                bail!("Claude 更新通道无法识别")
            };
            channel = c.into();
        }
    }
    Ok(channel)
}
fn available(channel: &str) -> Result<String> {
    let url = format!("https://downloads.claude.ai/claude-code-releases/{channel}");
    let response = run(
        "curl",
        &[
            "-q",
            "--fail",
            "--silent",
            "--show-error",
            "--connect-timeout",
            "5",
            "--max-time",
            "15",
            "--max-filesize",
            "128",
            "--proto",
            "=https",
            &url,
        ],
        20,
    )
    .context("无法检测 Claude 新版本，请检查服务器网络")?;
    version(&response).context("Claude 官方版本响应无法识别，请稍后重试")
}

pub fn check(claude: &str) -> Result<Value> {
    if !claude.starts_with('/') {
        bail!("请先检测 Claude 绝对路径")
    }
    let current = installed(claude)?;
    let (kind, can_update) = install_kind(Path::new(claude), &home()?)?;
    let channel = channel()?;
    let mut result = json!({"current_version":current,"install_kind":kind,"can_update":can_update,"channel":channel,"checked_at":now(),"path":claude});
    match available(&channel) {
        Ok(v) => {
            result["update_available"] = json!(newer(&v, &current));
            result["available_version"] = json!(v);
        }
        Err(e) => {
            result["check_error"] = json!(e.to_string());
        }
    }
    Ok(result)
}
pub fn update(v: &Value) -> Result<Value> {
    if v["origin"] != "app" || v["confirmed"] != true {
        bail!("Claude 更新只能由用户在 App 确认")
    }
    let claude = field(v, "claude")?;
    let expected = field(v, "expected_version")?;
    version(expected)?;
    if !claude.starts_with('/') {
        bail!("Claude 路径无效")
    }
    let current = installed(claude)?;
    if current != expected {
        bail!("Claude 版本已变化，请重新检测后再更新")
    }
    let (_, own) = install_kind(Path::new(claude), &home()?)?;
    if !own {
        bail!("此 Claude 安装不属于当前账号的独立安装，请由管理员或原包管理器更新")
    }
    // Atomic scheduler claim covers all project directories of this account, including analyses.
    let _guard = crate::scheduler::operation(&[home()?.canonicalize()?], "更新 Claude Code")?;
    {
        let registry = crate::scheduler::registry()?;
        if crate::scheduler::tasks()?.iter().any(|t| {
            ["queued", "running", "waiting_approval"].contains(&t.state.as_str())
                || t.ownership_unknown
        }) {
            bail!("当前账号仍有运行、排队或待审批任务，请结束后再更新 Claude Code")
        }
        drop(registry);
    }
    run(claude, &["update"], 180)?;
    let after = installed(claude)?;
    Ok(
        json!({"before_version":current,"current_version":after,"changed":after!=current,"checked_at":now(),"message":if after!=current{"Claude Code 已更新"}else{"Claude 版本未变化，请重新检查更新通道"}}),
    )
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn accepts_only_bounded_numeric_versions() {
        assert_eq!(version("2.1.308 (Claude Code)").unwrap(), "2.1.308");
        for s in ["", "2.1", "2.1.4;rm", "2.1.beta"] {
            assert!(version(s).is_err())
        }
        assert!(newer("2.1.317", "2.1.9"));
        assert!(!newer("2.1.9", "2.1.317"));
    }
    #[test]
    fn rejects_unconfirmed_updates_before_starting_commands() {
        assert!(
            update(&json!({"origin":"mcp","confirmed":true}))
                .unwrap_err()
                .to_string()
                .contains("用户")
        );
        assert!(update(&json!({"origin":"app","confirmed":false})).is_err());
    }
    #[test]
    fn recognizes_shared_and_private_native_paths() {
        let root =
            std::env::temp_dir().join(format!("ccbridge-claude-kind-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(root.join(".local/share/claude/versions")).unwrap();
        let path = root.join(".local/share/claude/versions/2.1.1");
        std::fs::write(&path, b"fixture").unwrap();
        assert_eq!(install_kind(&path, &root).unwrap(), ("native", true));
        let shared = root.join("other");
        std::fs::write(&shared, b"fixture").unwrap();
        assert_eq!(install_kind(&shared, &root).unwrap(), ("custom", false));
        std::fs::remove_dir_all(root).unwrap();
    }
}
