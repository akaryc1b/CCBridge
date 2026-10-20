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
fn git(args: &[&str]) -> Result<()> {
    let mut c = Command::new("git")
        .args(args)
        .env("GIT_TERMINAL_PROMPT", "0")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .context("无法启动本机 Git")?;
    let mut o = c.stdout.take().unwrap();
    let mut e = c.stderr.take().unwrap();
    let out = std::thread::spawn(move || {
        let mut b = Vec::new();
        let _ = o.read_to_end(&mut b);
    });
    let err = std::thread::spawn(move || {
        let mut b = Vec::new();
        let _ = e.read_to_end(&mut b);
        b
    });
    let deadline = Instant::now() + Duration::from_secs(300);
    let status = loop {
        if let Some(s) = c.try_wait()? {
            break s;
        }
        if Instant::now() >= deadline {
            let _ = c.kill();
            let _ = c.wait();
            bail!("本机 Git 读取超时；请检查本机 Codeup 认证和网络")
        };
        std::thread::sleep(Duration::from_millis(40));
    };
    let _ = out.join();
    let errors = err.join().unwrap_or_default();
    if !status.success() {
        bail!(
            "本机 Git 无法获取仓库：{}",
            crate::provision::failure(&String::from_utf8_lossy(&errors))
        )
    }
    Ok(())
}
pub fn bundle(url: &str) -> Result<Bundle> {
    let dir = crate::common::app_dir()?
        .join("transfers")
        .join(uuid::Uuid::new_v4().to_string());
    crate::common::private_dir(&dir)?;
    let b = Bundle {
        path: dir.join("repository.bundle"),
        dir: dir.clone(),
    };
    let mirror = dir.join("mirror.git");
    git(&[
        "clone",
        "--mirror",
        "--quiet",
        "--",
        url,
        mirror.to_str().context("本机临时路径无效")?,
    ])?;
    git(&[
        "-C",
        mirror.to_str().unwrap(),
        "bundle",
        "create",
        b.path.to_str().unwrap(),
        "--all",
    ])?;
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
