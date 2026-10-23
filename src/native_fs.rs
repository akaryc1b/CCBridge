//! Runner filesystem RPC for platforms without a POSIX shell.
use crate::common::*;
use anyhow::{Context, Result, bail};
use serde_json::{Value, json};
use std::{
    io::Read,
    path::{Path, PathBuf},
};

fn inside(path: &str) -> Result<PathBuf> {
    let p = Path::new(path).canonicalize().context("目录或文件不存在")?;
    if !p.starts_with(home()?.canonicalize()?) {
        bail!("此目录不在当前账号 HOME 内");
    }
    Ok(p)
}
fn branch(path: &Path) -> Option<String> {
    if !path.join(".git").exists() {
        return None;
    }
    crate::repository::inspect(path).ok().map(|r| r.branch)
}
pub fn rpc(v: &Value) -> Result<Value> {
    match field(v, "op")? {
        "fs_list" => list(field(v, "path")?, v["hidden"] == true),
        "fs_mkdir" => {
            let p = inside(field(v, "path")?)?;
            let name = field(v, "name")?;
            crate::remote_platform::windows_name(name)?;
            std::fs::create_dir(p.join(name)).context("创建失败，可能已有同名项；未覆盖")?;
            list(p.to_str().context("目录编码无效")?, false)
        }
        "fs_preview" => {
            let raw = field(v, "path")?;
            let p = inside(raw)?;
            if std::fs::symlink_metadata(raw)?.file_type().is_symlink() {
                bail!("链接文件不提供预览");
            }
            if !preview_allowed(raw) {
                bail!("隐藏文件、密钥与凭据文件不提供内容预览");
            }
            let mut bytes = Vec::new();
            std::fs::File::open(&p)?
                .take(131073)
                .read_to_end(&mut bytes)?;
            if bytes.len() > 131072 {
                bail!("文件超过 128 KiB，内容预览未加载");
            }
            if bytes.contains(&0) {
                bail!("二进制文件不提供文本预览");
            }
            Ok(json!({"path":p,"text":String::from_utf8(bytes).context("文件不是 UTF-8 文本")?}))
        }
        _ => bail!("未知文件操作"),
    }
}
pub fn preview_allowed(path: &str) -> bool {
    let lower = path.to_lowercase();
    !lower
        .split(['/', '\\'])
        .any(|s| s.starts_with('.') || ["appdata", "ntuser.dat"].contains(&s))
        && ![".pem", ".key", ".pfx", ".p12"]
            .iter()
            .any(|s| lower.ends_with(s))
        && !lower
            .rsplit(['/', '\\'])
            .next()
            .unwrap_or("")
            .starts_with("id_")
}
fn list(path: &str, hidden: bool) -> Result<Value> {
    let p = inside(path)?;
    if !p.is_dir() {
        bail!("远端路径不是目录");
    }
    let mut entries = Vec::new();
    let mut truncated = false;
    for item in std::fs::read_dir(&p)? {
        let item = item?;
        let name = item.file_name().to_string_lossy().to_string();
        if !hidden && name.starts_with('.') {
            continue;
        }
        if entries.len() >= 1000 {
            truncated = true;
            break;
        }
        let meta = std::fs::symlink_metadata(item.path())?;
        #[cfg(windows)]
        let link = {
            use std::os::windows::fs::MetadataExt;
            meta.file_attributes() & 0x400 != 0
        };
        #[cfg(not(windows))]
        let link = meta.file_type().is_symlink();
        let b = if meta.is_dir() && !link {
            branch(&item.path())
        } else {
            None
        };
        let kind = if link {
            "symlink"
        } else if b.is_some() {
            "repository"
        } else if meta.is_dir() {
            "directory"
        } else if meta.is_file() {
            "file"
        } else {
            "other"
        };
        let modified = meta
            .modified()
            .ok()
            .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
            .map(|d| d.as_secs())
            .unwrap_or(0);
        entries.push(json!({"name":name,"path":item.path(),"kind":kind,"branch":b,"modified_at":modified,"size":meta.len()}));
    }
    entries.sort_by_key(|v| {
        (
            !["directory", "repository"].contains(&v["kind"].as_str().unwrap_or("")),
            v["name"].as_str().unwrap_or("").to_lowercase(),
        )
    });
    Ok(json!({"path":p,"entries":entries,"branch":branch(&p),"truncated":truncated}))
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn credentials_are_not_previews() {
        for p in [
            "C:\\Users\\u\\.ssh\\id_rsa",
            "C:\\Users\\u\\AppData\\Local\\CCBridge\\state\\databases.json",
            "C:\\Users\\u\\KEY.PEM",
            "/home/u/.env",
        ] {
            assert!(!preview_allowed(p));
        }
        assert!(preview_allowed("C:\\Users\\u\\项目\\README.md"));
    }
}
