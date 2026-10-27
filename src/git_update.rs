//! Explicit fast-forward updates. Never pick a branch, stash, reset or push.
use crate::common::*;
use anyhow::{Context, Result, bail};
use serde_json::{Value, json};
use std::{path::Path, process::Command};
fn git(p: &Path, args: &[&str]) -> Result<String> {
    let out = Command::new("git")
        .arg("-C")
        .arg(p)
        .args([
            "-c",
            "core.hooksPath=/dev/null",
            "-c",
            "core.fsmonitor=false",
        ])
        .args(args)
        .env("GIT_TERMINAL_PROMPT", "0")
        .env("GIT_OPTIONAL_LOCKS", "0")
        .output()?;
    if !out.status.success() {
        bail!("仓库更新失败；改动已保留，请检查当前分支及其远端分支")
    }
    Ok(String::from_utf8(out.stdout)?.trim().into())
}
fn repo(p: &Path) -> Result<Value> {
    let r = crate::repository::inspect(p)?;
    if Path::new(&r.path) != p {
        bail!("请打开仓库根目录或包含仓库的项目目录")
    }
    let upstream = git(
        p,
        &[
            "rev-parse",
            "--abbrev-ref",
            "--symbolic-full-name",
            "@{upstream}",
        ],
    )
    .unwrap_or_default();
    let remote = git(
        p,
        &["config", "--get", &format!("branch.{}.remote", r.branch)],
    )
    .unwrap_or_default();
    let reason = if r.dirty {
        "有未提交改动，请先处理"
    } else if upstream.is_empty() || remote.is_empty() || remote == "." {
        "当前分支未配置远端跟踪分支，请先在分支管理中设置"
    } else {
        ""
    };
    let head = git(p, &["rev-parse", "HEAD"])?;
    let url = git(p, &["remote", "get-url", "--", &remote])
        .ok()
        .and_then(|s| url::Url::parse(&s).ok())
        .filter(|u| u.scheme() == "https" && u.host_str() == Some("codeup.aliyun.com"))
        .map(|mut u| {
            let _ = u.set_username("");
            let _ = u.set_password(None);
            u.set_query(None);
            u.set_fragment(None);
            u.to_string()
        })
        .unwrap_or_default();
    Ok(
        json!({"url":url,"path":r.path,"name":r.name,"branch":r.branch,"head":head,"dirty":r.dirty,"upstream":upstream,"remote":remote,"blocked":reason}),
    )
}
pub fn rpc(v: &Value) -> Result<Value> {
    rpc_at(v, &home()?.canonicalize()?)
}
pub fn rpc_at(v: &Value, home: &Path) -> Result<Value> {
    let p = std::fs::canonicalize(field(v, "path")?).context("项目目录不存在")?;
    if !p.starts_with(home) {
        bail!("Git 更新仅允许当前账号 HOME 内的目录")
    }
    if [
        "git_update_begin",
        "git_update_preflight",
        "git_update_apply",
    ]
    .contains(&v["op"].as_str().unwrap_or(""))
    {
        crate::policy::allow_git(v["origin"].as_str())?;
    }
    if v["op"] == "git_update_begin" {
        return crate::scheduler::begin(&p, "Git 更新", v["lease_token"].as_str());
    }
    if v["op"] == "git_update_end" {
        crate::scheduler::end(field(v, "lease_token")?, &p)?;
        return Ok(json!({"released":true}));
    }
    match field(v, "op")? {
        "git_update_plan" => {
            let mut repos = vec![];
            if p.join(".git").exists() {
                repos.push(repo(&p)?);
            } else {
                for entry in std::fs::read_dir(&p)? {
                    let entry = entry?;
                    if entry.file_type()?.is_dir() && entry.path().join(".git").exists() {
                        repos.push(repo(&entry.path().canonicalize()?)?);
                    }
                    if repos.len() > 32 {
                        bail!("项目下仓库超过32个，请选择具体仓库")
                    }
                }
            }
            repos.sort_by_key(|r| r["path"].as_str().unwrap_or("").to_string());
            if repos.is_empty() {
                bail!("此目录及直接子目录没有 Git 仓库，请选择仓库或项目根目录")
            }
            Ok(json!({"repositories":repos}))
        }
        "git_update_preflight" | "git_update_apply" => {
            let _token_lease = if let Some(token) = v["lease_token"].as_str() {
                Some(crate::scheduler::use_lease(token, &p)?)
            } else {
                None
            };
            let _lease = if v["lease_token"].is_string() {
                None
            } else {
                Some(crate::scheduler::operation(&[p.clone()], "更新仓库")?)
            };
            let r = repo(&p)?;
            if r["blocked"] != "" {
                return Err(coded(
                    if r["dirty"] == true {
                        "dirty"
                    } else {
                        "no_upstream"
                    },
                    r["blocked"].as_str().unwrap_or("无法更新"),
                ));
            }
            for key in ["head", "branch", "upstream", "remote"] {
                if let Some(expected) = v["expected"][key].as_str() {
                    if r[key] != expected {
                        return Err(coded("stale", "仓库的分支或提交已变化，请刷新"));
                    }
                }
            }
            if field(v, "op")? == "git_update_preflight" {
                return Ok(r);
            }
            // Ref was fetched using the transport's credentials. Confirm ancestry before touching worktree.
            let head = r["head"].as_str().unwrap();
            let upstream = r["upstream"].as_str().unwrap();
            let target = git(&p, &["rev-parse", upstream])?;
            if head == target {
                return Ok(
                    json!({"path":p,"branch":r["branch"],"before":head,"after":head,"state":"unchanged","message":"已是最新"}),
                );
            }
            if git(&p, &["merge-base", "--is-ancestor", &target, head]).is_ok() {
                return Ok(
                    json!({"path":p,"branch":r["branch"],"before":head,"after":head,"state":"ahead","message":"本地有领先提交，保留原状；不会推送"}),
                );
            }
            if git(&p, &["merge-base", "--is-ancestor", head, &target]).is_err() {
                return Err(coded("diverged", "已分叉，需手动处理"));
            }
            git(&p, &["merge", "--ff-only", "--", upstream])?;
            Ok(
                json!({"path":p,"branch":r["branch"],"before":head,"after":git(&p,&["rev-parse","HEAD"] )?,"state":"updated","message":"仓库已更新"}),
            )
        }
        _ => bail!("不支持的 Git 更新操作"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn commit(p: &Path, text: &str) {
        std::fs::write(p.join("README"), text).unwrap();
        git(p, &["add", "README"]).unwrap();
        git(
            p,
            &[
                "-c",
                "user.name=fixture",
                "-c",
                "user.email=fixture@example.test",
                "commit",
                "-qm",
                text,
            ],
        )
        .unwrap();
    }
    #[test]
    fn fast_forward_dirty_divergence_and_changed_head_guards() {
        let root = std::env::temp_dir().join(format!("ccb-update-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir(&root).unwrap();
        let root = root.canonicalize().unwrap();
        let source = root.join("source");
        std::fs::create_dir(&source).unwrap();
        git(&source, &["init", "-q", "-b", "main"]).unwrap();
        commit(&source, "one");
        let p = root.join("clone");
        git(
            &root,
            &["clone", source.to_str().unwrap(), p.to_str().unwrap()],
        )
        .unwrap();
        let plan = rpc_at(&json!({"op":"git_update_plan","path":p}), &root).unwrap();
        let expected = plan["repositories"][0].clone();
        assert_eq!(expected["blocked"], "");
        let update = json!({"op":"git_update_apply","path":p,"expected":expected});
        assert_eq!(rpc_at(&update, &root).unwrap()["state"], "unchanged");
        commit(&source, "two");
        git(&p, &["fetch", "origin"]).unwrap();
        std::fs::write(p.join("README"), "dirty").unwrap();
        assert!(rpc_at(&update, &root).is_err());
        assert_eq!(std::fs::read_to_string(p.join("README")).unwrap(), "dirty");
        std::fs::write(p.join("README"), "one").unwrap();
        assert_eq!(rpc_at(&update, &root).unwrap()["state"], "updated");
        assert!(rpc_at(&update, &root).is_err()); // stale expected HEAD cannot be silently reused
        commit(&p, "local");
        commit(&source, "remote");
        git(&p, &["fetch", "origin"]).unwrap();
        let expected = repo(&p).unwrap();
        let head = expected["head"].clone();
        assert!(
            rpc_at(
                &json!({"op":"git_update_apply","path":p,"expected":expected}),
                &root
            )
            .is_err()
        );
        assert_eq!(repo(&p).unwrap()["head"], head);
        let group = rpc_at(&json!({"op":"git_update_plan","path":root}), &root).unwrap();
        assert_eq!(group["repositories"].as_array().unwrap().len(), 2);
        assert!(rpc_at(&json!({"op":"git_update_plan","path":"/"}), &root).is_err());
        std::fs::remove_dir_all(root).unwrap();
    }
}
