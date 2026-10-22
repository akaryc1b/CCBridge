//! Git uses its own approved credential helper locally; only Git objects cross SSH.
use anyhow::{Context, Result, bail};
use std::{
    io::Read,
    path::PathBuf,
    process::{Command, Stdio},
    time::{Duration, Instant},
};
pub struct Bundle {
    pub path: PathBuf,
    dir: PathBuf,
}
impl Drop for Bundle {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}
const STDERR_LIMIT: usize = 64 * 1024;

// Drain the whole pipe without retaining unbounded server/helper output.
fn read_tail(mut reader: impl Read) -> std::io::Result<Vec<u8>> {
    let mut tail = Vec::with_capacity(STDERR_LIMIT);
    let mut buffer = [0u8; 8192];
    loop {
        let n = match reader.read(&mut buffer) {
            Err(e) if e.kind() == std::io::ErrorKind::Interrupted => continue,
            result => result?,
        };
        if n == 0 {
            return Ok(tail);
        }
        let discard = (tail.len() + n).saturating_sub(STDERR_LIMIT);
        tail.drain(..discard);
        tail.extend_from_slice(&buffer[..n]);
    }
}

fn run_git(mut command: Command, duration: Duration) -> Result<()> {
    use std::os::unix::process::CommandExt;
    // A dedicated group lets a timeout reap only this request's Git/helper processes.
    command.process_group(0);
    let mut c = command
        .env("GIT_TERMINAL_PROMPT", "0")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .context("无法启动本机 Git")?;
    let group = c.id() as i32;
    let e = c.stderr.take().unwrap();
    let err = std::thread::spawn(move || read_tail(e));
    let deadline = Instant::now() + duration;
    let status = (|| -> Result<_> {
        loop {
            if let Some(s) = c.try_wait()? {
                return Ok(s);
            }
            if Instant::now() >= deadline {
                bail!("本机 Git 读取超时；请检查本机 Codeup 认证和网络")
            };
            std::thread::sleep(Duration::from_millis(40));
        }
    })();
    // Also close inherited pipes held by helpers after Git exits, before joining the reader.
    unsafe {
        libc::kill(-group, libc::SIGKILL);
    }
    let _ = c.wait();
    let errors = err
        .join()
        .map_err(|_| anyhow::anyhow!("Git 错误输出读取线程异常"))??;
    let status = status?;
    if !status.success() {
        bail!(
            "本机 Git 无法获取仓库：{}",
            local_failure(&String::from_utf8_lossy(&errors))
        )
    }
    Ok(())
}
fn git(args: &[&str], cred: Option<&crate::git_auth::Credential>, host: &str) -> Result<()> {
    run_git(
        crate::git_auth::command(args, cred, host),
        Duration::from_secs(300),
    )
}
pub fn local_failure(stderr: &str) -> &'static str {
    let e = stderr.to_ascii_lowercase();
    if e.contains("25293") || e.contains("keychain") || e.contains("interaction is not allowed") {
        "macOS 钥匙串没有放行 Git 凭据；请在系统弹窗授权，或在 App 的 Codeup 认证中重新保存后重试"
    } else if e.contains("could not read username") || e.contains("terminal prompts disabled") {
        "本机 Git 未取得 Codeup 认证，请在 App 的 Codeup 认证中输入保存，再检测仓库访问"
    } else if [
        "username",
        "authentication",
        "access denied",
        "403",
        "401",
        "repository not found",
    ]
    .iter()
    .any(|s| e.contains(s))
    {
        "本机 Git 未通过认证或仓库读取校验；请在 App 的 Codeup 认证中填写或更新凭据，再检测仓库访问；也请确认仓库地址和账号权限"
    } else if e.contains("resolve host") || e.contains("connect") || e.contains("timed out") {
        "本机无法连接 Git 服务，请检查本机网络、代理和 DNS 后重试"
    } else if e.contains("certificate") {
        "本机无法验证 Git 服务证书，请修复证书链，不要关闭证书验证"
    } else {
        "本机 Git 获取仓库失败，请检查 Git 安装、凭据助手、网络及磁盘空间；没有上传密码或覆盖已有仓库"
    }
}
pub fn bundle(url: &str) -> Result<Bundle> {
    bundle_with_auth(url, None)
}
pub fn access(url: &str, cred: Option<&crate::git_auth::Credential>) -> Result<()> {
    let host = crate::git_auth::host(url)?;
    git(&["ls-remote", "--", url], cred, &host)
}
pub fn bundle_with_auth(url: &str, cred: Option<&crate::git_auth::Credential>) -> Result<Bundle> {
    let host = url::Url::parse(url)
        .ok()
        .and_then(|u| u.host_str().map(String::from))
        .unwrap_or_default();
    let dir = crate::common::app_dir()?
        .join("transfers")
        .join(uuid::Uuid::new_v4().to_string());
    crate::common::private_dir(&dir)?;
    let b = Bundle {
        path: dir.join("repository.bundle"),
        dir: dir.clone(),
    };
    let mirror = dir.join("mirror.git");
    git(
        &[
            "clone",
            "--mirror",
            "--quiet",
            "--",
            url,
            mirror.to_str().context("本机临时路径无效")?,
        ],
        cred,
        &host,
    )?;
    git(
        &[
            "-C",
            mirror.to_str().unwrap(),
            "bundle",
            "create",
            b.path.to_str().unwrap(),
            "--all",
        ],
        cred,
        &host,
    )?;
    Ok(b)
}
pub fn import_script(home: &str, parent: &str, name: &str, url: &str) -> Result<String> {
    let mut s = crate::remote_fs::guard(home, parent)?;
    let id = uuid::Uuid::new_v4();
    use crate::common::shell_quote;
    s += &format!(
        r#"umask 077
bundle="$path/.ccbridge-{id}.bundle"
dest="$path"/{name}
if [ -e "$dest" ] || [ -L "$dest" ]; then printf 'CCBRIDGE_ALREADY_EXISTS' >&2; exit 43; fi
set -C
cat > "$bundle"
git clone --quiet -- "$bundle" "$dest"
git -C "$dest" remote set-url origin {url}
rm -- "$bundle"
"#,
        name = shell_quote(name),
        url = shell_quote(url)
    );
    Ok(s)
}
#[cfg(test)]
mod tests {
    use super::*;
    use std::process::Command;
    #[test]
    fn output_tail_is_bounded_and_keeps_last_bytes() {
        let mut input = vec![b'x'; 2 * 1024 * 1024];
        input.extend_from_slice(b"fatal: authentication failed");
        let tail = read_tail(std::io::Cursor::new(&input)).unwrap();
        assert_eq!(tail.len(), STDERR_LIMIT);
        assert_eq!(&tail, &input[input.len() - STDERR_LIMIT..]);
        assert_eq!(read_tail(std::io::Cursor::new(b"short")).unwrap(), b"short");
    }
    #[test]
    fn noisy_git_and_its_helpers_do_not_block_cleanup() {
        let mut noisy = Command::new("sh");
        noisy.args(["-c", "head -c 2097152 /dev/zero; head -c 2097152 /dev/zero >&2; printf 'authentication failed' >&2; exit 1"]);
        assert!(
            run_git(noisy, Duration::from_secs(5))
                .unwrap_err()
                .to_string()
                .contains("认证")
        );
        let started = Instant::now();
        let mut hung = Command::new("sh");
        hung.args(["-c", "sleep 30 & wait"]);
        assert!(
            run_git(hung, Duration::from_millis(100))
                .unwrap_err()
                .to_string()
                .contains("超时")
        );
        assert!(started.elapsed() < Duration::from_secs(3));
        // The helper retains stderr even though the direct child has exited.
        let mut orphan_pipe = Command::new("sh");
        orphan_pipe.args(["-c", "sleep 30 & exit 0"]);
        run_git(orphan_pipe, Duration::from_secs(2)).unwrap();
        assert!(started.elapsed() < Duration::from_secs(3));
    }
    #[test]
    fn local_errors_identify_the_local_machine() {
        assert!(local_failure("fatal: could not read Username").contains("本机"));
        assert!(local_failure("osxkeychain: -25293").contains("钥匙串"));
        assert!(local_failure("Could not resolve host").contains("本机"));
        assert!(!local_failure("authentication failed").contains("远端"));
    }
    #[test]
    fn bundle_contains_repository_not_credentials() {
        let d = std::env::temp_dir().join(uuid::Uuid::new_v4().to_string());
        std::fs::create_dir(&d).unwrap();
        Command::new("git")
            .args(["init", "-q"])
            .arg(&d)
            .status()
            .unwrap();
        std::fs::write(d.join("README.md"), "fixture").unwrap();
        Command::new("git")
            .arg("-C")
            .arg(&d)
            .args(["add", "README.md"])
            .status()
            .unwrap();
        assert!(
            Command::new("git")
                .arg("-C")
                .arg(&d)
                .args([
                    "-c",
                    "user.name=test",
                    "-c",
                    "user.email=test@example.com",
                    "commit",
                    "-qm",
                    "fixture"
                ])
                .status()
                .unwrap()
                .success()
        );
        let b = bundle(d.to_str().unwrap()).unwrap();
        let home = d.join("home");
        std::fs::create_dir(&home).unwrap();
        let home = home.canonicalize().unwrap();
        let script = import_script(
            home.to_str().unwrap(),
            home.to_str().unwrap(),
            "copy",
            "https://example.com/team/repo.git",
        )
        .unwrap();
        let input = std::fs::File::open(&b.path).unwrap();
        let o = Command::new("sh")
            .args(["-c", &script])
            .stdin(input)
            .output()
            .unwrap();
        assert!(o.status.success(), "{}", String::from_utf8_lossy(&o.stderr));
        assert_eq!(
            std::fs::read_to_string(home.join("copy/README.md")).unwrap(),
            "fixture"
        );
        let remote = Command::new("git")
            .arg("-C")
            .arg(home.join("copy"))
            .args(["remote", "get-url", "origin"])
            .output()
            .unwrap();
        assert_eq!(
            String::from_utf8(remote.stdout).unwrap().trim(),
            "https://example.com/team/repo.git"
        );
        std::fs::remove_dir_all(d).unwrap();
    }
}
