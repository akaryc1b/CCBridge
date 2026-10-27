//! Account/connection-scoped snapshots for long App and MCP operations.
use serde_json::{Value, json};
use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};
#[derive(Clone, Default)]
pub struct Store {
    inner: Arc<Mutex<BTreeMap<String, Value>>>,
    history: Option<std::path::PathBuf>,
    history_error: Arc<Mutex<Option<String>>>,
}
impl Store {
    pub fn lock(
        &self,
    ) -> std::sync::LockResult<std::sync::MutexGuard<'_, BTreeMap<String, Value>>> {
        self.inner.lock()
    }
}
#[derive(Clone)]
pub struct Operation {
    pub(crate) store: Store,
    pub id: String,
}
fn millis() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}
impl Operation {
    pub fn begin(
        store: Store,
        id: String,
        kind: &str,
        account: &str,
        connection: &str,
        path: &str,
        human: bool,
    ) -> Self {
        let mut map = store.lock().unwrap();
        if map.len() >= 100 {
            if let Some(k) = map
                .iter()
                .filter(|(_, v)| v["state"] != "running" && v["state"] != "unknown")
                .min_by_key(|(_, v)| v["started_at_ms"].as_u64().unwrap_or(0))
                .map(|(k, _)| k.clone())
            {
                map.remove(&k);
            }
        }
        map.insert(id.clone(),json!({"id":id,"kind":kind,"account":account,"connection_id":connection,"path":path,"source":if human{"app"}else{"mcp"},"state":"running","started_at_ms":millis(),"updated_at_ms":millis(),"revision":0,"phase":"checking","processed":0,"total":0,"items":[]}));
        drop(map);
        persist(&store);
        Self { store, id }
    }
    pub fn patch(&self, p: Value) {
        let mut m = self.store.lock().unwrap();
        if let Some(v) = m.get_mut(&self.id) {
            for (k, x) in p.as_object().unwrap() {
                v[k] = x.clone();
            }
            v["updated_at_ms"] = json!(millis());
            v["revision"] = json!(v["revision"].as_u64().unwrap_or(0) + 1);
        }
        drop(m);
        if p.get("phase").is_some() || p.get("state").is_some() || p.get("verified_at").is_some() {
            persist(&self.store);
        }
    }
    pub fn item(&self, index: usize, result: Value) {
        let mut m = self.store.lock().unwrap();
        if let Some(v) = m.get_mut(&self.id) {
            if let Some(a) = v["items"].as_array_mut() {
                if index < a.len() {
                    a[index] = result;
                }
            }
            v["processed"] = json!(index + 1);
            v["updated_at_ms"] = json!(millis());
        }
    }
    pub fn finish(&self, result: &anyhow::Result<Value>) {
        match result {
            Ok(r) => {
                let failed =
                    r["failed"]
                        .as_u64()
                        .unwrap_or(if r["complete"] == false { 1 } else { 0 });
                let n = r["results"].as_array().map_or(0, Vec::len);
                let busy_only = r["results"].as_array().is_some_and(|a| {
                    !a.is_empty() && a.iter().all(|x| x["code"] == "workspace_busy")
                });
                self.patch(json!({"state":if busy_only{"blocked"}else if failed==0{"done"}else if n>failed as usize||r["repositories"].as_array().is_some_and(|a|!a.is_empty()){"partial"}else{"failed"},"result":r,"finished_at_ms":millis()}));
            }
            Err(e) => {
                let m = crate::common::error_summary(&format!("{e:#}"));
                self.patch(json!({"state":if m.contains("超时")||m.contains("不确定")||m.contains("断开"){"unknown"}else{"failed"},"error":m,"finished_at_ms":millis()}));
            }
        }
        persist(&self.store);
    }
}
pub fn snapshot(
    store: &Store,
    account: &str,
    connection: &str,
    id: Option<&str>,
    kind: Option<&str>,
    path: Option<&str>,
) -> Value {
    let m = store.lock().unwrap();
    let matches = |v: &&Value| {
        v["account"] == account
            && v["connection_id"] == connection
            && kind.is_none_or(|k| v["kind"] == k)
            && path.is_none_or(|p| v["path"] == p)
    };
    if let Some(id) = id {
        return m
            .get(id)
            .filter(|v| matches(v))
            .cloned()
            .unwrap_or(Value::Null);
    }
    m.values()
        .filter(matches)
        .max_by_key(|v| v["started_at_ms"].as_u64().unwrap_or(0))
        .cloned()
        .unwrap_or(Value::Null)
}
/// Numeric Git output only: no raw stderr/URLs/credentials enter progress.
pub fn git_progress(line: &str) -> Option<Value> {
    let (phase, rest) = if let Some(s) = line.strip_prefix("Receiving objects:") {
        ("receiving", s)
    } else if let Some(s) = line.strip_prefix("Resolving deltas:") {
        ("resolving", s)
    } else {
        return None;
    };
    let n = rest.split_once('(')?.1.split_once(')')?.0;
    let (a, b) = n.split_once('/')?;
    let (a, b) = (a.trim().parse::<u64>().ok()?, b.trim().parse::<u64>().ok()?);
    if b == 0 || a > b {
        return None;
    }
    Some(json!({"git_phase":phase,"objects":{"received":a,"total":b}}))
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn scoped_and_terminal() {
        let s = Store::default();
        let o = Operation::begin(s.clone(), "a".into(), "git_update", "u", "c", "/p", true);
        o.patch(json!({"total":3}));
        assert!(snapshot(&s, "other", "c", Some("a"), None, None).is_null());
        assert!(snapshot(&s, "u", "new", Some("a"), None, None).is_null());
        o.finish(&Ok(json!({"failed":1,"results":[{},{}]})));
        assert_eq!(
            snapshot(&s, "u", "c", Some("a"), None, None)["state"],
            "partial"
        );
    }
    #[test]
    fn real_git_numbers() {
        assert_eq!(
            git_progress("Receiving objects: 45% (450/1000), 1.20 MiB").unwrap()["objects"]["received"],
            450
        );
        assert!(git_progress("Receiving objects: 5% (5/0)").is_none());
        assert!(git_progress("fatal: https://secret@host").is_none());
    }
}

