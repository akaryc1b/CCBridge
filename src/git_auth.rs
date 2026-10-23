//! Git credential protocol. Mac private-file credentials; remote persistence is a separate human action.
use crate::common::*;
pub use crate::git_protocol::{Credential, HELPER, command};
use anyhow::{Context, Result, bail};
use serde_json::{Value, json};
use std::collections::BTreeMap;
pub fn host(url: &str) -> Result<String> {
    crate::provision::plan("access", url)?;
    let u = url::Url::parse(url)?;
    let host = u.host_str().context("仓库地址缺少域名")?;
    if host != "codeup.aliyun.com" {
        bail!("当前认证配置仅用于 https://codeup.aliyun.com 的仓库")
    }
    if u.port().is_some() {
        bail!("Codeup 地址不支持自定义端口")
    };
    Ok(host.into())
}
pub fn remote_prefix(host: &str, cred: Option<&Credential>) -> (String, Vec<u8>) {
    let mut s = String::from(
        "set -eu\nexport GIT_TERMINAL_PROMPT=0 GIT_ASKPASS=/usr/bin/false GCM_INTERACTIVE=Never\n",
    );
    if let Some(c) = cred {
        s += "IFS= read -r CCB_GIT_USER\nIFS= read -r CCB_GIT_TOKEN\nexport CCB_GIT_USER CCB_GIT_TOKEN\n";
        s += &format!(
            "export CCB_GIT_HOST={}\ngit_auth() {{ git -c credential.helper= -c {} \"$@\"; }}\n",
            shell_quote(host),
            shell_quote(&format!("credential.helper={HELPER}"))
        );
        (s, format!("{}\n{}\n", c.username, c.secret).into_bytes())
    } else {
        s += "git_auth() { git \"$@\"; }\n";
        (s, vec![])
    }
}
fn key(scope: &str, p: &Profile) -> Result<String> {
    match scope {
        "local" => Ok("local:codeup.aliyun.com".into()),
        "remote" => Ok(format!("remote:{}:codeup.aliyun.com", p.account_key())),
        _ => bail!("请选择本机或当前 SSH 账号"),
    }
}
fn metadata() -> Result<BTreeMap<String, String>> {
    let f = app_dir()?.join("git-auth.json");
    if f.exists() {
        read_json(&f)
    } else {
        Ok(BTreeMap::new())
    }
}
fn storage(k: &str) -> Result<String> {
    let f = app_dir()?.join("git-auth-stores.json");
    let m: BTreeMap<String, String> = if f.exists() {
        read_json(&f)?
    } else {
        BTreeMap::new()
    };
    Ok(m.get(k).cloned().unwrap_or_else(|| "keychain".into()))
}
pub fn status(scope: &str, p: &Profile) -> Result<Value> {
    let user = metadata()?.get(&key(scope, p)?).cloned();
    Ok(
        json!({"scope":scope,"configured":user.is_some(),"username":user,"host":"codeup.aliyun.com","storage":storage(&key(scope,p)?)?}),
    )
}
/// Configuration inspection only; does not invoke credential helpers or contact Codeup.
pub fn check_configuration(scope: &str, p: &Profile) -> Result<Value> {
    let mut result = status(scope, p)?;
    if scope == "local" {
        use std::io::Read;
        let mut child = std::process::Command::new("git")
            .args([
                "config",
                "--get-urlmatch",
                "credential.helper",
                "https://codeup.aliyun.com",
            ])
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::null())
            .spawn()
            .context("无法检查本机 Git 配置")?;
        let mut bytes = Vec::new();
        let read = child
            .stdout
            .take()
            .unwrap()
            .take(16 * 1024 + 1)
            .read_to_end(&mut bytes);
        if read.is_err() || bytes.len() > 16 * 1024 {
            let _ = child.kill();
            let _ = child.wait();
            bail!("Git 凭据助手配置读取失败或过大");
        }
        result["helper_configured"] =
            json!(child.wait()?.success() && bytes.iter().any(|b| !b.is_ascii_whitespace()));
    }
    result["checked_at"] = json!(now());
    Ok(result)
}
#[cfg(target_os = "macos")]
pub async fn save(scope: &str, p: &Profile, c: Credential, store: &str) -> Result<Value> {
    c.validate()?;
    let key = key(scope, p)?;
    if store != "file" {
        bail!("仅支持本机保存")
    };
    let username = c.username.clone();
    crate::secret_store::set(
        store,
        "dev.ccbridge.git.codeup",
        &key,
        serde_json::to_string(&c)?,
    )
    .await?;
    let f = app_dir()?.join("git-auth-stores.json");
    let mut stores: BTreeMap<String, String> = if f.exists() {
        read_json(&f)?
    } else {
        BTreeMap::new()
    };
    stores.insert(key.clone(), store.into());
    write_json(&f, &stores)?;
    let mut m = metadata()?;
    m.insert(key, username);
    write_json(&app_dir()?.join("git-auth.json"), &m)?;
    status(scope, p)
}
#[cfg(target_os = "macos")]
pub async fn load(scope: &str, p: &Profile) -> Result<Option<Credential>> {
    let k = key(scope, p)?;
    if !metadata()?.contains_key(&k) {
        return Ok(None);
    }
    if storage(&k)? != "file" {
        bail!("旧版Codeup认证尚未迁移，请在App认证窗口点击迁移")
    }
    let encoded = crate::secret_store::get("file", "dev.ccbridge.git.codeup", &k)
        .await
        .context("无法读取Codeup认证，请检查保存位置")?;
    let c: Credential = serde_json::from_str(&encoded)?;
    c.validate()?;
    Ok(Some(c))
}
#[cfg(target_os = "macos")]
pub async fn migrate(scope: &str, p: &Profile) -> Result<Value> {
    let k = key(scope, p)?;
    if storage(&k)? == "file" {
        return status(scope, p);
    }
    let encoded = crate::secret_store::get("keychain", "dev.ccbridge.git.codeup", &k).await?;
    let c: Credential = serde_json::from_str(&encoded)?;
    save(scope, p, c, "file").await
}
#[cfg(not(target_os = "macos"))]
pub async fn migrate(_scope: &str, _p: &Profile) -> Result<Value> {
    bail!("请在Mac App迁移旧凭据")
}
#[cfg(target_os = "macos")]
pub async fn remove(scope: &str, p: &Profile) -> Result<Value> {
    let k = key(scope, p)?;
    crate::secret_store::forget("dev.ccbridge.git.codeup", &k).await?;
    let mut m = metadata()?;
    m.remove(&k);
    write_json(&app_dir()?.join("git-auth.json"), &m)?;
    status(scope, p)
}
#[cfg(not(target_os = "macos"))]
pub async fn load(_scope: &str, _p: &Profile) -> Result<Option<Credential>> {
    Ok(None)
}
#[cfg(not(target_os = "macos"))]
pub async fn save(_scope: &str, _p: &Profile, _c: Credential, _store: &str) -> Result<Value> {
    bail!("认证保存仅在 macOS App 内可用")
}
#[cfg(not(target_os = "macos"))]
pub async fn remove(_scope: &str, _p: &Profile) -> Result<Value> {
    bail!("认证保存仅在 macOS App 内可用")
}
/// Only used after a human explicitly asks to synchronize this credential to the current VPS.
pub async fn for_vps(
    scope: &str,
    p: &Profile,
    url: &str,
    draft: Option<Credential>,
) -> Result<Credential> {
    for_vps_from(scope, p, url, draft, "auto").await
}
pub async fn for_vps_from(
    scope: &str,
    p: &Profile,
    url: &str,
    draft: Option<Credential>,
    source: &str,
) -> Result<Credential> {
    if !["auto", "input", "app", "git_helper"].contains(&source) {
        bail!("请选择认证来源");
    }
    // Copying an explicit or App-saved host credential needs no repository path.
    if url != "https://codeup.aliyun.com/" {
        host(url)?;
    }
    if source == "input" || source == "auto" {
        if let Some(c) = draft {
            c.validate()?;
            return Ok(c);
        }
        if source == "input" {
            bail!("请填写要同步的 Codeup 用户名和令牌");
        }
    }
    if source == "app" || source == "auto" {
        if let Some(c) = load(scope, p).await? {
            return Ok(c);
        }
        if source == "app" {
            bail!("App 尚未保存这份认证，请选择本机 Git 凭据助手或输入令牌");
        }
    }
    if source != "git_helper" && scope != "local" {
        bail!("App 尚未保存服务器档认证，请输入令牌或选择本机 Git 凭据助手")
    }
    // Ask the configured Git helper, not a shell/keychain export. No returned secret reaches UI/logs.
    let url = url.to_string();
    tokio::task::spawn_blocking(move || -> Result<Credential> {
        use std::io::{Read, Write};
        use std::os::unix::process::CommandExt;
        use std::process::Stdio;
        use std::time::{Duration, Instant};
        let parsed = url::Url::parse(&url)?;
        let mut child = std::process::Command::new("git")
            .args(["credential", "fill"])
            .process_group(0)
            .env("GIT_TERMINAL_PROMPT", "0")
            .env("GIT_ASKPASS", "/usr/bin/false")
            .env("GCM_INTERACTIVE", "Never")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()?;
        child.stdin.take().unwrap().write_all(
            format!(
                "protocol=https\nhost=codeup.aliyun.com\npath={}\n\n",
                parsed.path().trim_start_matches('/')
            )
            .as_bytes(),
        )?;
        let output = child.stdout.take().unwrap();
        let reader = std::thread::spawn(move || {
            let mut bytes = Vec::new();
            output
                .take(16 * 1024 + 1)
                .read_to_end(&mut bytes)
                .map(|_| bytes)
        });
        let group = child.id() as i32;
        let deadline = Instant::now() + Duration::from_secs(30);
        let state = loop {
            match child.try_wait() {
                Ok(Some(status)) => break Ok(status),
                Ok(None) if Instant::now() < deadline => {
                    std::thread::sleep(Duration::from_millis(40))
                }
                Ok(None) => {
                    break Err(anyhow::anyhow!(
                        "读取本机 Git 认证超时，请处理系统授权后重试"
                    ));
                }
                Err(e) => break Err(e.into()),
            }
        };
        // Terminate only this helper process group, including inherited pipe holders.
        unsafe {
            libc::kill(-group, libc::SIGKILL);
        }
        let _ = child.wait();
        let bytes = reader
            .join()
            .map_err(|_| anyhow::anyhow!("Git 认证读取线程异常"))??;
        if !state?.success() || bytes.len() > 16 * 1024 {
            bail!("本机 Git 助手没有可用认证，请在 App 输入 Codeup 用户名与令牌")
        }
        let text = String::from_utf8(bytes).context("Git 助手返回的认证格式无效")?;
        let mut c = Credential {
            username: String::new(),
            secret: String::new(),
        };
        for line in text.lines() {
            if let Some(v) = line.strip_prefix("username=") {
                c.username = v.into()
            }
            if let Some(v) = line.strip_prefix("password=") {
                c.secret = v.into()
            }
        }
        c.validate()?;
        Ok(c)
    })
    .await?
}
#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test]
    async fn explicit_host_credential_sync_does_not_need_repository_url() {
        let c = for_vps(
            "local",
            &Profile::default(),
            "https://codeup.aliyun.com/",
            Some(Credential {
                username: "fixture".into(),
                secret: "fixture-token".into(),
            }),
        )
        .await
        .unwrap();
        assert_eq!(c.username, "fixture");
        assert!(
            for_vps(
                "local",
                &Profile::default(),
                "https://other.example/",
                Some(c)
            )
            .await
            .is_err()
        );
    }
    #[test]
    fn git_protocol_and_remote_input_keep_secrets_out_of_commands() {
        let fallback = command(&["--version"], None, "codeup.aliyun.com");
        assert!(
            !fallback
                .get_envs()
                .any(|(key, _)| key == "GIT_ASKPASS" || key == "GCM_INTERACTIVE")
        );
        use std::io::Write;
        let c = Credential {
            username: "fixture-user".into(),
            secret: "fixture-token-not-real".into(),
        };
        let mut command = command(&["credential", "fill"], Some(&c), "codeup.aliyun.com");
        let mut child = command
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .spawn()
            .unwrap();
        child
            .stdin
            .take()
            .unwrap()
            .write_all(b"protocol=https\nhost=codeup.aliyun.com\n\n")
            .unwrap();
        let out = child.wait_with_output().unwrap();
        assert!(out.status.success());
        assert!(
            String::from_utf8(out.stdout)
                .unwrap()
                .contains("password=fixture-token-not-real")
        );
        let (mut script, mut input) = remote_prefix("codeup.aliyun.com", Some(&c));
        assert!(!script.contains(&c.secret));
        script += "git_auth credential fill\n";
        input.extend_from_slice(b"protocol=https\nhost=codeup.aliyun.com\n\n");
        let mut child = std::process::Command::new("sh")
            .args(["-c", &script])
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .spawn()
            .unwrap();
        child.stdin.take().unwrap().write_all(&input).unwrap();
        let out = child.wait_with_output().unwrap();
        assert!(out.status.success());
        assert!(
            String::from_utf8(out.stdout)
                .unwrap()
                .contains("password=fixture-token-not-real")
        );
        let mut first = Profile::default();
        first.host = "server-a".into();
        first.user = "alice".into();
        let mut second = first.clone();
        second.user = "bob".into();
        assert_ne!(
            key("remote", &first).unwrap(),
            key("remote", &second).unwrap()
        );
        assert_eq!(
            key("local", &first).unwrap(),
            key("local", &second).unwrap()
        );
    }
    #[test]
    fn helper_is_host_scoped_and_not_command_interpolated() {
        let helper = HELPER.strip_prefix('!').unwrap();
        let run = |host: &str| {
            use std::io::Write;
            let mut p = std::process::Command::new("sh")
                .args(["-c", helper, "helper", "get"])
                .env("CCB_GIT_HOST", "codeup.aliyun.com")
                .env("CCB_GIT_USER", "u'$(false)")
                .env("CCB_GIT_TOKEN", "test-token-not-real")
                .stdin(std::process::Stdio::piped())
                .stdout(std::process::Stdio::piped())
                .spawn()
                .unwrap();
            writeln!(p.stdin.take().unwrap(), "protocol=https\nhost={host}\n").unwrap();
            p.wait_with_output().unwrap().stdout
        };
        assert!(
            String::from_utf8(run("codeup.aliyun.com"))
                .unwrap()
                .contains("password=test-token-not-real")
        );
        assert!(run("evil.example").is_empty());
        assert!(host("https://u:token@codeup.aliyun.com/o/r.git").is_err());
    }
}
