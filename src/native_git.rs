//! Native argv-only Git transport for Windows Runner; credentials arrive over SSH JSON stdin.
use crate::{common::*, git_protocol::Credential, platform::ProcessTree};
use anyhow::{Context, Result, bail};
use serde_json::{Value, json};
use std::{
    io::{Read, Write},
    path::{Path, PathBuf},
    process::{Command, Stdio},
    time::{Duration, Instant},
};
fn inside(raw: &str) -> Result<PathBuf> {
    let p = Path::new(raw).canonicalize().context("仓库目录不存在")?;
    if !p.starts_with(home()?.canonicalize()?) {
        bail!("Git 操作仅允许当前账号 HOME 内的目录");
    }
    Ok(p)
}
fn url_allowed(raw: &str) -> Result<String> {
    let u = url::Url::parse(raw).context("仓库地址必须是完整 HTTPS URL")?;
    if u.scheme() != "https"
        || u.host_str().is_none()
        || !u.username().is_empty()
        || u.password().is_some()
        || u.query().is_some()
        || u.fragment().is_some()
    {
        bail!("仓库地址必须是不含凭据的 HTTPS URL");
    }
    Ok(u.host_str().unwrap().into())
}
fn tail(mut input: impl Read, emit: bool, limit: usize) -> std::io::Result<Vec<u8>> {
    let mut out = Vec::new();
    let mut buf = [0; 8192];
    loop {
        let n = input.read(&mut buf)?;
        if n == 0 {
            break;
        }
        if emit {
            let _ = std::io::stderr().write_all(&buf[..n]);
        }
        let discard = (out.len() + n).saturating_sub(limit);
        out.drain(..discard.min(out.len()));
        out.extend_from_slice(&buf[..n]);
    }
    Ok(out)
}
fn run_prepared(mut cmd: Command) -> Result<Vec<u8>> {
    cmd.stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let mut child =
        ProcessTree::spawn(cmd).context("无法启动服务器 Git，请先安装 Git for Windows")?;
    let output = child.stdout().context("Git 输出管道不可用")?;
    let errors = child_stderr(&mut child)?;
    let out = std::thread::spawn(move || tail(output, false, 1024 * 1024));
    let err = std::thread::spawn(move || tail(errors, true, 65536));
    let deadline = Instant::now() + Duration::from_secs(600);
    let status = loop {
        if let Some(s) = child.try_wait()? {
            break Ok(s);
        }
        if Instant::now() >= deadline {
            break Err(anyhow::anyhow!("服务器 Git 操作超时，请检查网络与认证"));
        }
        std::thread::sleep(Duration::from_millis(50));
    };
    let _ = child.terminate(true);
    let _ = child.wait();
    let output = out
        .join()
        .map_err(|_| anyhow::anyhow!("Git 输出读取失败"))??;
    let error = err
        .join()
        .map_err(|_| anyhow::anyhow!("Git 错误读取失败"))??;
    if !status?.success() {
        return Err(failure(&String::from_utf8_lossy(&error)));
    }

    Ok(output)
}
fn child_stderr(child: &mut ProcessTree) -> Result<std::process::ChildStderr> {
    child.stderr().context("Git 错误管道不可用")
}
fn command(v: &Value, url: &str) -> Result<Command> {
    let host = url_allowed(url)?;
    let cred: Option<Credential> = v
        .get("credential")
        .filter(|v| !v.is_null())
        .map(|v| serde_json::from_value(v.clone()))
        .transpose()?;
    if let Some(c) = &cred {
        c.validate()?;
        if host != "codeup.aliyun.com" {
            bail!("Codeup 认证不能用于其他域名");
        }
    }
    let mut cmd = crate::git_protocol::command(&[], cred.as_ref(), &host);
    cmd.env("GIT_TERMINAL_PROMPT", "0")
        .env("GCM_INTERACTIVE", "Never")
        .env("LC_ALL", "C")
        .args([
            "-c",
            "credential.interactive=false",
            "-c",
            "core.fsmonitor=false",
        ]);
    Ok(cmd)
}
pub fn rpc(v: &Value) -> Result<Value> {
    match field(v, "op")? {
        "git_native_access" => {
            let mut cmd = command(v, field(v, "url")?)?;
            cmd.args(["ls-remote", "--heads", "--", field(v, "url")?]);
            let out = run_prepared(cmd)?;
            Ok(
                json!({"accessible":true,"branches":out.split(|b|*b==b'\n').filter(|l|!l.is_empty()).count()}),
            )
        }
        "git_native_url" => {
            let p = inside(field(v, "path")?)?;
            let remote = remote(v)?;
            let mut cmd = Command::new("git");
            cmd.arg("-C")
                .arg(p)
                .args(["remote", "get-url", "--", remote]);
            let url = String::from_utf8(run_prepared(cmd)?)?.trim().to_string();
            url_allowed(&url)?;
            Ok(json!({"url":url}))
        }
        "git_native_fetch" => {
            let p = inside(field(v, "path")?)?;
            crate::workspace_ops::protected_running(&p)?;
            let remote = remote(v)?;
            let target = rpc(&json!({"op":"git_native_url","path":p,"remote":remote}))?;
            let mut cmd = command(v, field(&target, "url")?)?;
            cmd.arg("-C")
                .arg(&p)
                .args(["fetch", "--progress", "--", remote]);
            run_prepared(cmd)?;
            crate::workspace_ops::rpc(&json!({"op":"branches","path":p}))
        }
        "git_native_clone" => {
            let p = inside(field(v, "path")?)?;
            let name = field(v, "name")?;
            crate::remote_platform::windows_name(name)?;
            let dest = p.join(name);
            if std::fs::symlink_metadata(&dest).is_ok() {
                bail!("同名目标已存在，未覆盖");
            }
            // Clone into an owned staging directory; failure never leaves a broken final repository.
            let stage = p.join(format!(".ccbridge-clone-{}", uuid::Uuid::new_v4()));
            let result = (|| -> Result<Value> {
                let mut cmd = command(v, field(v, "url")?)?;
                cmd.args(["clone", "--progress", "--", field(v, "url")?])
                    .arg(&stage);
                run_prepared(cmd)?;
                #[cfg(windows)]
                crate::platform::move_exclusive(&stage, &dest)?;
                #[cfg(unix)]
                {
                    if dest.exists() {
                        bail!("同名目标已存在，未覆盖");
                    }
                    std::fs::rename(&stage, &dest)?;
                }
                Ok(json!({"path":dest,"cloned":true}))
            })();
            if result.is_err() {
                let _ = std::fs::remove_dir_all(&stage);
            }
            result
        }
        _ => bail!("未知 Git 传输操作"),
    }
}
fn remote(v: &Value) -> Result<&str> {
    let r = field(v, "remote")?;
    if r.is_empty() || r.starts_with('-') || r.contains(['\0', '\n', '\r']) {
        bail!("远端名称无效");
    }
    Ok(r)
}

