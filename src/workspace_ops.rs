//! Human-only workspace operations, executed by the existing per-account Runner.
use crate::common::*;
use anyhow::{Context, Result, bail};
use serde_json::{Value, json};
use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
};
fn inside(home: &Path, path: &str) -> Result<PathBuf> {
    let raw = Path::new(path);
    if !raw.is_absolute()
        || raw
            .components()
            .any(|c| matches!(c, std::path::Component::ParentDir))
    {
        bail!("请使用账号 HOME 内的绝对路径，不允许 ..")
    }
    let parent = raw.parent().context("不能操作账号 HOME")?.canonicalize()?;
    let name = raw.file_name().context("路径无效")?;
    let p = parent.join(name);
    if !parent.starts_with(home) || p == home {
        bail!("不能操作 HOME 或 HOME 以外的目录")
    }
    for component in p.strip_prefix(home)?.components() {
        let c = component.as_os_str().to_string_lossy();
        if [
            ".git", ".ssh", ".claude", ".codex", ".aws", ".gnupg", ".kube",
        ]
        .contains(&c.as_ref())
        {
            bail!("不能通过文件管理修改 Git 元数据或凭据目录")
        }
    }
    if p.starts_with(home.join(".local")) || p.starts_with(home.join(".config")) {
        bail!("不能通过文件管理修改账号运行配置目录")
    }
    Ok(p)
}
fn name(s: &str) -> Result<()> {
    if s.is_empty() || [".", ".."].contains(&s) || s.contains(['/', '\0', '\n', '\r']) {
        bail!("名称不能为空，也不能包含斜杠或换行")
    };
    Ok(())
}
fn move_exclusive(from: &Path, to: &Path) -> Result<()> {
    use std::os::unix::ffi::OsStrExt;
    let f = std::ffi::CString::new(from.as_os_str().as_bytes())?;
    let t = std::ffi::CString::new(to.as_os_str().as_bytes())?;
    #[cfg(target_os = "linux")]
    let rc = unsafe {
        libc::renameat2(
            libc::AT_FDCWD,
            f.as_ptr(),
            libc::AT_FDCWD,
            t.as_ptr(),
            libc::RENAME_NOREPLACE,
        )
    };
    #[cfg(target_os = "macos")]
    let rc = unsafe { libc::renamex_np(f.as_ptr(), t.as_ptr(), libc::RENAME_EXCL) };
    #[cfg(not(any(target_os = "linux", target_os = "macos")))]
    let rc = -1;
    if rc != 0 {
        let e = std::io::Error::last_os_error();
        if e.kind() == std::io::ErrorKind::AlreadyExists {
            bail!("同名目标已存在，未覆盖")
        };
        bail!("移动失败（未覆盖目标；跨磁盘移动暂不支持）：{e}")
    };
    Ok(())
}
fn protected(p: &Path, home: &Path, extra: &[String]) -> Result<()> {
    let projects = crate::projects::rpc(&json!({"op":"project_list"}))?;
    for path in projects["projects"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|v| v["path"].as_str())
        .chain(extra.iter().map(String::as_str))
    {
        if !path.is_empty() && Path::new(path).starts_with(p) {
            bail!("目录是当前工作区、已注册项目或其上级，请先调整项目配置")
        }
    }
    if p == home {
        bail!("不能修改 HOME")
    };
    Ok(())
}
fn git(path: &Path, args: &[&str]) -> Result<String> {
    let o = Command::new("git")
        .arg("-C")
        .arg(path)
        .args([
            "-c",
            "core.hooksPath=/dev/null",
            "-c",
            "core.fsmonitor=false",
        ])
        .args(args)
        .env("GIT_TERMINAL_PROMPT", "0")
        .output()?;
    if !o.status.success() {
        bail!(
            "Git 操作失败：请检查分支是否存在、是否已被 worktree 使用，或远端引用是否过期；未强制覆盖改动"
        )
    };
    Ok(String::from_utf8(o.stdout)?.trim().into())
}
pub fn rpc(v: &Value) -> Result<Value> {
    rpc_at(v, &home()?.canonicalize()?)
}
fn rpc_at(v: &Value, h: &Path) -> Result<Value> {
    let trash = h.join(".local/share/ccbridge/trash");
    let op = field(v, "op")?;
    if trash.exists() && trash.canonicalize()? != trash {
        bail!("回收区路径含符号链接，已停止")
    }
    if op == "trash_list" {
        let mut items = vec![];
        if trash.exists() {
            for entry in fs::read_dir(&trash)? {
                let e = entry?;
                if let Ok(meta) = read_json::<Value>(&e.path().join("meta.json")) {
                    if e.path().join("item").symlink_metadata().is_ok() {
                        items.push(meta)
                    }
                }
            }
        }
        items.sort_by_key(|x| std::cmp::Reverse(x["deleted_at"].as_u64().unwrap_or(0)));
        return Ok(json!({"items":items}));
    }
    if op == "trash_restore" {
        let id = field(v, "id")?;
        uuid::Uuid::parse_str(id)?;
        let slot = trash.join(id);
        if slot.canonicalize()? != slot {
            bail!("回收项路径含符号链接，已停止")
        }
        let meta: Value = read_json(&slot.join("meta.json"))?;
        let target = inside(h, field(&meta, "original")?)?;
        let _lease = crate::scheduler::operation(&[target.clone()], "恢复文件")?;
        protected(&target, h, &[])?;
        move_exclusive(&slot.join("item"), &target)?;
        fs::remove_file(slot.join("meta.json"))?;
        fs::remove_dir(slot)?;
        return Ok(json!({"message":"已恢复到原路径","path":target}));
    }
    if ["file_create", "file_mkdir"].contains(&op) {
        let parent = Path::new(field(v, "path")?).canonicalize()?;
        let n = field(v, "name")?;
        name(n)?;
        let target = inside(h, parent.join(n).to_str().context("路径无效")?)?;
        let _lease = crate::scheduler::operation(&[target.clone()], "新建文件")?;
        if op == "file_mkdir" {
            fs::create_dir(&target).context("新建文件夹失败，可能已有同名项；未覆盖")?;
            return Ok(json!({"message":"已新建文件夹","path":target}));
        }
        use std::os::unix::fs::OpenOptionsExt;
        fs::OpenOptions::new()
            .create_new(true)
            .write(true)
            .mode(0o600)
            .open(&target)
            .context("新建文件失败，可能已有同名项；未覆盖")?;
        return Ok(json!({"message":"已新建空文件","path":target}));
    }
    let p = inside(h, field(v, "path")?)?;
    if !p.try_exists()? && p.symlink_metadata().is_err() {
        bail!("文件或文件夹不存在，请刷新目录")
    }
    let _lease = if ["file_mutate", "branch_change"].contains(&op) {
        let mut paths = vec![p.canonicalize().unwrap_or(p.clone())];
        if op == "file_mutate" {
            if v["action"] == "move" {
                paths.push(Path::new(field(v, "destination")?).canonicalize()?);
            }
            if v["action"] == "rename" {
                paths.push(p.parent().context("路径无效")?.canonicalize()?);
            }
        }
        Some(crate::scheduler::operation(
            &paths,
            if op == "branch_change" {
                "修改分支"
            } else {
                "修改文件"
            },
        )?)
    } else {
        None
    };
    if op == "file_mutate" {
        let extra = v["protected"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|v| v.as_str().map(String::from))
            .collect::<Vec<_>>();
        protected(&p, h, &extra)?;
        match field(v, "action")? {
            "rename" => {
                let n = field(v, "name")?;
                name(n)?;
                let target = inside(h, p.parent().unwrap().join(n).to_str().context("路径无效")?)?;
                protected(&target, h, &extra)?;
                move_exclusive(&p, &target)?;
                Ok(json!({"message":"已重命名","path":target}))
            }
            "move" => {
                let dest = Path::new(field(v, "destination")?).canonicalize()?;
                if !dest.is_dir() || !dest.starts_with(h) {
                    bail!("目标目录必须在当前账号 HOME 内")
                };
                let target = inside(
                    h,
                    dest.join(p.file_name().unwrap())
                        .to_str()
                        .context("路径无效")?,
                )?;
                if target.starts_with(&p) {
                    bail!("不能把文件夹移动到自身或子目录")
                };
                protected(&target, h, &extra)?;
                move_exclusive(&p, &target)?;
                Ok(json!({"message":"已移动","path":target}))
            }
            "trash" => {
                let id = uuid::Uuid::new_v4().to_string();
                private_dir(&trash)?;
                if trash.canonicalize()? != trash {
                    bail!("回收区路径含符号链接，已停止")
                }
                let slot = trash.join(&id);
                private_dir(&slot)?;
                let meta = json!({"id":id,"original":p,"name":p.file_name().unwrap().to_string_lossy(),"deleted_at":now()});
                write_json(&slot.join("meta.json"), &meta)?;
                if let Err(e) = move_exclusive(&p, &slot.join("item")) {
                    let _ = fs::remove_file(slot.join("meta.json"));
                    let _ = fs::remove_dir(&slot);
                    return Err(e);
                };
                Ok(json!({"message":"已移入回收区，可恢复","id":id}))
            }
            _ => bail!("不支持的文件操作"),
        }
    } else {
        if p.symlink_metadata()?.file_type().is_symlink() {
            bail!("不能在符号链接目录中修改分支")
        }
        let repo = crate::repository::inspect(&p)?;
        if Path::new(&repo.path) != p {
            bail!("请先打开 Git 仓库根目录")
        }
        if op == "branch_change" {
            if v["expected_current"]
                .as_str()
                .is_some_and(|n| n != repo.branch)
            {
                bail!("当前分支已变化，请刷新后重新操作")
            }
            if repo.dirty {
                bail!("仓库有未提交改动，请先处理；不会自动 stash 或丢弃")
            }
            if let Some(result) = crate::branch_ops::change(&p, v)? {
                return Ok(result);
            }
            let branch = field(v, "branch")?;
            git(&p, &["check-ref-format", "--branch", branch])?;
            match field(v, "action")? {
                "switch" => {
                    git(&p, &["switch", "--no-guess", branch])?;
                }
                "create" => {
                    if let Some(base) = v["start_point"].as_str().filter(|s| !s.is_empty()) {
                        git(&p, &["show-ref", "--verify", &format!("refs/heads/{base}")])?;
                        git(&p, &["switch", "-c", branch, &format!("refs/heads/{base}")])?;
                    } else {
                        git(&p, &["switch", "-c", branch])?;
                    }
                }
                "fast_forward" => {
                    let reference = format!("refs/remotes/{branch}");
                    git(&p, &["show-ref", "--verify", &reference])?;
                    git(&p, &["merge", "--ff-only", "--", &reference])?;
                }
                "track" => {
                    let remote = field(v, "remote")?;
                    if !git(
                        &p,
                        &["for-each-ref", "--format=%(refname:short)", "refs/remotes"],
                    )?
                    .lines()
                    .any(|r| r == remote)
                    {
                        bail!("远端分支不存在，请刷新引用")
                    };
                    git(&p, &["switch", "--track", "-c", branch, remote])?;
                }
                _ => bail!("不支持的分支操作"),
            }
        }
        crate::branch_ops::inspect(&p, v)
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn files_collision_move_trash_restore_and_guards() {
        let tmp = std::env::temp_dir().join(uuid::Uuid::new_v4().to_string());
        private_dir(&tmp).unwrap();
        let h = tmp.canonicalize().unwrap();
        rpc_at(&json!({"op":"file_create","path":h,"name":"new.txt"}), &h).unwrap();
        assert!(rpc_at(&json!({"op":"file_create","path":h,"name":"new.txt"}), &h).is_err());
        fs::create_dir(h.join("folder")).unwrap();
        fs::write(h.join("file.txt"), "keep").unwrap();
        fs::write(h.join("other.txt"), "other").unwrap();
        let call = |v: Value| rpc_at(&v, &h);
        let file = h.join("file.txt").to_string_lossy().to_string();
        assert!(
            call(json!({"op":"file_mutate","path":file,"action":"rename","name":"other.txt"}))
                .is_err()
        );
        assert_eq!(fs::read_to_string(h.join("other.txt")).unwrap(), "other");
        call(json!({"op":"file_mutate","path":file,"action":"rename","name":"新 文件.txt"}))
            .unwrap();
        let renamed = h.join("新 文件.txt");
        call(json!({"op":"file_mutate","path":renamed,"action":"move","destination":h.join("folder")})).unwrap();
        let moved = h.join("folder/新 文件.txt");
        let r = call(json!({"op":"file_mutate","path":moved,"action":"trash"})).unwrap();
        assert!(!moved.exists());
        assert_eq!(
            call(json!({"op":"trash_list"})).unwrap()["items"]
                .as_array()
                .unwrap()
                .len(),
            1
        );
        fs::write(&moved, "collision").unwrap();
        assert!(call(json!({"op":"trash_restore","id":r["id"]})).is_err());
        fs::remove_file(&moved).unwrap();
        call(json!({"op":"trash_restore","id":r["id"]})).unwrap();
        assert_eq!(fs::read_to_string(&moved).unwrap(), "keep");
        let dir = h.join("folder");
        assert!(
            call(json!({"op":"file_mutate","path":dir,"action":"move","destination":dir})).is_err()
        );
        assert!(call(json!({"op":"file_mutate","path":h,"action":"trash"})).is_err());
        assert!(
            call(json!({"op":"file_mutate","path":dir,"action":"trash","protected":[moved]}))
                .is_err()
        );
        std::os::unix::fs::symlink("/", h.join("escape")).unwrap();
        assert!(inside(&h, h.join("escape/etc/passwd").to_str().unwrap()).is_err());
        fs::create_dir(h.join(".git")).unwrap();
        assert!(inside(&h, h.join(".git/config").to_str().unwrap()).is_err());
        fs::remove_dir_all(h).unwrap();
    }
    #[test]
    fn branches_never_discard_dirty_work() {
        let tmp = std::env::temp_dir().join(uuid::Uuid::new_v4().to_string());
        private_dir(&tmp).unwrap();
        let h = tmp.canonicalize().unwrap();
        let repo = h.join("repo");
        fs::create_dir(&repo).unwrap();
        git(&repo, &["init", "-q"]).unwrap();
        fs::write(repo.join("README"), "first").unwrap();
        git(&repo, &["add", "README"]).unwrap();
        git(
            &repo,
            &[
                "-c",
                "user.name=fixture",
                "-c",
                "user.email=fixture@example.test",
                "commit",
                "-qm",
                "first",
            ],
        )
        .unwrap();
        let base = git(&repo, &["branch", "--show-current"]).unwrap();
        rpc_at(
            &json!({"op":"branch_change","path":repo,"action":"create","branch":"feature/test"}),
            &h,
        )
        .unwrap();
        assert_eq!(
            git(&repo, &["branch", "--show-current"]).unwrap(),
            "feature/test"
        );
        fs::write(repo.join("README"), "uncommitted").unwrap();
        assert!(
            rpc_at(
                &json!({"op":"branch_change","path":repo,"action":"switch","branch":base}),
                &h
            )
            .is_err()
        );
        assert_eq!(
            fs::read_to_string(repo.join("README")).unwrap(),
            "uncommitted"
        );
        fs::write(repo.join("README"), "first").unwrap();
        rpc_at(
            &json!({"op":"branch_change","path":repo,"action":"switch","branch":base}),
            &h,
        )
        .unwrap();
        assert_eq!(git(&repo, &["branch", "--show-current"]).unwrap(), base);
        // A source refresh only advances to an existing remote ref; never resets divergence.
        let before = git(&repo, &["rev-parse", "HEAD"]).unwrap();
        fs::write(repo.join("README"), "remote update").unwrap();
        git(&repo, &["add", "README"]).unwrap();
        git(
            &repo,
            &[
                "-c",
                "user.name=fixture",
                "-c",
                "user.email=fixture@example.test",
                "commit",
                "-qm",
                "remote",
            ],
        )
        .unwrap();
        let remote = git(&repo, &["rev-parse", "HEAD"]).unwrap();
        git(&repo, &["update-ref", "refs/remotes/origin/main", &remote]).unwrap();
        git(&repo, &["reset", "--hard", &before]).unwrap(); // disposable fixture only
        let update = json!({"op":"branch_change","path":repo,"action":"fast_forward","branch":"origin/main"});
        rpc_at(&update, &h).unwrap();
        assert_eq!(git(&repo, &["rev-parse", "HEAD"]).unwrap(), remote);
        assert_eq!(git(&repo, &["branch", "--show-current"]).unwrap(), base);
        git(&repo, &["reset", "--hard", &before]).unwrap();
        fs::write(repo.join("README"), "local update").unwrap();
        git(&repo, &["add", "README"]).unwrap();
        git(
            &repo,
            &[
                "-c",
                "user.name=fixture",
                "-c",
                "user.email=fixture@example.test",
                "commit",
                "-qm",
                "local",
            ],
        )
        .unwrap();
        let local = git(&repo, &["rev-parse", "HEAD"]).unwrap();
        assert!(rpc_at(&update, &h).is_err());
        assert_eq!(git(&repo, &["rev-parse", "HEAD"]).unwrap(), local);
        fs::remove_dir_all(h).unwrap();
    }
    #[test]
    fn branch_delete_runs_under_own_lease_and_uses_requested_start_point() {
        let tmp = std::env::temp_dir().join(uuid::Uuid::new_v4().to_string());
        private_dir(&tmp).unwrap();
        let h = tmp.canonicalize().unwrap();
        let p = h.join("repo");
        fs::create_dir(&p).unwrap();
        git(&p, &["init", "-q", "--initial-branch=main"]).unwrap();
        git(
            &p,
            &[
                "-c",
                "user.name=fixture",
                "-c",
                "user.email=fixture@example.invalid",
                "commit",
                "--allow-empty",
                "-qm",
                "initial",
            ],
        )
        .unwrap();
        let call = |v: Value| rpc_at(&v, &h);
        call(json!({"op":"branch_change","path":p,"action":"create","branch":"feature/a","start_point":"main"})).unwrap();
        call(json!({"op":"branch_change","path":p,"action":"switch","branch":"main"})).unwrap();
        let state = call(json!({"op":"branches","path":p})).unwrap();
        let a = state["details"]
            .as_array()
            .unwrap()
            .iter()
            .find(|b| b["name"] == "feature/a")
            .unwrap();
        assert_eq!(a["deletable"], true);
        let r=call(json!({"op":"branch_change","path":p,"action":"delete","base":state["base"],"base_oid":state["base_oid"],"selections":[{"name":"feature/a","oid":a["oid"]}]})).unwrap();
        assert_eq!(r["results"][0]["ok"], true);
        assert!(
            !r["branches"]
                .as_array()
                .unwrap()
                .iter()
                .any(|b| b == "feature/a")
        );
        fs::remove_dir_all(h).unwrap();
    }
}