/// Account scoped history; old connection states are observations, never replay instructions.
pub fn list(store: &Store, account: &str, connection: &str) -> Value {
    let map = store.lock().unwrap();
    let mut values: Vec<Value> = map
        .values()
        .filter(|v| v["account"] == account)
        .cloned()
        .map(|mut v| {
            v.as_object_mut().unwrap().remove("result");
            v.as_object_mut().unwrap().remove("items");
            if v["connection_id"] != connection && v["state"] == "running" {
                v["state"] = json!("unknown");
            }
            v
        })
        .collect();
    values.sort_by_key(|v| std::cmp::Reverse(v["started_at_ms"].as_u64().unwrap_or(0)));
    json!({"operations":values,"history_error":store.history_error.lock().unwrap().clone()})
}

/// Persist only a small allow-list. Never serialize request/result/items/stderr.
fn persist(store: &Store) {
    let Some(path) = &store.history else { return };
    let map = store.lock().unwrap();
    let rows: Vec<Value> = map
        .values()
        .map(|v| {
            let mut out = json!({});
            for k in [
                "id",
                "kind",
                "account",
                "connection_id",
                "path",
                "source",
                "state",
                "started_at_ms",
                "finished_at_ms",
                "updated_at_ms",
                "revision",
                "phase",
                "processed",
                "total",
                "verified_at",
                "verification",
            ] {
                if !v[k].is_null() {
                    out[k] = v[k].clone();
                }
            }
            out
        })
        .collect();
    *store.history_error.lock().unwrap() = crate::common::write_json(path, &rows)
        .err()
        .map(|_| "操作历史未保存，重启后可能无法找回".to_string());
}
pub fn restore() -> Store {
    let Ok(root) = crate::common::app_dir() else {
        return Store::default();
    };
    restore_at(root.join("operation-history.json"))
}
fn restore_at(path: std::path::PathBuf) -> Store {
    let store = Store {
        history: Some(path.clone()),
        ..Store::default()
    };
    {
        if let Ok(rows) = crate::common::read_json::<Vec<Value>>(&path) {
            let mut map = store.lock().unwrap();
            for mut row in rows {
                if let Some(id) = row["id"]
                    .as_str()
                    .filter(|id| uuid::Uuid::parse_str(id).is_ok())
                    .map(str::to_owned)
                {
                    if row["state"] == "running" {
                        row["state"] = json!("unknown");
                    }
                    map.insert(id, row);
                }
            }
        }
    }
    store
}

#[cfg(test)]
mod history_tests {
    use super::*;
    #[test]
    fn private_history_has_no_secret_payload_and_never_replays() {
        let dir = std::env::temp_dir().join(uuid::Uuid::new_v4().to_string());
        crate::common::private_dir(&dir).unwrap();
        let path = dir.join("history.json");
        let store = restore_at(path.clone());
        let id = uuid::Uuid::new_v4().to_string();
        let op = Operation::begin(
            store.clone(),
            id.clone(),
            "git_vps_auth_install",
            "a",
            "c",
            "/home/a",
            true,
        );
        op.patch(json!({"phase":"syncing","request":{"secret":"NEVER_PERSIST"},"items":[{"secret":"NEVER_PERSIST"}]}));
        let text = std::fs::read_to_string(&path).unwrap();
        assert!(!text.contains("NEVER_PERSIST"));
        let restored = restore_at(path);
        assert_eq!(restored.lock().unwrap()[&id]["state"], "unknown");
        assert_eq!(
            list(&restored, "other", "c")["operations"]
                .as_array()
                .unwrap()
                .len(),
            0
        );
        assert_eq!(
            list(&restored, "a", "new")["operations"][0]["state"],
            "unknown"
        );
        std::fs::remove_dir_all(dir).unwrap();
    }
}