fn failure(error: &str) -> anyhow::Error {
    let text = error.to_ascii_lowercase();
    let (code, message) = if text.contains("certificate") || text.contains("ssl peer") {
        (
            "certificate",
            "服务器无法验证 Git 证书，请检查证书链；不要关闭证书验证",
        )
    } else if [
        "could not resolve",
        "failed to connect",
        "timed out",
        "connection refused",
    ]
    .iter()
    .any(|s| text.contains(s))
    {
        ("network", "服务器无法连接 Git 服务，请检查网络、代理和 DNS")
    } else if text.contains("not found") || text.contains("404") {
        (
            "repository",
            "仓库不存在或当前账号不可见，请核对仓库地址与读取权限",
        )
    } else if [
        "authentication",
        "username",
        "access denied",
        "403",
        "401",
        "terminal prompts disabled",
    ]
    .iter()
    .any(|s| text.contains(s))
    {
        (
            "auth",
            "服务器 Git 未通过认证或权限校验，请在 App 检测认证与仓库访问",
        )
    } else {
        (
            "git_failed",
            "服务器 Git 操作失败，请检查仓库状态、网络和磁盘空间；已有文件未覆盖",
        )
    };
    coded(code, message)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn address_and_network_failures_do_not_request_new_credentials() {
        for (raw, code) in [
            ("repository not found 404", "repository"),
            ("Failed to connect localhost", "network"),
            ("SSL certificate problem", "certificate"),
            ("Authentication failed 401", "auth"),
        ] {
            assert_eq!(error_code(&failure(raw)), code);
        }
    }
}
