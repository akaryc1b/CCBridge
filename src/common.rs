use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::{
    io::Write,
    path::{Path, PathBuf},
};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};

pub fn home() -> Result<PathBuf> {
    Ok(PathBuf::from(
        std::env::var("HOME").context("无法读取当前用户 HOME")?,
    ))
}
pub fn app_dir() -> Result<PathBuf> {
    let p = if let Ok(p) = std::env::var("CCBRIDGE_HOME") {
        PathBuf::from(p)
    } else {
        home()?.join("Library/Application Support/CCBridge")
    };
    private_dir(&p)?;
    Ok(p)
}
pub fn runner_dir() -> Result<PathBuf> {
    let p = if let Ok(p) = std::env::var("CCBRIDGE_RUNNER_HOME") {
        PathBuf::from(p)
    } else {
        home()?.join(".local/state/ccbridge")
    };
    private_dir(&p)?;
    Ok(p)
}
pub fn private_dir(p: &Path) -> Result<()> {
    use std::os::unix::fs::PermissionsExt;
    std::fs::create_dir_all(p)?;
    std::fs::set_permissions(p, std::fs::Permissions::from_mode(0o700))?;
    Ok(())
}
pub fn write_json(p: &Path, value: &impl Serialize) -> Result<()> {
    let tmp = p.with_extension(format!("{}.tmp", uuid::Uuid::new_v4()));
    let mut f = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&tmp)?;
    f.write_all(&serde_json::to_vec_pretty(value)?)?;
    f.sync_all()?;
    std::fs::rename(tmp, p)?;
    Ok(())
}
pub fn read_json<T: serde::de::DeserializeOwned>(p: &Path) -> Result<T> {
    Ok(serde_json::from_slice(&std::fs::read(p)?)?)
}
pub fn valid_id(s: &str) -> Result<()> {
    uuid::Uuid::parse_str(s).context("任务、会话或审批编号无效")?;
    Ok(())
}
pub fn task_dir(id: &str) -> Result<PathBuf> {
    valid_id(id)?;
    Ok(runner_dir()?.join(id))
}
pub fn shell_quote(s: &str) -> String {
    format!("'{}'", s.replace('\'', "'\"'\"'"))
}
pub fn now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}
pub fn field<'a>(v: &'a Value, key: &str) -> Result<&'a str> {
    v[key]
        .as_str()
        .with_context(|| format!("缺少必填文本字段：{key}"))
}
/// Human-facing summaries; raw diagnostics remain available separately.
pub fn error_summary(raw: &str) -> String {
    let first = raw.lines().next().unwrap_or("");
    if first.starts_with("HOST_KEY:") {
        return raw.into();
    }
    let lower = raw.to_ascii_lowercase();
    for (needle, message) in [
        (
            "permission denied",
            "访问被拒绝，请检查 SSH 账号权限或 macOS 文件与文件夹访问权限",
        ),
        (
            "connection refused",
            "服务器拒绝连接，请检查 SSH 地址、端口及服务是否运行",
        ),
        (
            "no such file or directory",
            "文件或目录不存在，请检查配置路径和远端 Runner 是否已安装",
        ),
        (
            "connection reset",
            "连接被对端重置，请查询连接状态后重新连接",
        ),
        (
            "broken pipe",
            "通信连接已中断，请查询连接状态；不要重复派发原任务",
        ),
        (
            "timed out",
            "请求超时，请先查询连接与任务状态，不要重复创建任务",
        ),
        (
            "connection closed",
            "连接已关闭，请重新连接并查询原任务状态",
        ),
        (
            "invalid json",
            "返回数据不是有效 JSON，请检查 Runner 版本和远端 shell 输出",
        ),
    ] {
        if lower.contains(needle) {
            return message.into();
        }
    }
    if first
        .chars()
        .any(|c| ('\u{4e00}'..='\u{9fff}').contains(&c))
    {
        first.split(": ").next().unwrap_or(first).into()
    } else {
        "操作失败，请展开诊断详情查看原因。".into()
    }
}
pub fn envelope(r: Result<Value>) -> Value {
    match r {
        Ok(v) => json!({"ok":true,"data":v}),
        Err(e) => {
            let raw = format!("{e:#}");
            json!({"ok":false,"error":raw,"error_detail":{"code":"operation_failed","message_zh":error_summary(&raw),"diagnostic":raw,"retryable":false}})
        }
    }
}
pub fn unwrap(v: Value) -> Result<Value> {
    if v["ok"] == true {
        Ok(v["data"].clone())
    } else {
        bail!("{}", v["error"].as_str().unwrap_or("服务返回了无效响应"))
    }
}

