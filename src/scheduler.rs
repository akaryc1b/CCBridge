//! Per-account conflict arbitration. Registry lock is held only while claiming/releasing.
use crate::{common::*, runner::Task};
use anyhow::{Result, bail};
use serde_json::{Value, json};
use std::{
    fs::{File, OpenOptions},
    os::fd::AsRawFd,
    path::{Path, PathBuf},
};
pub fn registry() -> Result<File> {
    let f = lock(&runner_dir()?.join("registry.lock"), false)?
        .ok_or_else(|| anyhow::anyhow!("无法取得调度锁"))?;
    let root = runner_dir()?;
    let index = root.join("active");
    private_dir(&index)?;
    if !root.join("active-index-v1").exists() {
        for entry in std::fs::read_dir(&root)? {
            let entry = entry?;
            let id = entry.file_name();
            if id.to_str().is_some_and(|id| valid_id(id).is_ok()) {
                let p = entry.path().join("task.json");
                if !p.exists() {
                    continue;
                }
                let v: Value = read_json(&p).map_err(|e| {
                    e.context(format!("任务记录损坏，需核实后恢复：{}", p.display()))
                })?;
                if !["completed", "failed", "cancelled", "interrupted"]
                    .contains(&v["state"].as_str().unwrap_or(""))
                    || v["ownership_unknown"] == true
                {
                    std::fs::write(index.join(id), b"")?;
                }
            }
        }
        std::fs::write(root.join("active-index-v1"), b"ready")?;
    }
    Ok(f)
}
pub fn remember(t: &Task) -> Result<()> {
    let d = runner_dir()?.join("active");
    private_dir(&d)?;
    let p = d.join(&t.id);
    if pending(t) {
        std::fs::write(p, b"")?;
    } else if p.exists() {
        std::fs::remove_file(p)?;
    }
    Ok(())
}
pub fn next_sequence() -> Result<u64> {
    let p = runner_dir()?.join("sequence.json");
    let seq = if p.exists() {
        read_json::<u64>(&p)?
    } else {
        tasks()?.iter().map(|t| t.seq).max().unwrap_or(0)
    };
    let n = seq
        .checked_add(1)
        .ok_or_else(|| anyhow::anyhow!("队列序号耗尽"))?;
    write_json(&p, &n)?;
    Ok(n)
}
fn lock(p: &Path, nonblock: bool) -> Result<Option<File>> {
    let f = OpenOptions::new()
        .create(true)
        .truncate(false)
        .read(true)
        .write(true)
        .open(p)?;
    if unsafe {
        libc::flock(
            f.as_raw_fd(),
            libc::LOCK_EX | if nonblock { libc::LOCK_NB } else { 0 },
        )
    } == 0
    {
        Ok(Some(f))
    } else {
        let e = std::io::Error::last_os_error();
        if nonblock && e.kind() == std::io::ErrorKind::WouldBlock {
            Ok(None)
        } else {
            Err(e.into())
        }
    }
}
pub fn worker_lock(id: &str) -> Result<Option<File>> {
    lock(&task_dir(id)?.join("worker.lock"), true)
}
pub fn overlaps(a: &Path, b: &Path) -> bool {
    a.starts_with(b) || b.starts_with(a)
}
pub fn tasks() -> Result<Vec<Task>> {
    let mut tasks: Vec<Task> = vec![];
    let root = runner_dir()?;
    let index = root.join("active");
    if !index.exists() {
        return Ok(tasks);
    }
    for e in std::fs::read_dir(index)? {
        let e = e?;
        let id = e.file_name();
        if id.to_str().is_some_and(|id| valid_id(id).is_ok()) {
            let p = root.join(&id).join("task.json");
            let v: Value = read_json(&p)?;
            if ["completed", "cancelled", "failed", "interrupted"]
                .contains(&v["state"].as_str().unwrap_or(""))
                && v["ownership_unknown"] != true
            {
                continue;
            }
            let mut t: Task = serde_json::from_value(v.clone())
                .map_err(|e| anyhow::anyhow!("活动任务记录需要恢复（{}）：{e}", p.display()))?;
            t.id = id.to_string_lossy().into_owned();
            if !["queued", "running", "waiting_approval"].contains(&t.state.as_str()) {
                t.ownership_unknown = true;
            }
            if t.workspace.is_empty() {
                bail!("活动任务缺少目录，需核实后恢复：{}", p.display())
            }
            tasks.push(t);
        }
    }
    tasks.sort_by_key(|t| (t.seq, t.created_at, t.id.clone()));
    Ok(tasks)
}
pub fn holds(t: &Task) -> bool {
    t.state == "running" || t.ownership_unknown || (t.state == "queued" && t.phase != "waiting")
}
pub fn pending(t: &Task) -> bool {
    ["queued", "running"].contains(&t.state.as_str()) || t.ownership_unknown
}
pub fn conflicts(a: &str, b: &str) -> bool {
    a == "exclusive" || b == "exclusive" || (a != "analysis" && b != "analysis")
}
// A finished operation can remove its record after read_dir returned its entry.
// Only that expected disappearance is ignored; corrupt/unreadable records still block.
fn operation_record(path: &Path) -> Result<Option<Value>> {
    match read_json(path) {
        Ok(v) => Ok(Some(v)),
        Err(e)
            if e.downcast_ref::<std::io::Error>()
                .is_some_and(|e| e.kind() == std::io::ErrorKind::NotFound) =>
        {
            Ok(None)
        }
        Err(e) => Err(e),
    }
}
pub fn blockers(path: &Path, except: &str) -> Result<Vec<Value>> {
    blockers_for(path, except, "exclusive")
}
pub fn blockers_for(path: &Path, except: &str, access: &str) -> Result<Vec<Value>> {
    let mut found = vec![];
    for t in tasks()? {
        if t.id != except
            && holds(&t)
            && conflicts(access, &t.access)
            && overlaps(path, Path::new(&t.workspace))
        {
            found.push(json!({"kind":"task","id":t.id,"title":t.prompt.chars().take(80).collect::<String>(),"state":t.state,"path":t.workspace,"ownership_unknown":t.ownership_unknown}));
        }
    }
    let dir = runner_dir()?.join("operations");
    private_dir(&dir)?;
    for e in std::fs::read_dir(dir)? {
        let e = e?;
        if e.path().extension().is_some_and(|e| e == "json") {
            let Some(v) = operation_record(&e.path())? else {
                continue;
            };
            let lp = e.path().with_extension("lock");
            if (v["persistent"] == true || lock(&lp, true)?.is_none())
                && v["paths"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .any(|p| p.as_str().is_some_and(|p| overlaps(path, Path::new(p))))
            {
                found.push(v);
            }
        }
    }
    Ok(found)
}
pub fn busy(blockers: Vec<Value>) -> anyhow::Error {
    let title = blockers
        .first()
        .and_then(|b| b["title"].as_str())
        .unwrap_or("其他操作");
    crate::common::CodedError {
        code: "workspace_busy".into(),
        message: format!("任务「{title}」正在使用此目录，结束后再试"),
        blockers: Some(json!(blockers)),
    }
    .into()
}
pub fn queue(t: &Task) -> Result<Value> {
    if t.state != "queued" || t.phase != "waiting" {
        return Ok(Value::Null);
    }
    let mut blocked = blockers_for(Path::new(&t.workspace), &t.id, &t.access)?;
    let ahead: Vec<_> = tasks()?
        .into_iter()
        .take_while(|x| x.id != t.id)
        .filter(|x| {
            x.state == "queued"
                && x.phase == "waiting"
                && conflicts(&t.access, &x.access)
                && overlaps(Path::new(&t.workspace), Path::new(&x.workspace))
        })
        .collect();
    for a in &ahead {
        blocked.push(json!({"kind":"task","id":a.id,"title":a.prompt.chars().take(80).collect::<String>(),"state":"queued","path":a.workspace}));
    }
    Ok(
        json!({"position":ahead.len()+1,"ahead":blocked.len(),"blocked_by":blocked,"since":t.created_at}),
    )
}
pub struct Operation {
    path: PathBuf,
    _file: File,
}
impl Drop for Operation {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.path);
        let _ = std::fs::remove_file(self.path.with_extension("lock"));
    }
}
pub fn operation(paths: &[PathBuf], label: &str) -> Result<Operation> {
    let _guard = registry()?;
    for p in paths {
        let b = blockers(p, "")?;
        if !b.is_empty() {
            return Err(busy(b));
        }
    }
    let dir = runner_dir()?.join("operations");
    private_dir(&dir)?;
    let p = dir.join(format!("{}.json", uuid::Uuid::new_v4()));
    let f = lock(&p.with_extension("lock"), false)?.unwrap();
    write_json(
        &p,
        &json!({"kind":"operation","title":label,"state":"running","paths":paths}),
    )?;
    Ok(Operation { path: p, _file: f })
}
pub fn check_access(v: &Value) -> Result<()> {
    if v["access"]
        .as_str()
        .is_some_and(|s| !["write", "analysis"].contains(&s))
    {
        return Err(coded(
            "unsupported_access",
            "任务类型必须为 analysis 或 write",
        ));
    };
    Ok(())
}

