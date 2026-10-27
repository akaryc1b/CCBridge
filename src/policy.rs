//! Human-controlled account policy. It applies to CCBridge, not arbitrary SSH programs.
use crate::common::*;
use anyhow::Result;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
#[derive(Default, Serialize, Deserialize)]
pub struct Policy {
    pub read_only: bool,
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
        json!({"read_only":p.read_only,"revision":p.revision,"updated_at":p.updated_at,
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
    p.read_only = v["read_only"]
        .as_bool()
        .ok_or_else(|| coded("invalid_policy", "缺少保护开关状态"))?;
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
