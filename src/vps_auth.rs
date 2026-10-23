//! User-authorized Codeup credential-store, isolated to this VPS account and HTTPS host.
use crate::common::*;
use anyhow::{Context, Result, bail};
use serde_json::{Value, json};
#[cfg(unix)]
use std::os::unix::fs::{MetadataExt, OpenOptionsExt, PermissionsExt};
use std::{
    fs,
    io::{Read, Write},
    path::{Path, PathBuf},
    process::{Command, Stdio},
};
#[cfg(unix)]
fn safe(p: &Path, private: bool) -> Result<()> {
    let m = fs::symlink_metadata(p)?;
    if m.file_type().is_symlink()
        || m.uid() != unsafe { libc::geteuid() }
        || (private && m.permissions().mode() & 0o077 != 0)
    {
        bail!("VPS 认证路径权限或归属异常：{}，已停止操作", p.display())
    }
    Ok(())
}
#[cfg(windows)]
fn safe(p: &Path, _private: bool) -> Result<()> {
    crate::platform::private_file(p)
}
fn paths(home: &Path, create: bool) -> Result<(PathBuf, PathBuf)> {
    let mut p = home.to_path_buf();
    for component in [".local", "share", "ccbridge", "codeup-auth"] {
        p.push(component);
        if !p.exists() && create {
            crate::platform::private_dir(&p)?;
        }
        if fs::symlink_metadata(&p).is_ok() {
            safe(&p, component == "codeup-auth")?;
        }
    }
    for name in ["credentials", "helper.gitconfig"] {
        let f = p.join(name);
        if fs::symlink_metadata(&f).is_ok() {
            safe(&f, true)?;
        }
    }
    let global = home.join(".gitconfig");
    if fs::symlink_metadata(&global).is_ok() {
        safe(&global, false)?;
    }
    Ok((p.join("credentials"), p.join("helper.gitconfig")))
}
fn git(home: &Path, args: &[&str], input: &[u8]) -> Result<std::process::Output> {
    let mut c = Command::new("git");
    c.args(args)
        .env("HOME", home)
        .env("GIT_CONFIG_GLOBAL", home.join(".gitconfig"))
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env("GIT_TERMINAL_PROMPT", "0")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null());
    let mut child = c.spawn()?;
    child.stdin.take().unwrap().write_all(input)?;
    Ok(child.wait_with_output()?)
}
fn private_write(p: &Path, bytes: &[u8]) -> Result<()> {
    let tmp = p.with_extension(format!("{}.tmp", uuid::Uuid::new_v4()));
    let result = (|| -> Result<()> {
        let mut options = fs::OpenOptions::new();
        options.create_new(true).write(true);
        #[cfg(unix)]
        options.mode(0o600);
        let mut f = options.open(&tmp)?;
        crate::platform::created_private(&tmp)?;
        f.write_all(bytes)?;
        f.sync_all()?;
        crate::platform::replace(&tmp, p)?;
        Ok(())
    })();
    if result.is_err() {
        let _ = fs::remove_file(tmp);
    }
    result
}
// Inspect only our managed store; never return its password or raw URL.
fn credential_username(path: &Path) -> Result<Option<String>> {
    if !path.exists() {
        return Ok(None);
    }
    let mut bytes = Vec::new();
    fs::File::open(path)?
        .take(64 * 1024 + 1)
        .read_to_end(&mut bytes)?;
    if bytes.len() > 64 * 1024 {
        bail!("VPS 认证文件过大，请重新同步");
    }
    for line in String::from_utf8_lossy(&bytes).lines() {
        if let Ok(url) = url::Url::parse(line) {
            if url.scheme() == "https"
                && url.host_str() == Some("codeup.aliyun.com")
                && !url.username().is_empty()
                && url.password().is_some_and(|p| !p.is_empty())
            {
                let encoded = format!("username={}", url.username().replace('+', "%2B"));
                return Ok(url::form_urlencoded::parse(encoded.as_bytes())
                    .next()
                    .map(|(_, v)| v.into_owned()));
            }
        }
    }
    Ok(None)
}
pub fn rpc(v: &Value) -> Result<Value> {
    rpc_at(v, &home()?.canonicalize()?)
}
pub fn rpc_at(v: &Value, home: &Path) -> Result<Value> {
    let (credentials, config) = paths(home, v["op"] == "git_vps_auth_install")?;
    let config_text = config.to_str().context("VPS HOME 路径无效")?;
    let includes = git(
        home,
        &["config", "--global", "--get-all", "include.path"],
        &[],
    )?;
    let installed = String::from_utf8(includes.stdout)?
        .lines()
        .any(|s| s == config_text);
    match field(v, "op")? {
        "git_vps_auth_install" => {
            if v["server_storage_ack"] != true {
                bail!("请在 App 明确确认将 Codeup 凭据保存到当前 VPS 账号")
            }
            let username = field(v, "username")?;
            let secret = field(v, "secret")?;
            if username.is_empty()
                || secret.is_empty()
                || username.len() > 256
                || secret.len() > 8192
                || username.contains(['\n', '\r', '\0'])
                || secret.contains(['\n', '\r', '\0'])
            {
                bail!("Codeup 用户名或令牌无效")
            }
            // Standard Git store, with credential protocol through stdin, never URL/argv/log output.
            let file = credentials.to_str().unwrap();
            let protocol = format!(
                "protocol=https\nhost=codeup.aliyun.com\nusername={username}\npassword={secret}\n\n"
            );
            let staged = credentials.with_extension(format!("{}.tmp", uuid::Uuid::new_v4()));
            let saved = (|| -> Result<()> {
                private_write(&staged, b"")?;
                let out = git(
                    home,
                    &[
                        "credential-store",
                        &format!("--file={}", staged.display()),
                        "store",
                    ],
                    protocol.as_bytes(),
                )?;
                if !out.status.success() {
                    bail!("VPS 凭据写入失败；原认证未覆盖")
                }
                crate::platform::created_private(&staged)?;
                crate::platform::replace(&staged, &credentials)?;
                Ok(())
            })();
            if saved.is_err() {
                let _ = fs::remove_file(&staged);
            }
            saved?;
            let value = serde_json::to_string(&format!("store --file={}", shell_quote(file)))?;
            let helper = format!(
                "[credential \"https://codeup.aliyun.com\"]\n\thelper =\n\thelper = {value}\n"
            );
            private_write(&config, helper.as_bytes())?;
            if !installed
                && !git(
                    home,
                    &["config", "--global", "--add", "include.path", config_text],
                    &[],
                )?
                .status
                .success()
            {
                bail!("凭据已写入私有文件，但 Git 配置接入失败，请重新同步或移除")
            }
        }
        "git_vps_auth_remove" => {
            if v["confirm"] != true {
                bail!("请在 App 确认移除 VPS 上的这份认证")
            }
            if installed
                && !git(
                    home,
                    &[
                        "config",
                        "--global",
                        "--fixed-value",
                        "--unset-all",
                        "include.path",
                        config_text,
                    ],
                    &[],
                )?
                .status
                .success()
            {
                bail!("无法解除 VPS Git 配置，未删除凭据")
            }
            for p in [&credentials, &config] {
                if p.exists() {
                    fs::remove_file(p)?;
                }
            }
        }
        "git_vps_auth_status" => {}
        _ => bail!("不支持的 VPS 认证操作"),
    }
    let includes = git(
        home,
        &["config", "--global", "--get-all", "include.path"],
        &[],
    )?;
    let included = String::from_utf8(includes.stdout)?
        .lines()
        .any(|s| s == config_text);
    let username = credential_username(&credentials)?;
    let configured = included && config.is_file() && username.is_some();
    Ok(json!({"configured":configured,"included":included,
        "entry_present":username.is_some(),"username":username,
        "files_present":credentials.is_file() && config.is_file(),
        "host":"codeup.aliyun.com","storage":"vps_private_file"}))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn git_store_is_account_local_host_scoped_and_removable() {
        let home = std::env::temp_dir().join(format!("ccb vps auth {}", uuid::Uuid::new_v4()));
        fs::create_dir(&home).unwrap();
        fs::write(home.join(".gitconfig"), "[alias]\n\tsafe = status\n").unwrap();
        let mut req = json!({"op":"git_vps_auth_install","username":"fixture-user","secret":"fixture-token-not-real"});
        assert!(rpc_at(&req, &home).is_err());
        req["server_storage_ack"] = json!(true);
        assert_eq!(rpc_at(&req, &home).unwrap()["configured"], true);
        let out = git(
            &home,
            &["credential", "fill"],
            b"protocol=https\nhost=codeup.aliyun.com\n\n",
        )
        .unwrap();
        assert!(out.status.success());
        let text = String::from_utf8(out.stdout).unwrap();
        assert!(text.contains("password=fixture-token-not-real"));
        let other = git(
            &home,
            &["credential", "fill"],
            b"protocol=https\nhost=other.example.test\n\n",
        )
        .unwrap();
        assert!(!String::from_utf8_lossy(&other.stdout).contains("fixture-token"));
        req["secret"] = json!("fixture-new-token");
        rpc_at(&req, &home).unwrap();
        let (cred, _) = paths(&home, true).unwrap();
        #[cfg(unix)]
        assert_eq!(
            fs::metadata(&cred).unwrap().permissions().mode() & 0o777,
            0o600
        );
        assert!(
            !fs::read_to_string(&cred)
                .unwrap()
                .contains("fixture-token-not-real")
        );
        assert!(rpc_at(&json!({"op":"git_vps_auth_remove"}), &home).is_err());
        assert_eq!(
            rpc_at(&json!({"op":"git_vps_auth_remove","confirm":true}), &home).unwrap()["configured"],
            false
        );
        assert!(
            fs::read_to_string(home.join(".gitconfig"))
                .unwrap()
                .contains("safe = status")
        );
        assert!(!cred.exists());
        fs::remove_dir_all(home).unwrap();
    }
    #[test]
    fn status_is_read_only_and_detects_erased_credentials() {
        let home = std::env::temp_dir().join(format!("ccb-status-{}", uuid::Uuid::new_v4()));
        fs::create_dir(&home).unwrap();
        let request = json!({"op":"git_vps_auth_status"});
        assert_eq!(rpc_at(&request, &home).unwrap()["configured"], false);
        assert!(!home.join(".local").exists());
        rpc_at(&json!({"op":"git_vps_auth_install","server_storage_ack":true,"username":"fixture-user","secret":"fixture-token"}),&home).unwrap();
        assert_eq!(rpc_at(&request, &home).unwrap()["username"], "fixture-user");
        let (credentials, _) = paths(&home, false).unwrap();
        fs::write(credentials, "").unwrap(); // Git erase after a rejected credential.
        let state = rpc_at(&request, &home).unwrap();
        assert_eq!(state["configured"], false);
        assert_eq!(state["entry_present"], false);
        assert_eq!(state["files_present"], true);
        assert!(!state.to_string().contains("fixture-token"));
        fs::remove_dir_all(home).unwrap();
    }
    #[test]
    fn rejects_symlink_credentials_and_line_injection() {
        let home = std::env::temp_dir().join(format!("ccb-auth-{}", uuid::Uuid::new_v4()));
        fs::create_dir(&home).unwrap();
        let (cred, _) = paths(&home, true).unwrap();
        #[cfg(unix)]
        std::os::unix::fs::symlink("/tmp", &cred).unwrap();
        assert!(rpc_at(&json!({"op":"git_vps_auth_status"}), &home).is_err());
        fs::remove_file(cred).unwrap();
        assert!(rpc_at(&json!({"op":"git_vps_auth_install","server_storage_ack":true,"username":"u\nhost=other","secret":"fake"}),&home).is_err());
        fs::remove_dir_all(home).unwrap();
    }
}
