//! Fixed SSH filesystem operations. All inputs are quoted, canonical paths stay inside account HOME.
use crate::common::shell_quote;
use anyhow::{Result, bail};
use serde_json::{Value, json};
pub fn valid_path(p: &str) -> Result<()> {
    if !p.starts_with('/') || p.contains(['\0', '\n', '\r']) {
        bail!("远端路径必须为不含换行的绝对路径")
    }
    Ok(())
}
pub fn guard(home: &str, path: &str) -> Result<String> {
    valid_path(home)?;
    valid_path(path)?;
    Ok(format!(
        r#"# ccbridge:filesystem
set -eu
home={home}
path={path}
[ -d "$path" ] || {{ printf 'CCBRIDGE_NOT_DIRECTORY' >&2; exit 41; }}
path=$(cd "$path" && pwd -P)
case "$path/" in "$home/"*) ;; *) printf 'CCBRIDGE_OUTSIDE_HOME' >&2; exit 42;; esac
"#,
        home = shell_quote(home),
        path = shell_quote(path)
    ))
}
pub fn list_script(home: &str, path: &str, hidden: bool) -> Result<String> {
    let mut s = guard(home, path)?;
    s += r#"printf 'CCBRIDGE_FS_V1\000%s\000' "$path"
count=0
"#;
    let glob = if hidden {
        r#""$path"/* "$path"/.[!.]* "$path"/..?*"#
    } else {
        r#""$path"/*"#
    };
    s += &format!("for entry in {glob}; do\n");
    s += r#"[ -e "$entry" ] || [ -L "$entry" ] || continue
count=$((count+1))
if [ "$count" -gt 1000 ]; then printf 'truncated\000\000'; break; fi
name=${entry##*/}
if [ -L "$entry" ]; then kind=symlink
elif [ -d "$entry" ]; then
 if [ -e "$entry/.git" ]; then kind=repository; else kind=directory; fi
elif [ -f "$entry" ]; then kind=file
else kind=other; fi
printf '%s\000%s\000' "$kind" "$name"
done
"#;
    Ok(s)
}
pub fn parse_list(out: &[u8]) -> Result<Value> {
    let a: Vec<&[u8]> = out.split(|b| *b == 0).collect();
    if a.len() < 3 || a[0] != b"CCBRIDGE_FS_V1" {
        bail!("远端目录列表格式无效")
    }
    let path = std::str::from_utf8(a[1])?;
    let mut entries = Vec::new();
    let mut truncated = false;
    for pair in a[2..].chunks(2) {
        if pair.len() < 2 {
            break;
        }
        let kind = std::str::from_utf8(pair[0])?;
        let name = String::from_utf8_lossy(pair[1]);
        if kind == "truncated" {
            truncated = true;
            continue;
        }
        if name.is_empty() {
            continue;
        }
        entries.push(json!({"name":name,"path":format!("{path}/{name}"),"kind":kind}));
    }
    entries.sort_by_key(|v| {
        (
            !["directory", "repository"].contains(&v["kind"].as_str().unwrap_or("")),
            v["name"].as_str().unwrap_or("").to_lowercase(),
        )
    });
    Ok(json!({"path":path,"entries":entries,"truncated":truncated}))
}
pub fn mkdir_script(home: &str, parent: &str, name: &str) -> Result<String> {
    if name.is_empty() || [".", ".."].contains(&name) || name.contains(['/', '\0', '\n', '\r']) {
        bail!("文件夹名称不能为空，也不能包含斜杠或换行")
    }
    let mut s = guard(home, parent)?;
    s += &format!("umask 077\nmkdir -- \"$path\"/{}\n", shell_quote(name));
    Ok(s)
}
pub fn preview_allowed(path: &str) -> bool {
    !path.split('/').any(|s| s.starts_with('.') && !s.is_empty())
        && ![".pem", ".key"].iter().any(|s| path.ends_with(s))
        && !path.rsplit('/').next().unwrap_or("").starts_with("id_")
}
pub fn preview_script(home: &str, path: &str) -> Result<String> {
    valid_path(path)?;
    if !preview_allowed(path) {
        bail!("隐藏文件、密钥与凭据文件不提供内容预览")
    }
    let p = std::path::Path::new(path);
    let parent = p
        .parent()
        .and_then(|p| p.to_str())
        .ok_or_else(|| anyhow::anyhow!("文件路径无效"))?;
    let name = p
        .file_name()
        .and_then(|p| p.to_str())
        .ok_or_else(|| anyhow::anyhow!("文件名称无效"))?;
    let mut s = guard(home, parent)?;
    s += &format!(
        "file=\"$path\"/{}\n[ ! -L \"$file\" ] && [ -f \"$file\" ] || exit 43\nhead -c 131073 \"$file\"\n",
        shell_quote(name)
    );
    Ok(s)
}
pub fn failure(err: &str) -> &str {
    if err.contains("CCBRIDGE_OUTSIDE_HOME") {
        "此目录不在当前账号 HOME 内，未继续访问"
    } else if err.contains("CCBRIDGE_NOT_DIRECTORY") {
        "远端目录不存在或无法访问"
    } else if err.contains("Permission denied") {
        "当前 SSH 账号没有访问此目录的权限"
    } else if err.contains("File exists") {
        "同名文件或文件夹已存在，未覆盖"
    } else {
        "远端文件操作失败，请检查路径和当前账号权限"
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use std::{fs, process::Command};
    #[test]
    fn filesystem_contract() {
        let d = std::env::temp_dir().join(format!("cb-fs-{}", uuid::Uuid::new_v4()));
        fs::create_dir(&d).unwrap();
        let d = d.canonicalize().unwrap();
        let h = d.to_str().unwrap();
        fs::create_dir(d.join("项目 空格")).unwrap();
        fs::write(d.join("README.md"), "hello").unwrap();
        fs::write(d.join(".env"), "secret").unwrap();
        let run = |s: String| Command::new("sh").arg("-c").arg(s).output().unwrap();
        let out = run(list_script(h, h, false).unwrap());
        assert!(out.status.success());
        let v = parse_list(&out.stdout).unwrap();
        assert_eq!(v["entries"].as_array().unwrap().len(), 2);
        assert_eq!(v["entries"][0]["kind"], "directory");
        assert!(preview_script(h, &format!("{h}/.env")).is_err());
        assert_eq!(
            run(preview_script(h, &format!("{h}/README.md")).unwrap()).stdout,
            b"hello"
        );
        assert!(run(mkdir_script(h, h, "new").unwrap()).status.success());
        assert!(!run(mkdir_script(h, h, "new").unwrap()).status.success());
        #[cfg(unix)]
        {
            std::os::unix::fs::symlink("/", d.join("escape")).unwrap();
            assert!(
                !run(list_script(h, &format!("{h}/escape"), false).unwrap())
                    .status
                    .success()
            );
        }
        fs::remove_dir_all(d).unwrap();
    }
}
