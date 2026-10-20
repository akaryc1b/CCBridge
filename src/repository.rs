//! Git identities are compared without exposing URL passwords, query tokens or SSH usernames.
use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeSet,
    io::Read,
    path::{Path, PathBuf},
    process::{Command, Stdio},
    time::{Duration, Instant},
};

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct Repository {
    pub path: String,
    pub name: String,
    pub identities: Vec<String>,
    pub branch: String,
    pub head: String,
    pub dirty: bool,
}
fn git(path: &Path, args: &[&str]) -> Result<String> {
    let mut child = Command::new("git")
        .arg("-C")
        .arg(path)
        .args([
            "-c",
            "core.fsmonitor=false",
            "-c",
            "core.untrackedCache=false",
        ])
        .args(args)
        .env("GIT_TERMINAL_PROMPT", "0")
        .env("GIT_OPTIONAL_LOCKS", "0")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .context("需要 Git 命令")?;
    let mut stdout = child.stdout.take().unwrap();
    let mut stderr = child.stderr.take().unwrap();
    let output = std::thread::spawn(move || {
        let mut b = Vec::new();
        let _ = stdout.read_to_end(&mut b);
        b
    });
    let errors = std::thread::spawn(move || {
        let mut b = Vec::new();
        let _ = stderr.read_to_end(&mut b);
        b
    });
    let deadline = Instant::now() + Duration::from_secs(5);
    let status = loop {
        if let Some(s) = child.try_wait()? {
            break s;
        }
        if Instant::now() >= deadline {
            let _ = child.kill();
            let _ = child.wait();
            let _ = output.join();
            let _ = errors.join();
            bail!(
                "Git 读取超时；请通过“选择本地仓库”重新选择目录，并检查 macOS 的文件与文件夹访问权限。SSH 无需重新登录"
            )
        }
        std::thread::sleep(Duration::from_millis(20));
    };
    let bytes = output.join().unwrap_or_default();
    let _ = errors.join();
    if !status.success() {
        bail!("Git 操作失败：{}", args.join(" "))
    }
    Ok(String::from_utf8(bytes)?.trim().into())
}
pub fn identity(raw: &str) -> Option<String> {
    let raw = raw.trim();
    let (host, port, path) = if raw.contains("://") {
        let u = url::Url::parse(raw).ok()?;
        if !["ssh", "https", "http", "git"].contains(&u.scheme()) {
            return None;
        }
        (
            u.host_str()?.to_ascii_lowercase(),
            u.port().filter(|p| ![22, 80, 443, 9418].contains(p)),
            u.path().to_owned(),
        )
    } else {
        let (host, path) = raw.split_once(':')?;
        if host.contains('/') || host.is_empty() {
            return None;
        }
        (
            host.rsplit('@').next()?.to_ascii_lowercase(),
            None,
            path.split(['?', '#']).next()?.to_owned(),
        )
    };
    let path = path
        .trim_matches('/')
        .strip_suffix(".git")
        .unwrap_or(path.trim_matches('/'));
    if path.is_empty() {
        return None;
    }
    Some(format!(
        "{}{}/{}",
        host,
        port.map(|p| format!(":{p}")).unwrap_or_default(),
        path
    ))
}
pub fn inspect(path: &Path) -> Result<Repository> {
    if path.as_os_str().is_empty() {
        bail!("请先选择本地 Git 仓库目录；空路径不能用于仓库匹配")
    }
    if !path.is_absolute() {
        bail!("请使用本地仓库的绝对路径")
    }
    if !path.is_dir() {
        bail!("选择的本地目录不存在或无法访问")
    }
    let root = PathBuf::from(git(path, &["rev-parse", "--show-toplevel"])
        .context("所选目录不是可访问的 Git 工作区。包含多个仓库的父目录不能直接匹配，请选择其中一个有 .git 的项目目录")?).canonicalize()?;
    let mut identities = BTreeSet::new();
    let remotes = git(&root, &["remote"])?;
    let names: Vec<&str> = remotes.lines().collect();
    let chosen = if names.contains(&"origin") {
        vec!["origin"]
    } else if names.len() <= 1 {
        names
    } else {
        bail!(
            "多个 Git remote 且没有 origin，无法确定仓库主身份；请设置 origin 或明确使用手动远端目录"
        )
    };
    for remote in chosen {
        if let Ok(urls) = git(&root, &["remote", "get-url", "--all", remote]) {
            for url in urls.lines() {
                if let Some(id) = identity(url) {
                    identities.insert(id);
                }
            }
        }
    }
    let branch = git(&root, &["symbolic-ref", "--short", "HEAD"])
        .unwrap_or_else(|_| "(detached HEAD)".into());
    let head =
        git(&root, &["rev-parse", "--short", "HEAD"]).unwrap_or_else(|_| "(no commit)".into());
    let dirty = !git(
        &root,
        &["status", "--porcelain", "--untracked-files=normal"],
    )?
    .is_empty();
    Ok(Repository {
        path: root.to_string_lossy().into(),
        name: root
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .into(),
        identities: identities.into_iter().collect(),
        branch,
        head,
        dirty,
    })
}
#[derive(Serialize)]
pub struct Scan {
    pub repositories: Vec<Repository>,
    pub root: String,
    pub visited: usize,
    pub truncated: bool,
    pub unreadable_directories: usize,
    pub failed_repositories: usize,
}
pub fn scan(root: &Path, home: &Path) -> Result<Scan> {
    let home = home.canonicalize()?;
    let root = root.canonicalize()?;
    if !root.starts_with(&home) {
        bail!("仓库搜索范围必须在当前远端账号 HOME 下")
    }
    let mut r = Scan {
        repositories: vec![],
        root: root.to_string_lossy().into(),
        visited: 0,
        truncated: false,
        unreadable_directories: 0,
        failed_repositories: 0,
    };
    let mut stack = vec![(root, 0)];
    let deadline = Instant::now() + Duration::from_secs(20);
    while let Some((dir, depth)) = stack.pop() {
        if r.visited >= 25000 || Instant::now() > deadline {
            r.truncated = true;
            break;
        }
        r.visited += 1;
        if dir.join(".git").exists() {
            match inspect(&dir) {
                Ok(repo) => r.repositories.push(repo),
                Err(_) => r.failed_repositories += 1,
            }
        }
        if depth >= 8 {
            r.truncated = true;
            continue;
        }
        let entries = match std::fs::read_dir(&dir) {
            Ok(e) => e,
            Err(_) => {
                r.unreadable_directories += 1;
                continue;
            }
        };
        for e in entries.flatten() {
            let Ok(t) = e.file_type() else { continue };
            if !t.is_dir() || t.is_symlink() {
                continue;
            }
            let name = e.file_name();
            let s = name.to_string_lossy();
            if s.starts_with('.')
                || [
                    "node_modules",
                    "target",
                    "vendor",
                    "Library",
                    "Applications",
                    "venv",
                    "__pycache__",
                ]
                .contains(&s.as_ref())
            {
                continue;
            }
            stack.push((e.path(), depth + 1));
        }
    }
    r.repositories.sort_by(|a, b| a.path.cmp(&b.path));
    Ok(r)
}
pub fn same(a: &Repository, b: &Repository) -> bool {
    a.identities.iter().any(|id| b.identities.contains(id))
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn empty_local_path_is_actionable() {
        assert!(
            inspect(Path::new(""))
                .unwrap_err()
                .to_string()
                .contains("请先选择")
        );
        assert!(
            inspect(Path::new("relative"))
                .unwrap_err()
                .to_string()
                .contains("绝对路径")
        );
    }
    #[test]
    fn normalize_remote_urls() {
        assert_eq!(
            identity("git@github.com:team/service.git"),
            identity("https://name:secret@github.com/team/service.git?token=secret")
        );
        assert_eq!(
            identity("ssh://git@git.example.com:22/Team/repo.git"),
            Some("git.example.com/Team/repo".into())
        );
        assert_eq!(
            identity("ssh://git@git.example.com:2222/Team/repo.git"),
            Some("git.example.com:2222/Team/repo".into())
        );
        assert!(identity("/local/clone").is_none());
        assert!(
            identity("https://git.example.com/team/repo")
                .unwrap()
                .contains("team/repo")
        );
    }
}