pub fn begin(path: &Path, label: &str, token: Option<&str>) -> Result<Value> {
    let _g = registry()?;
    let id = token
        .map(str::to_owned)
        .unwrap_or_else(|| uuid::Uuid::new_v4().to_string());
    valid_id(&id)?;
    let dir = runner_dir()?.join("operations");
    private_dir(&dir)?;
    if dir.join(format!("{id}.json")).exists() {
        verify(&id, path)?;
        return Ok(json!({"lease_token":id}));
    }
    let b = blockers(path, "")?;
    if !b.is_empty() {
        return Err(busy(b));
    }
    write_json(
        &dir.join(format!("{id}.json")),
        &json!({"id":id,"kind":"operation","title":label,"state":"running","paths":[path],"persistent":true}),
    )?;
    Ok(json!({"lease_token":id}))
}
pub fn verify(token: &str, path: &Path) -> Result<()> {
    valid_id(token)?;
    let v: Value = read_json(
        &runner_dir()?
            .join("operations")
            .join(format!("{token}.json")),
    )?;
    if v["persistent"] != true || v["paths"] != json!([path]) {
        bail!("操作租约与目录不匹配")
    };
    Ok(())
}
pub fn end(token: &str, path: &Path) -> Result<()> {
    let _g = registry()?;
    valid_id(token)?;
    if !runner_dir()?
        .join("operations")
        .join(format!("{token}.json"))
        .exists()
    {
        return Ok(());
    }
    verify(token, path)?;
    let _active = lock(
        &runner_dir()?
            .join("operations")
            .join(format!("{token}.lock")),
        true,
    )?
    .ok_or_else(|| coded("operation_busy", "操作仍在执行，不能释放"))?;
    std::fs::remove_file(
        runner_dir()?
            .join("operations")
            .join(format!("{token}.json")),
    )?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn access_conflicts_are_typed() {
        assert!(!conflicts("analysis", "analysis"));
        assert!(!conflicts("write", "analysis"));
        assert!(conflicts("write", "write"));
        assert!(conflicts("analysis", "exclusive"));
    }
    #[test]
    fn path_conflicts_are_component_based() {
        assert!(overlaps(Path::new("/a"), Path::new("/a/b")));
        assert!(overlaps(Path::new("/a/b"), Path::new("/a")));
        assert!(!overlaps(Path::new("/a"), Path::new("/ab")));
    }
    #[test]
    fn analysis_is_a_supported_explicit_scope() {
        assert!(check_access(&json!({"access":"analysis"})).is_ok());
        assert!(check_access(&json!({})).is_ok());
    }
}

pub fn use_lease(token: &str, path: &Path) -> Result<File> {
    let _g = registry()?;
    verify(token, path)?;
    lock(
        &runner_dir()?
            .join("operations")
            .join(format!("{token}.lock")),
        true,
    )?
    .ok_or_else(|| coded("operation_busy", "更新操作仍在执行"))
}

#[cfg(test)]
mod operation_record_tests {
    use super::*;
    #[test]
    fn completed_record_disappearance_is_normal_but_corruption_is_not() {
        let p = std::env::temp_dir().join(format!(
            "ccbridge-operation-record-{}.json",
            uuid::Uuid::new_v4()
        ));
        std::fs::write(&p, b"{\"state\":\"running\"}").unwrap();
        assert!(operation_record(&p).unwrap().is_some());
        std::fs::remove_file(&p).unwrap();
        assert!(operation_record(&p).unwrap().is_none());
        std::fs::write(&p, b"{bad-json").unwrap();
        assert!(operation_record(&p).is_err());
        std::fs::remove_file(p).unwrap();
    }
}
