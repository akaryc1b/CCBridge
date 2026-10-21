//! Git credential protocol; secrets live in macOS Keychain, never in remote URLs/configs.
use crate::common::*;
use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::collections::BTreeMap;
#[derive(Serialize, Deserialize, Clone)]
pub struct Credential {
    pub username: String,
    pub secret: String,
}
impl Credential {
    pub fn validate(&self) -> Result<()> {
        if self.username.trim().is_empty()
            || self.secret.is_empty()
            || self.username.contains(['\n', '\r', '\0'])
            || self.secret.contains(['\n', '\r', '\0'])
            || self.username.len() > 256
            || self.secret.len() > 8192
        {
            bail!("请填写有效的 Codeup HTTPS 用户名和密码 / 访问令牌（不能包含换行）")
        };
        Ok(())
    }
}
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
pub const HELPER: &str = r#"!f() {
[ "$1" = get ] || exit 0
protocol=; host=
while IFS='=' read -r key value; do
 case "$key" in protocol) protocol=$value;; host) host=$value;; esac
done
[ "$protocol" = https ] && [ "$host" = "$CCB_GIT_HOST" ] || exit 0
printf 'username=%s\npassword=%s\n' "$CCB_GIT_USER" "$CCB_GIT_TOKEN"
}; f "$@""#;
pub fn command(args: &[&str], cred: Option<&Credential>, host: &str) -> std::process::Command {
    let mut c = std::process::Command::new("git");
    if let Some(secret) = cred {
        c.args([
            "-c",
            "credential.helper=",
            "-c",
            &format!("credential.helper={HELPER}"),
        ])
        .env("CCB_GIT_HOST", host)
        .env("CCB_GIT_USER", &secret.username)
        .env("CCB_GIT_TOKEN", &secret.secret)
        .env("GIT_ASKPASS", "/usr/bin/false")
        .env("GCM_INTERACTIVE", "Never");
    }
    c.args(args).env("GIT_TERMINAL_PROMPT", "0");
    c
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
#[cfg(test)]
mod tests {
    use super::*;
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
