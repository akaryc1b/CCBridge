//! Explicit human-triggered project preparation, not automatic clone during matching.
use crate::common::shell_quote;
use anyhow::{Result, bail};
pub fn plan(group: &str, urls: &str) -> Result<Vec<(String, String)>> {
    if group.is_empty()
        || group.len() > 80
        || !group
            .bytes()
            .all(|c| c.is_ascii_alphanumeric() || c == b'-' || c == b'_')
    {
        bail!("项目组名称仅支持英文字母、数字、短横线和下划线")
    }
    let mut out = Vec::new();
    for raw in urls.split_whitespace() {
        let u =
            url::Url::parse(raw).map_err(|_| anyhow::anyhow!("仓库地址必须是完整 HTTPS URL"))?;
        if u.scheme() != "https"
            || u.host_str().is_none()
            || !u.username().is_empty()
            || u.password().is_some()
            || u.query().is_some()
            || u.fragment().is_some()
        {
            bail!("仅支持不含账号、密码或令牌参数的 HTTPS 仓库地址；认证沿用远端 Git 配置")
        }
        let name = u
            .path()
            .trim_end_matches('/')
            .rsplit('/')
            .next()
            .unwrap_or("")
            .trim_end_matches(".git");
        if name.is_empty()
            || name.len() > 100
            || !name
                .bytes()
                .all(|c| c.is_ascii_alphanumeric() || c == b'-' || c == b'_' || c == b'.')
            || [".", ".."].contains(&name)
        {
            bail!("仓库名称无效，不能作为目录名")
        }
        if out.iter().any(|(n, _)| n == name) {
            bail!("多个仓库使用同一目录名，请分别放入不同项目组")
        }
        out.push((name.into(), u.to_string()));
    }
    if out.is_empty() || out.len() > 10 {
        bail!("请填写 1–10 个仓库地址，以空格分隔")
    }
    Ok(out)
}
pub fn clone_script(home: &str, group: &str, name: &str, url: &str) -> String {
    let root = format!("{home}/projects/{group}");
    let dest = format!("{root}/{name}");
    format!(
        r#"# ccbridge:prepare-repository
set -eu
umask 077
root={root}
dest={dest}
base={base}
mkdir -p "$base"
[ "$(cd "$base" && pwd -P)" = "$base" ] || {{ printf 'CCBRIDGE_PATH_CONFLICT' >&2; exit 42; }}
[ ! -L "$root" ] || {{ printf 'CCBRIDGE_PATH_CONFLICT' >&2; exit 42; }}
mkdir -p "$root"
actual=$(cd "$root" && pwd -P)
[ "$actual" = "$root" ] || {{ printf 'CCBRIDGE_PATH_CONFLICT' >&2; exit 42; }}
if [ -e "$dest" ] || [ -L "$dest" ]; then printf 'CCBRIDGE_ALREADY_EXISTS' >&2; exit 43; fi
export GIT_TERMINAL_PROMPT=0
export GIT_ASKPASS=/usr/bin/false
git -c credential.interactive=false clone --quiet -- {url} "$dest"
"#,
        root = shell_quote(&root),
        base = shell_quote(&format!("{home}/projects")),
        dest = shell_quote(&dest),
        url = shell_quote(url)
    )
}
pub fn failure(stderr: &str) -> &'static str {
    let e = stderr.to_ascii_lowercase();
    if e.contains("username")
        || e.contains("authentication")
        || e.contains("access denied")
        || e.contains("403")
        || e.contains("401")
        || e.contains("repository not found")
    {
        "远端 Git 尚未获得此仓库访问权限；请在服务器配置有读取权限的 Codeup 凭据后重试。SSH 登录密码不等于 Git 仓库凭据"
    } else if e.contains("ccbridge_path_conflict") {
        "项目目录包含指向其他位置的符号链接，已停止，未覆盖目标"
    } else if e.contains("ccbridge_already_exists") {
        "目标目录已存在，已停止克隆；请检查其 Git 身份后重试"
    } else if e.contains("resolve host") {
        "服务器无法解析 Git 仓库域名，请检查远端 DNS"
    } else if e.contains("permission denied") {
        "当前 SSH 账号没有创建目录或读取仓库的权限"
    } else if e.contains("certificate") {
        "服务器无法验证 Git 服务的 HTTPS 证书，请修复证书链；不要关闭证书校验"
    } else {
        "远端 Git 拉取失败，请检查服务器网络、磁盘空间和 Git 认证。未修改已有仓库"
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn project_plan() {
        let p = plan(
            "project-a",
            "https://example.com/team/backend.git https://example.com/team/frontend.git",
        )
        .unwrap();
        assert_eq!(p[0].0, "backend");
        assert!(plan("../bad", "https://example.com/a.git").is_err());
        for s in [
            "https://u:secret@example.com/a.git",
            "https://example.com/a.git?token=x",
            "file:///tmp/x",
            "https://example.com/..git",
        ] {
            assert!(plan("a", s).is_err(), "{s}")
        }
        assert!(
            clone_script("/home/test", "a", "backend", &p[0].1)
                .contains("clone --quiet -- 'https://example.com/team/backend.git'")
        );
        assert!(failure("fatal: could not read Username").contains("Codeup"));
    }
}

#[cfg(test)]
mod clone_tests {
    use super::*;
    #[test]
    fn clone_and_preserve_existing() {
        use std::{fs, process::Command};
        let root =
            std::env::temp_dir().join(format!("ccbridge-provision-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(root.join("source/backend.git")).unwrap();
        assert!(
            Command::new("git")
                .args(["init", "--bare", "-q"])
                .arg(root.join("source/backend.git"))
                .status()
                .unwrap()
                .success()
        );
        let home = root.join("account");
        fs::create_dir(&home).unwrap();
        let home = home.canonicalize().unwrap();
        let script = clone_script(
            home.to_str().unwrap(),
            "example",
            "backend",
            "https://example.com/team/backend.git",
        );
        let run = || {
            Command::new("sh")
                .arg("-c")
                .arg(&script)
                .env("GIT_CONFIG_GLOBAL", "/dev/null")
                .env("GIT_CONFIG_NOSYSTEM", "1")
                .env("GIT_CONFIG_COUNT", "1")
                .env(
                    "GIT_CONFIG_KEY_0",
                    format!("url.file://{}/.insteadOf", root.join("source").display()),
                )
                .env("GIT_CONFIG_VALUE_0", "https://example.com/team/")
                .output()
                .unwrap()
        };
        let out = run();
        assert!(
            out.status.success(),
            "{}",
            String::from_utf8_lossy(&out.stderr)
        );
        let dest = home.join("projects/example/backend");
        assert!(dest.join(".git").is_dir());
        fs::write(dest.join("keep-local.txt"), "keep").unwrap();
        let repeat = run();
        assert_eq!(repeat.status.code(), Some(43));
        assert_eq!(
            fs::read_to_string(dest.join("keep-local.txt")).unwrap(),
            "keep"
        );
        #[cfg(unix)]
        {
            std::os::unix::fs::symlink(root.join("source"), home.join("projects/escape")).unwrap();
            let cmd = clone_script(
                home.to_str().unwrap(),
                "escape",
                "backend",
                "https://example.com/team/backend.git",
            );
            assert_eq!(
                Command::new("sh")
                    .arg("-c")
                    .arg(cmd)
                    .status()
                    .unwrap()
                    .code(),
                Some(42)
            );
        }
        fs::remove_dir_all(root).unwrap();
    }
}
