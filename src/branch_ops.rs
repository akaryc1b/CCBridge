//! Branch inspection and conservative human-owned changes. Called under workspace leases.
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
        .output()?;
    if !out.status.success() {
        bail!("Git 操作未完成：请刷新分支状态后重试；未强制覆盖改动")
    }
    Ok(String::from_utf8(out.stdout)?.trim().into())
}
fn valid(p: &Path, name: &str) -> Result<()> {
    if name.starts_with('-') {
        bail!("分支名称无效")
    }
    git(p, &["check-ref-format", "--branch", name])?;
    Ok(())
}
fn oid(p: &Path, name: &str) -> Result<String> {
    git(
        p,
        &[
            "show-ref",
            "--verify",
            "--hash",
            &format!("refs/heads/{name}"),
        ],
    )
}
fn merged(p: &Path, commit: &str, base: &str) -> bool {
    Command::new("git")
        .arg("-C")
        .arg(p)
        .args(["merge-base", "--is-ancestor", commit, base])
        .status()
        .is_ok_and(|s| s.success())
}
pub fn inspect(p: &Path, v: &Value) -> Result<Value> {
    let repo = crate::repository::inspect(p)?;
    let names = git(
        p,
        &["for-each-ref", "--format=%(refname:short)", "refs/heads"],
    )?;
    let remotes = git(
        p,
        &[
            "for-each-ref",
            "--format=%(refname) %(symref)",
            "refs/remotes",
        ],
    )?;
    let default = git(p, &["symbolic-ref", "--short", "refs/remotes/origin/HEAD"])
        .ok()
        .and_then(|s| s.strip_prefix("origin/").map(String::from))
        .or_else(|| {
            ["main", "master"]
                .iter()
                .find(|n| names.lines().any(|s| s == **n))
                .map(|s| s.to_string())
        })
        .unwrap_or_else(|| repo.branch.clone());
    let base = v["base"]
        .as_str()
        .filter(|s| !s.is_empty())
        .unwrap_or(&default);
    valid(p, base)?;
    let base_oid = oid(p, base).context("合并目标不存在，请刷新分支列表")?;
    let worktrees = git(p, &["worktree", "list", "--porcelain"])?;
    let protected =
        git(p, &["config", "--get-all", "ccbridge.protectedBranch"]).unwrap_or_default();
    let blockers: Vec<_> = crate::scheduler::blockers(p, "")?
        .into_iter()
        .filter(|b| b["kind"] == "task")
        .collect();
    let changed_files = if repo.dirty {
        git(p, &["status", "--short"])?
            .lines()
            .take(50)
            .map(String::from)
            .collect::<Vec<_>>()
    } else {
        vec![]
    };
    let mut details = vec![];
    for n in names.lines() {
        let commit = oid(p, n)?;
        let in_use = worktrees
            .lines()
            .any(|l| l == format!("branch refs/heads/{n}"));
        let reason = if n == repo.branch {
            "当前分支"
        } else if n == default
            || ["main", "master"].contains(&n)
            || n == base
            || protected.lines().any(|s| s == n)
        {
            "受保护分支"
        } else if !blockers.is_empty() {
            "任务使用中"
        } else if in_use {
            "工作树使用中"
        } else if !merged(p, &commit, &base_oid) {
            "未合并"
        } else {
            ""
        };
        let upstream = git(
            p,
            &[
                "for-each-ref",
                "--format=%(upstream:short)",
                &format!("refs/heads/{n}"),
            ],
        )?;
        details.push(json!({"name":n,"oid":commit,"upstream":upstream,"merged":merged(p,&commit,&base_oid),"reason":reason,"deletable":reason.is_empty()}));
    }
    Ok(
        json!({"path":p,"current":repo.branch,"dirty":repo.dirty,"changed_files":changed_files,"branches":names.lines().collect::<Vec<_>>(),"remote_branches":remotes.lines().filter_map(|line| { let mut fields=line.split_whitespace(); let reference=fields.next()?; if fields.next().is_some() { return None; } reference.strip_prefix("refs/remotes/") }).collect::<Vec<_>>(),"details":details,"base":base,"base_oid":base_oid,"default_branch":default}),
    )
}
pub fn change(p: &Path, v: &Value) -> Result<Option<Value>> {
    let action = v["action"].as_str().unwrap_or("");
    if !["delete", "clean", "remote_delete_plan"].contains(&action) {
        return Ok(None);
    }
    let state = inspect(p, v)?;
    if v["base_oid"] != state["base_oid"] {
        bail!("合并目标已变化，请刷新后重新选择")
    }
    let selections = v["selections"].as_array().context("请选择分支")?;
    if selections.is_empty() || selections.len() > 100 {
        bail!("每次请选择 1 至 100 个分支")
    }
    for s in selections {
        let n = s["name"].as_str().context("分支名称缺失")?;
        let d = state["details"]
            .as_array()
            .unwrap()
            .iter()
            .find(|d| d["name"] == n)
            .context("分支已不存在，请刷新")?;
        if d["oid"] != s["oid"] {
            bail!("分支 {n} 已变化，请刷新后重试")
        }
        if d["deletable"] != true {
            bail!(
                "不能删除 {n}：{}",
                d["reason"].as_str().unwrap_or("状态未知")
            )
        }
    }
    if action == "remote_delete_plan" {
        if selections.len() != 1 {
            bail!("每次只能删除一个远端分支")
        }
        let n = selections[0]["name"].as_str().unwrap();
        let remote = v["remote"].as_str().context("请选择远端")?;
        if remote.starts_with('-') || remote.contains(['/', '\n', '\r', '\0']) {
            bail!("远端名称无效")
        }
        let reference = format!("refs/remotes/{remote}/{n}");
        let remote_oid = git(p, &["show-ref", "--verify", "--hash", &reference])?;
        if !merged(p, &remote_oid, state["base_oid"].as_str().unwrap()) {
            bail!("远端分支尚未合并，已保留")
        }
        return Ok(Some(
            json!({"remote":remote,"branch":n,"oid":remote_oid,"local_oid":selections[0]["oid"],"base":state["base"],"base_oid":state["base_oid"]}),
        ));
    }
    let mut results = vec![];
    for s in selections {
        let n = s["name"].as_str().unwrap();
        // Re-read immediately before each delete. Git -d also refuses checked-out/unmerged refs.
        let result = (|| -> Result<()> {
            if oid(p, n)? != s["oid"].as_str().unwrap() {
                bail!("分支已变化，未删除")
            }
            git(p, &["branch", "-d", "--", n])?;
            Ok(())
        })();
        results.push(match result {
            Ok(()) => json!({"name":n,"ok":true}),
            Err(e) => json!({"name":n,"ok":false,"message":format!("{e:#}")}),
        });
    }
    let mut updated = inspect(p, v)?;
    let failed = results.iter().filter(|r| r["ok"] != true).count();
    updated["failed"] = json!(failed);
    updated["deleted"] = json!(results.len() - failed);
    updated["ok"] = json!(failed == 0);
    updated["results"] = json!(results);
    Ok(Some(updated))
}

