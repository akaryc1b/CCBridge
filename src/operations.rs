//! Account/connection-scoped snapshots for long App and MCP operations.
use serde_json::{Value, json};
use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};
pub type Store = Arc<Mutex<BTreeMap<String, Value>>>;
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
                .filter(|(_, v)| v["state"] != "running")
                .min_by_key(|(_, v)| v["started_at_ms"].as_u64().unwrap_or(0))
                .map(|(k, _)| k.clone())
            {
                map.remove(&k);
            }
        }
        map.insert(id.clone(),json!({"id":id,"kind":kind,"account":account,"connection_id":connection,"path":path,"source":if human{"app"}else{"mcp"},"state":"running","started_at_ms":millis(),"updated_at_ms":millis(),"revision":0,"phase":"checking","processed":0,"total":0,"items":[]}));
        drop(map);
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
                self.patch(json!({"state":if failed==0{"done"}else if n>failed as usize||r["repositories"].as_array().is_some_and(|a|!a.is_empty()){"partial"}else{"failed"},"result":r,"finished_at_ms":millis()}));
            }
            Err(e) => {
                let m = crate::common::error_summary(&format!("{e:#}"));
                self.patch(json!({"state":if m.contains("超时")||m.contains("不确定")||m.contains("断开"){"unknown"}else{"failed"},"error":m,"finished_at_ms":millis()}));
            }
        }
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
