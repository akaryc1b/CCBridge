//! Scoped, comment-preserving Codex configuration with recovery backup.
use anyhow::{Context, Result, bail};
use std::{
    fs,
    io::Write,
    path::{Path, PathBuf},
};
use toml_edit::{Array, DocumentMut, Item, Table, value};
fn merge(source: &str, exe: &Path) -> Result<String> {
    let mut d = source
        .parse::<DocumentMut>()
        .context("Codex 配置不是有效 TOML，未修改")?;
    if d.get("mcp_servers").is_none() {
        d["mcp_servers"] = Item::Table(Table::new());
    }
    if !d["mcp_servers"].is_table() {
        bail!("mcp_servers 必须为 TOML 表，未修改")
    }
    if d["mcp_servers"].get("ccbridge").is_none() {
        d["mcp_servers"]["ccbridge"] = Item::Table(Table::new());
    }
    let t = d["mcp_servers"]["ccbridge"]
        .as_table_mut()
        .context("ccbridge 必须为 TOML 表，未修改")?;
    // Keep unrelated settings; local cwd is obsolete: remote workspace is selected in App.
    for k in [
        "cwd",
        "url",
        "http_headers",
        "env_http_headers",
        "bearer_token_env_var",
    ] {
        t.remove(k);
    }
    t["command"] = value(exe.to_str().context("程序路径不是 UTF-8")?);
    let mut a = Array::new();
    a.push("mcp");
    t["args"] = value(a);
    t["tool_timeout_sec"] = value(960);
    t["enabled"] = value(true);
    Ok(d.to_string())
}
pub fn executable() -> PathBuf {
    #[cfg(target_os = "macos")]
    {
        PathBuf::from("/Applications/CCBridge.app/Contents/MacOS/ccbridge")
    }
    #[cfg(not(target_os = "macos"))]
    {
        std::env::current_exe().unwrap_or_default()
    }
}
pub fn snippet(exe: &Path) -> String {
    format!(
        "[mcp_servers.ccbridge]\ncommand = {}\nargs = [\"mcp\"]\ntool_timeout_sec = 960",
        serde_json::to_string(&exe.to_string_lossy()).unwrap()
    )
}
pub fn configure() -> Result<String> {
    let exe = executable();
    let home = std::env::var_os("CODEX_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            PathBuf::from(std::env::var_os("HOME").unwrap_or_default()).join(".codex")
        });
    configure_at(&home.join("config.toml"), &exe)
}
fn configure_at(path: &Path, exe: &Path) -> Result<String> {
    if !exe.is_file() {
        bail!("请先把 CCBridge.app 安装到 /Applications，再配置 MCP；未修改 Codex 配置")
    }
    if path.is_symlink() {
        bail!("config.toml 是符号链接，请手动配置其实际目标")
    }
    let old = match fs::read_to_string(path) {
        Ok(s) => s,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => String::new(),
        Err(e) => return Err(e.into()),
    };
    let new = merge(&old, exe)?;
    if old == new {
        return Ok("Codex 已配置，无需修改；重新加载 MCP 或新建会话即可。".into());
    }
    let dir = path.parent().context("配置路径无父目录")?;
    fs::create_dir_all(dir)?;
    let id = uuid::Uuid::new_v4();
    let backup = dir.join(format!("config.toml.ccbridge-{id}.bak"));
    if path.exists() {
        fs::copy(path, &backup)?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&backup, fs::Permissions::from_mode(0o600))?;
        }
    }
    let tmp = dir.join(format!(".ccbridge-{id}.tmp"));
    let result = (|| -> Result<()> {
        let mut out = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&tmp)?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            out.set_permissions(fs::Permissions::from_mode(0o600))?;
        }
        out.write_all(new.as_bytes())?;
        out.sync_all()?;
        let current = fs::read_to_string(path).unwrap_or_default();
        if current != old {
            bail!("配置被其他程序修改，已取消写入，请重试")
        }
        fs::rename(&tmp, path)?;
        Ok(())
    })();
    if result.is_err() {
        let _ = fs::remove_file(tmp);
    }
    result?;
    Ok(format!(
        "已配置 {}；恢复文件：{}。重新加载 MCP 或新建 Codex 会话生效；SSH 保持连接。",
        path.display(),
        if old.is_empty() {
            "原文件不存在".into()
        } else {
            backup.display().to_string()
        }
    ))
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn scoped_merge() {
        let s = "# keep\nmodel = 'example'\n[mcp_servers.other]\ncommand = 'other'\n[mcp_servers.ccbridge]\nurl = 'http://old'\ncwd = '/tmp'\n";
        let n = merge(s, Path::new("/test/bin")).unwrap();
        assert!(n.contains("# keep"));
        assert!(n.contains("command = 'other'"));
        assert!(!n.contains("cwd"));
        assert!(
            !snippet(Path::new(
                "/Applications/CCBridge.app/Contents/MacOS/ccbridge"
            ))
            .contains("cwd")
        );
        assert!(!n.contains("http://old"));
        assert_eq!(merge(&n, Path::new("/test/bin")).unwrap(), n);
        assert!(merge("[broken", Path::new("/bin/x")).is_err());
    }
    #[test]
    fn backup_and_repeat() {
        let dir = std::env::temp_dir().join(uuid::Uuid::new_v4().to_string());
        fs::create_dir(&dir).unwrap();
        let p = dir.join("config.toml");
        fs::write(&p, "# preserved\nmodel='x'\n").unwrap();
        configure_at(&p, &std::env::current_exe().unwrap()).unwrap();
        assert_eq!(fs::read_dir(&dir).unwrap().count(), 2);
        assert!(
            configure_at(&p, &std::env::current_exe().unwrap())
                .unwrap()
                .contains("无需修改")
        );
        fs::remove_dir_all(dir).unwrap();
    }
}