#[cfg(test)]
mod tests {
    use super::*;
    fn fixture() -> std::path::PathBuf {
        let p = std::env::temp_dir().join(format!("ccbridge-branches-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir(&p).unwrap();
        git(&p, &["init", "-q", "--initial-branch=main"]).unwrap();
        git(&p, &["config", "user.name", "fixture"]).unwrap();
        git(&p, &["config", "user.email", "fixture@example.invalid"]).unwrap();
        std::fs::write(p.join("README"), "fixture").unwrap();
        git(&p, &["add", "README"]).unwrap();
        git(&p, &["commit", "-qm", "initial"]).unwrap();
        p
    }
    fn request(p: &Path, n: &str, action: &str) -> Value {
        let s = inspect(p, &json!({})).unwrap();
        json!({"action":action,"base":s["base"],"base_oid":s["base_oid"],"selections":[{"name":n,"oid":oid(p,n).unwrap()}]})
    }
    #[test]
    fn delete_and_clean_keep_unmerged_current_default_and_stale_refs() {
        let p = fixture();
        git(&p, &["branch", "feature/merged"]).unwrap();
        let r = request(&p, "feature/merged", "delete");
        let out = change(&p, &r).unwrap().unwrap();
        assert_eq!(out["results"][0]["ok"], true);
        assert!(oid(&p, "feature/merged").is_err());
        assert!(change(&p, &request(&p, "main", "delete")).is_err());
        git(&p, &["switch", "-c", "feature/unmerged"]).unwrap();
        std::fs::write(p.join("extra"), "unmerged").unwrap();
        git(&p, &["add", "extra"]).unwrap();
        git(&p, &["commit", "-qm", "unmerged"]).unwrap();
        git(&p, &["switch", "main"]).unwrap();
        assert!(change(&p, &request(&p, "feature/unmerged", "delete")).is_err());
        git(&p, &["branch", "feature/stale"]).unwrap();
        let stale = request(&p, "feature/stale", "delete");
        let newer = oid(&p, "feature/unmerged").unwrap();
        git(&p, &["update-ref", "refs/heads/feature/stale", &newer]).unwrap();
        assert!(change(&p, &stale).is_err());
        assert_eq!(oid(&p, "feature/stale").unwrap(), newer);
        git(&p, &["branch", "feature/a"]).unwrap();
        git(&p, &["branch", "feature/b"]).unwrap();
        let mut batch = request(&p, "feature/a", "clean");
        batch["selections"]
            .as_array_mut()
            .unwrap()
            .push(json!({"name":"feature/b","oid":oid(&p,"feature/b").unwrap()}));
        let result = change(&p, &batch).unwrap().unwrap();
        assert_eq!(result["results"].as_array().unwrap().len(), 2);
        assert!(
            result["results"]
                .as_array()
                .unwrap()
                .iter()
                .all(|r| r["ok"] == true)
        );
        std::fs::remove_dir_all(p).unwrap();
    }
    #[test]
    fn worktrees_protected_configuration_and_remote_plan_are_conservative() {
        let p = fixture();
        git(&p, &["branch", "feature/worktree"]).unwrap();
        let wt = p.with_extension("worktree");
        git(
            &p,
            &["worktree", "add", wt.to_str().unwrap(), "feature/worktree"],
        )
        .unwrap();
        assert!(change(&p, &request(&p, "feature/worktree", "delete")).is_err());
        git(&p, &["branch", "release/keep"]).unwrap();
        git(
            &p,
            &[
                "config",
                "--add",
                "ccbridge.protectedBranch",
                "release/keep",
            ],
        )
        .unwrap();
        assert!(change(&p, &request(&p, "release/keep", "delete")).is_err());
        git(&p, &["branch", "feature/remote"]).unwrap();
        let commit = oid(&p, "main").unwrap();
        git(
            &p,
            &["update-ref", "refs/remotes/origin/feature/remote", &commit],
        )
        .unwrap();
        git(&p, &["update-ref", "refs/remotes/origin/main", &commit]).unwrap();
        git(
            &p,
            &[
                "symbolic-ref",
                "refs/remotes/origin/HEAD",
                "refs/remotes/origin/main",
            ],
        )
        .unwrap();
        let listed = inspect(&p, &json!({})).unwrap();
        assert!(
            !listed["remote_branches"]
                .as_array()
                .unwrap()
                .iter()
                .any(|n| n == "origin" || n == "origin/HEAD")
        );
        assert!(
            listed["remote_branches"]
                .as_array()
                .unwrap()
                .iter()
                .any(|n| n == "origin/main")
        );
        let mut r = request(&p, "feature/remote", "remote_delete_plan");
        r["remote"] = json!("origin");
        let plan = change(&p, &r).unwrap().unwrap();
        assert_eq!(plan["oid"], commit);
        assert!(oid(&p, "feature/remote").is_ok()); // Plan cannot delete.
        r["base_oid"] = json!("0000000000000000000000000000000000000000");
        assert!(change(&p, &r).is_err());
        git(&p, &["worktree", "remove", wt.to_str().unwrap()]).unwrap();
        std::fs::remove_dir_all(p).unwrap();
    }
    #[test]
    fn git_delete_rejection_is_counted_as_failure_not_task_success() {
        let p = fixture();
        git(&p, &["branch", "old-upstream"]).unwrap();
        git(&p, &["switch", "-c", "feature/check-upstream"]).unwrap();
        std::fs::write(p.join("new"), "new commit").unwrap();
        git(&p, &["add", "new"]).unwrap();
        git(&p, &["commit", "-qm", "next"]).unwrap();
        git(&p, &["switch", "main"]).unwrap();
        git(&p, &["merge", "--ff-only", "feature/check-upstream"]).unwrap();
        git(&p, &["config", "branch.feature/check-upstream.remote", "."]).unwrap();
        git(
            &p,
            &[
                "config",
                "branch.feature/check-upstream.merge",
                "refs/heads/old-upstream",
            ],
        )
        .unwrap();
        let r = change(&p, &request(&p, "feature/check-upstream", "delete"))
            .unwrap()
            .unwrap();
        assert_eq!(r["failed"], 1);
        assert_eq!(r["ok"], false);
        assert_eq!(r["deleted"], 0);
        assert!(oid(&p, "feature/check-upstream").is_ok());
        std::fs::remove_dir_all(p).unwrap();
    }
}