#[derive(Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct RepositoryBinding {
    pub account: String,
    pub local_path: String,
    pub identity: String,
    pub remote_path: String,
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Profile {
    pub name: String,
    pub host: String,
    pub port: u16,
    pub user: String,
    pub fingerprint: String,
    pub runner: String,
    pub workspace: String,
    pub active_remote_workspace: String,
    pub claude: String,
    pub model: String,
    pub remember_password: bool,
    pub credential_store: String,
    pub allow_auto: bool,
    pub local_repository: String,
    pub search_root: String,
    pub bindings: Vec<RepositoryBinding>,
}
impl Default for Profile {
    fn default() -> Self {
        Self {
            name: String::new(),
            host: String::new(),
            port: 22,
            user: String::new(),
            fingerprint: String::new(),
            runner: String::new(),
            workspace: String::new(),
            active_remote_workspace: String::new(),
            claude: String::new(),
            model: String::new(),
            remember_password: false,
            credential_store: "keychain".into(),
            allow_auto: false,
            local_repository: String::new(),
            search_root: String::new(),
            bindings: vec![],
        }
    }
}
impl Profile {
    pub fn account_key(&self) -> String {
        serde_json::to_string(&(&self.host, self.port, &self.user)).unwrap()
    }
    pub fn migrate(&mut self) {
        for p in [&mut self.runner, &mut self.workspace, &mut self.claude] {
            if p.starts_with("/home/your-user/") {
                p.clear();
            }
        }
    }
    pub fn save(&self) -> Result<()> {
        let dir = app_dir()?;
        let path = dir.join("profiles.json");
        let mut profiles: std::collections::BTreeMap<String, Profile> = if path.exists() {
            read_json(&path)?
        } else {
            Default::default()
        };
        profiles.insert(self.account_key(), self.clone());
        write_json(&path, &profiles)?;
        write_json(&dir.join("profile.json"), self)
    }
    pub fn validate(&self) -> Result<()> {
        if !self.credential_store.is_empty() {
            crate::secret_store::validate_store(&self.credential_store)?;
        }
        if self.host.trim().is_empty() || self.user.trim().is_empty() || self.port == 0 {
            bail!("服务器、用户名和端口不能为空")
        }
        for (name, path) in [
            ("Runner", &self.runner),
            ("工作区", &self.workspace),
            ("远端选中工作区", &self.active_remote_workspace),
            ("Claude", &self.claude),
        ] {
            if !path.is_empty()
                && (!path.starts_with('/') || path.contains('\0') || path.contains('\n'))
            {
                bail!("{name} 必须是远端绝对路径")
            }
        }
        Ok(())
    }
}
pub async fn local_call(mut v: Value) -> Result<Value> {
    v["client_protocol"] = json!(1);
    let path = app_dir()?.join("bridge.sock");
    let mut stream = tokio::net::UnixStream::connect(path)
        .await
        .context("请先打开 CCBridge App 并连接 SSH")?;
    let mut bytes = serde_json::to_vec(&v)?;
    bytes.push(b'\n');
    stream.write_all(&bytes).await?;
    let mut line = String::new();
    tokio::time::timeout(
        std::time::Duration::from_secs(75),
        BufReader::new(stream)
            .take(8 * 1024 * 1024)
            .read_line(&mut line),
    )
    .await
    .context("App 请求超时；不要自动重试创建任务，请先查询任务列表")??;
    unwrap(serde_json::from_str(&line)?)
}
use tokio::io::AsyncReadExt;

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn chinese_error_summaries() {
        assert!(error_summary("Connection refused (os error 61)").contains("服务器拒绝连接"));
        assert!(error_summary("SSH 请求超时: deadline has elapsed").contains("SSH 请求超时"));
        assert_eq!(
            error_summary("unmapped library failure"),
            "操作失败，请展开诊断详情查看原因。"
        );
    }
    #[test]
    fn quote_and_ids() {
        assert_eq!(shell_quote("a'b"), "'a'\"'\"'b'");
        assert!(valid_id("../../bad").is_err());
    }
    #[test]
    fn profile_paths() {
        let mut p = Profile::default();
        p.host = "localhost".into();
        p.user = "test".into();
        assert!(p.validate().is_ok());
        p.runner = "~/bin/ccbridge".into();
        assert!(p.validate().is_err());
    }
}
