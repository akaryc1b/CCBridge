//! Human-controlled account policy. It applies to CCBridge, not arbitrary SSH programs.
use crate::common::*;
use anyhow::Result;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
#[derive(Default, Serialize, Deserialize)]
pub struct Policy {
    pub read_only: bool,
    #[serde(default)]
    pub writable_workspaces: Vec<String>,
    pub revision: u64,
    pub updated_at: u64,
}
pub fn read() -> Result<Policy> {
    let path = runner_dir()?.join("policy.json");
    if path.exists() {
        read_json(&path)
    } else {
        Ok(Policy::default())
    }
}
pub fn protected(policy: &Policy, workspace: &std::path::Path) -> bool {
    policy.read_only
        && !policy.writable_workspaces.iter().any(|root| {
            // Re-resolve both ends: replacement symlinks must not broaden a saved exception.
            std::fs::canonicalize(root)
                .ok()
                .filter(|p| p.to_string_lossy() == root.as_str())
                .zip(workspace.canonicalize().ok())
                .is_some_and(|(root, path)| path.starts_with(root))
        })
}
pub fn workspace_scope(
    policy: &Policy,
    workspace: &std::path::Path,
    requested: Option<&str>,
    parent: Option<&str>,
) -> Result<String> {
    let effective = Policy {
        read_only: protected(policy, workspace),
        ..Policy::default()
    };
    scope(&effective, requested, parent)
}
pub fn scope(policy: &Policy, requested: Option<&str>, parent: Option<&str>) -> Result<String> {
    let requested = requested.filter(|s| !s.is_empty());
    let effective = requested.unwrap_or_else(|| {
        if policy.read_only {
            "analysis"
        } else {
            parent.unwrap_or("write")
        }
    });
    if !["write", "analysis"].contains(&effective) {
        return Err(coded("invalid_access", "任务类型必须为 analysis 或 write"));
    }
    if policy.read_only && effective == "write" {
        return Err(coded(
            "policy_blocked",
            "此账号已禁止 AI 修改项目文件，请使用分析任务",
        ));
    }
    Ok(effective.into())
}
pub fn get() -> Result<Value> {
    let _g = crate::scheduler::registry()?;
    let p = read()?;
    let tasks = crate::scheduler::tasks()?;
    Ok(
        json!({"read_only":p.read_only,"revision":p.revision,"updated_at":p.updated_at,"writable_workspaces":p.writable_workspaces,
        "running_writes": tasks.iter().filter(|t| t.access != "analysis" && t.state=="running" && !t.ownership_unknown).count(),
        "unknown_writes":tasks.iter().filter(|t|t.access!="analysis"&&t.ownership_unknown).count(),
        "queued_writes":tasks.iter().filter(|t| t.access != "analysis" && t.state=="queued" && t.phase=="waiting").count()}),
    )
}
pub fn set(v: &Value) -> Result<Value> {
    if v["origin"] != "app" || v["confirmed"] != true {
        return Err(coded("human_required", "修改项目保护只能由人在 App 确认"));
    }
    let _g = crate::scheduler::registry()?;
    let mut p = read()?;
    if v["expected_revision"].as_u64() != Some(p.revision) {
        return Err(coded("stale_policy", "保护设置已变化，请刷新后重试"));
    }
    if v["workspace"].is_string() {
        let raw = field(v, "workspace")?;
        let path = std::fs::canonicalize(raw)?;
        if !path.is_dir() || !path.starts_with(home()?.canonicalize()?) {
            return Err(coded("invalid_policy", "请选择当前账号HOME下的工作区目录"));
        }
        let enabled = v["write_enabled"]
            .as_bool()
            .ok_or_else(|| coded("invalid_policy", "缺少工作区写入设置"))?;
        let key = path.to_string_lossy().to_string();
        p.writable_workspaces.retain(|p| p != &key);
        if enabled {
            p.writable_workspaces.push(key);
        }
    } else {
        p.read_only = v["read_only"]
            .as_bool()
            .ok_or_else(|| coded("invalid_policy", "缺少保护开关状态"))?;
    }
    p.revision = p
        .revision
        .checked_add(1)
        .ok_or_else(|| coded("invalid_policy", "保护设置版本已超出范围"))?;
    p.updated_at = now();
    write_json(&runner_dir()?.join("policy.json"), &p)?;
    Ok(serde_json::to_value(p)?)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn exception_applies_only_inside_canonical_workspace() {
        let d = std::env::temp_dir().join(uuid::Uuid::new_v4().to_string());
        std::fs::create_dir_all(d.join("allowed/repo")).unwrap();
        std::fs::create_dir(d.join("other")).unwrap();
        let d = d.canonicalize().unwrap();
        let p = Policy {
            read_only: true,
            writable_workspaces: vec![d.join("allowed").to_string_lossy().into()],
            ..Default::default()
        };
        assert!(!protected(&p, &d.join("allowed/repo")));
        assert!(protected(&p, &d.join("other")));
        assert!(workspace_scope(&p, &d.join("other"), Some("write"), None).is_err());
        assert_eq!(
            workspace_scope(&p, &d.join("allowed"), None, Some("analysis")).unwrap(),
            "analysis"
        );
        assert_eq!(
            workspace_scope(&p, &d.join("allowed"), Some("write"), Some("analysis")).unwrap(),
            "write"
        );
        std::os::unix::fs::symlink(d.join("other"), d.join("allowed/link")).unwrap();
        assert!(protected(&p, &d.join("allowed/link")));
        std::fs::remove_dir_all(d).unwrap();
    }
    #[test]
    fn scope_preserves_analysis_reply_and_blocks_explicit_write() {
        let p = Policy::default();
        assert_eq!(scope(&p, None, None).unwrap(), "write");
        assert_eq!(scope(&p, None, Some("analysis")).unwrap(), "analysis");
        let p = Policy {
            read_only: true,
            ..Default::default()
        };
        assert_eq!(scope(&p, None, Some("write")).unwrap(), "analysis");
        assert!(scope(&p, Some("write"), None).is_err());
        assert!(scope(&p, Some("unknown"), None).is_err());
    }
}
