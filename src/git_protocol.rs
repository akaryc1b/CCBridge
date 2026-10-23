//! Shared Git credential protocol, with secrets only in process environment.
use anyhow::{Result, bail};
use serde::{Deserialize, Serialize};
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
        .env(
            "GIT_ASKPASS",
            if cfg!(windows) {
                "false"
            } else {
                "/usr/bin/false"
            },
        )
        .env("GCM_INTERACTIVE", "Never");
    }
    c.args(args).env("GIT_TERMINAL_PROMPT", "0");
    c
}
