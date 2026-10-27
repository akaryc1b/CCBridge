//! Account-local session projections. Worker task files remain authoritative and untouched.
use crate::common::*;
use anyhow::{Result, bail};
use serde_json::{Value, json};
use std::{
    collections::BTreeMap,
    fs::{File, OpenOptions},
    path::Path,
};

fn guard(root: &Path) -> Result<File> {
    use std::os::fd::AsRawFd;
    private_dir(root)?;
    let f = OpenOptions::new()
        .create(true)
        .truncate(false)
        .write(true)
        .open(root.join("meta.lock"))?;
    if unsafe { libc::flock(f.as_raw_fd(), libc::LOCK_EX) } != 0 {
        return Err(std::io::Error::last_os_error().into());
    }
    Ok(f)
}
fn meta(root: &Path, id: &str) -> Result<Value> {
    let p = root.join(format!("{id}.json"));
    if p.exists() {
        read_json(&p)
    } else {
        Ok(json!({"schema":1,"session_id":id,"revision":0,"archived":false}))
    }
}
pub fn task_metadata(session: &str, first_prompt: &str) -> Value {
    let m = runner_dir()
        .ok()
        .and_then(|r| meta(&r.join("sessions"), session).ok())
        .unwrap_or(json!({}));
    json!({"title":m["title"].as_str().map(str::to_owned).unwrap_or_else(||title(first_prompt)),"archived":m["archived"]==true,"revision":m["revision"].as_u64().unwrap_or(0)})
}
fn title(text: &str) -> String {
    text.lines()
        .find(|l| !l.trim().is_empty())
        .unwrap_or("会话")
        .trim()
        .chars()
        .take(48)
        .collect()
}
/// Reads every legal task, never the legacy list's latest-100 window.
fn scan(root: &Path, q: &str) -> Result<(Vec<Value>, usize, Vec<Value>)> {
    let lower = q.to_lowercase();
    let mut tasks = vec![];
    let mut unreadable = 0;
    let mut diagnostics = vec![];
    for e in std::fs::read_dir(root)? {
        let e = e?;
        if valid_id(&e.file_name().to_string_lossy()).is_err() {
            continue;
        }
        if !e.file_type()?.is_dir() {
            unreadable += 1;
            if diagnostics.len() < 100 {
                diagnostics.push(json!({"task_id":e.file_name().to_string_lossy(),"reason":"记录不可读取或格式无效"}));
            }
            continue;
        }
        match read_json::<Value>(&e.path().join("task.json")) {
            Ok(mut t) => {
                if t["id"].as_str() != e.file_name().to_str() {
                    unreadable += 1;
                    if diagnostics.len() < 100 {
                        diagnostics.push(json!({"task_id":e.file_name().to_string_lossy(),"reason":"记录不可读取或格式无效"}));
                    }
                    continue;
                }
                let mut hits = vec![];
                if !q.is_empty() {
                    for (field, text) in [
                        ("prompt", t["prompt"].as_str().unwrap_or("")),
                        ("result", t["result"]["result"].as_str().unwrap_or("")),
                    ] {
                        if text.to_lowercase().contains(&lower) {
                            hits.push(
                                json!({"field":field,"task_id":t["id"],"snippet":snippet(text,q)}),
                            );
                        }
                    }
                }
                t["prompt"] = json!(title(t["prompt"].as_str().unwrap_or("")));
                t["search_hits"] = json!(hits);
                t["task_id"] = t["id"].clone();
                t.as_object_mut().unwrap().retain(|k, _| {
                    [
                        "id",
                        "task_id",
                        "session_id",
                        "workspace",
                        "prompt",
                        "state",
                        "mode",
                        "access",
                        "blocked_reason",
                        "phase",
                        "queue",
                        "ownership_unknown",
                        "created_at",
                        "updated_at",
                        "resume",
                        "reply_to",
                        "model_requested",
                        "model_resolved",
                        "search_hits",
                    ]
                    .contains(&k.as_str())
                });
                tasks.push(t);
            }
            Err(_) => {
                unreadable += 1;
                if diagnostics.len() < 100 {
                    diagnostics.push(json!({"task_id":e.file_name().to_string_lossy(),"reason":"记录不可读取或格式无效"}));
                }
            }
        }
    }
    tasks.sort_by_key(|t| {
        (
            t["created_at"].as_u64().unwrap_or(0),
            t["id"].as_str().unwrap_or("").to_owned(),
        )
    });
    Ok((tasks, unreadable, diagnostics))
}
fn snippet(text: &str, q: &str) -> String {
    // Character-indexed truncation keeps Chinese and emoji intact.
    let chars: Vec<_> = text.chars().collect();
    let start = chars
        .iter()
        .enumerate()
        .find_map(|(i, _)| {
            let s: String = chars[i..].iter().take(q.chars().count() + 8).collect();
            s.to_lowercase()
                .starts_with(&q.to_lowercase())
                .then_some(i.saturating_sub(35))
        })
        .unwrap_or(0);
    format!(
        "{}{}{}",
        if start > 0 { "…" } else { "" },
        chars.iter().skip(start).take(160).collect::<String>(),
        if chars.len() > start + 160 { "…" } else { "" }
    )
}
fn project_matches(workspace: &str, project: &str) -> bool {
    project.is_empty()
        || workspace == project
        || workspace.starts_with(&format!("{}/", project.trim_end_matches('/')))
}
pub fn rpc(v: &Value) -> Result<Value> {
    rpc_at(&runner_dir()?, v)
}
fn rpc_at(root: &Path, v: &Value) -> Result<Value> {
    let op = field(v, "op")?;
    let metadata = root.join("sessions");
    if op == "session_meta_set" || op == "session_meta_get" {
        let id = field(v, "session_id")?;
        valid_id(id)?;
        let (tasks, _, _) = scan(root, "")?;
        if let Some(home) = v["allowed_home"].as_str() {
            if tasks.iter().filter(|t| t["session_id"] == id).any(|t| {
                let w = t["workspace"].as_str().unwrap_or("");
                !Path::new(w).starts_with(home) && v["allowed_workspace"].as_str() != Some(w)
            }) {
                bail!("会话不属于当前账号允许目录");
            }
        }
        if !tasks.iter().any(|t| t["session_id"] == id) {
            bail!("会话不存在");
        }
        if op == "session_meta_get" {
            return meta(&metadata, id);
        }
        let _lock = guard(&metadata)?;
        let mut m = meta(&metadata, id)?;
        if v["expected_revision"].as_u64() != m["revision"].as_u64() {
            return Err(coded(
                "session_conflict",
                "会话已在其他窗口修改，请读取最新状态",
            ));
        }
        if let Some(s) = v["title"].as_str() {
            let s = s.trim();
            if s.is_empty() || s.chars().count() > 80 || s.chars().any(char::is_control) {
                bail!("会话名称须为1–80个字符，不能含控制字符");
            }
            m["title"] = json!(s);
        }
        if let Some(archived) = v["archived"].as_bool() {
            m["archived"] = json!(archived);
            m["archived_at"] = if archived { json!(now()) } else { Value::Null };
        }
        m["revision"] = json!(m["revision"].as_u64().unwrap_or(0) + 1);
        m["updated_at"] = json!(now());
        write_json(&metadata.join(format!("{id}.json")), &m)?;
        // Invalidate query membership/order after a metadata edit.
        write_json(
            &metadata.join("generation.json"),
            &json!(uuid::Uuid::new_v4().to_string()),
        )?;
        return Ok(m);
    }
    let generation_before =
        read_json::<Value>(&metadata.join("generation.json")).unwrap_or(json!("initial"));
    let q = v["q"].as_str().unwrap_or("").trim();
    if q.chars().count() > 200 {
        bail!("搜索内容最多200个字符");
    }
    let (mut tasks, mut unreadable, mut diagnostics) = scan(root, q)?;
    if let Some(home) = v["allowed_home"].as_str() {
        tasks.retain(|t| {
            let w = t["workspace"].as_str().unwrap_or("");
            Path::new(w).starts_with(home) || v["allowed_workspace"].as_str() == Some(w)
        });
    }
    if op == "active_tasks" {
        let mut active = vec![];
        for t in &tasks {
            if ["queued", "running", "waiting_approval"]
                .contains(&t["state"].as_str().unwrap_or(""))
            {
                // Reuse reconciliation and approval detection without changing worker contracts.
                match crate::runner::snapshot(t["id"].as_str().unwrap(), 0) {
                    Ok(mut s) => {
                        let first = tasks
                            .iter()
                            .find(|x| x["session_id"] == t["session_id"])
                            .unwrap_or(t);
                        s["session_meta"] = task_metadata(
                            t["session_id"].as_str().unwrap_or(""),
                            first["prompt"].as_str().unwrap_or(""),
                        );
                        s["title"] = s["session_meta"]["title"].clone();
                        s.as_object_mut().unwrap().remove("events");
                        s.as_object_mut().unwrap().remove("result");
                        active.push(s);
                    }
                    Err(_) => {
                        unreadable += 1;
                    }
                }
            }
        }
        return Ok(
            json!({"tasks":active,"incomplete":unreadable>0,"unreadable_count":unreadable,"diagnostics":diagnostics}),
        );
    }
    if op == "session_turns" {
        let id = field(v, "session_id")?;
        valid_id(id)?;
        let turns: Vec<_> = tasks
            .into_iter()
            .filter(|t| t["session_id"] == id)
            .map(|mut t| {
                t.as_object_mut().unwrap().remove("result");
                t
            })
            .collect();
        let offset = v["offset"].as_u64().unwrap_or(0) as usize;
        if offset > turns.len() {
            return Err(coded("search_expired", "轮次位置无效，请刷新"));
        }
        return Ok(
            json!({"turns":turns.iter().skip(offset).take(20).collect::<Vec<_>>(),"next_offset":if offset+20<turns.len(){Some(offset+20)}else{None},"incomplete":unreadable>0}),
        );
    }
    let q = v["q"].as_str().unwrap_or("").trim();
    if q.chars().count() > 200 {
        bail!("搜索内容最多200个字符");
    }
    let archive = v["archive_filter"].as_str().unwrap_or("recent");
    let project = v["project"].as_str().unwrap_or("");
    let mut groups: BTreeMap<String, Vec<Value>> = BTreeMap::new();
    for t in tasks {
        let id = t["session_id"]
            .as_str()
            .filter(|s| valid_id(s).is_ok())
            .unwrap_or(t["id"].as_str().unwrap())
            .to_owned();
        groups.entry(id).or_default().push(t);
    }
    let mut sessions = vec![];
    for (id, turns) in groups {
        let m = match meta(&metadata, &id) {
            Ok(m) => m,
            Err(_) => {
                unreadable += 1;
                if diagnostics.len() < 100 {
                    diagnostics
                        .push(json!({"session_id":id,"reason":"会话元数据不可读取或格式无效"}));
                }
                continue;
            }
        };
        let archived = m["archived"] == true;
        if (archive == "recent" && archived) || (archive == "archived" && !archived) {
            continue;
        }
        let first = &turns[0];
        let latest = turns.last().unwrap();
        if !project_matches(latest["workspace"].as_str().unwrap_or(""), project) {
            continue;
        }
        let name = m["title"]
            .as_str()
            .map(str::to_owned)
            .unwrap_or_else(|| title(first["prompt"].as_str().unwrap_or("")));
        let mut hits = vec![];
        if !q.is_empty() {
            let lower = q.to_lowercase();
            if name.to_lowercase().contains(&lower) {
                hits.push(
                    json!({"field":"title","task_id":latest["id"],"snippet":snippet(&name,q)}),
                );
            }
            for t in &turns {
                for hit in t["search_hits"].as_array().into_iter().flatten() {
                    if hits.len() < 20 {
                        hits.push(hit.clone());
                    }
                }
            }
            if hits.is_empty() {
                continue;
            }
        }
        sessions.push(json!({"session_id":id,"task_id":latest["id"],"title":name,"workspace":latest["workspace"],"state":latest["state"],"access":latest["access"],"mode":latest["mode"],"model_resolved":latest["model_resolved"],"model_requested":latest["model_requested"],"created_at":first["created_at"],"updated_at":latest["updated_at"],"archived":archived,"revision":m["revision"],"turn_count":turns.len(),"legacy":first["session_id"].as_str().is_none_or(|s|valid_id(s).is_err()),"hits":hits}));
    }
    sessions.sort_by_key(|s| {
        std::cmp::Reverse((
            s["updated_at"].as_u64().unwrap_or(0),
            s["session_id"].as_str().unwrap().to_owned(),
        ))
    });
    private_dir(&metadata)?;
    let generation =
        read_json::<Value>(&metadata.join("generation.json")).unwrap_or(json!("initial"));
    if generation_before != generation {
        return Err(coded("search_expired", "会话已变化，请刷新"));
    }
    let key = json!({"q":q,"archive":archive,"project":project,"allowed_home":v["allowed_home"],"allowed_workspace":v["allowed_workspace"]});
    let snapshots = metadata.join("queries");
    private_dir(&snapshots)?;
    let (ids, offset, snapshot_id) = if let Some(cursor) = v["cursor"].as_str() {
        let (id, offset) = cursor
            .split_once(':')
            .ok_or_else(|| coded("search_expired", "分页已过期，请刷新搜索"))?;
        valid_id(id).map_err(|_| coded("search_expired", "分页游标无效，请刷新"))?;
        let snap: Value = read_json(&snapshots.join(format!("{id}.json")))
            .map_err(|_| coded("search_expired", "分页已过期，请刷新搜索"))?;
        if snap["expires_at"].as_u64().unwrap_or(0) < now()
            || snap["generation"] != generation
            || snap["key"] != key
        {
            return Err(coded("search_expired", "搜索已变化，请刷新"));
        }
        (
            snap["ids"].as_array().cloned().unwrap_or_default(),
            offset
                .parse::<usize>()
                .map_err(|_| coded("search_expired", "分页位置无效，请刷新"))?,
            id.to_owned(),
        )
    } else {
        let mut old = vec![];
        for e in std::fs::read_dir(&snapshots)?.flatten() {
            if e.path().extension().is_some_and(|s| s == "json") {
                let modified = e
                    .metadata()
                    .and_then(|m| m.modified())
                    .unwrap_or(std::time::UNIX_EPOCH);
                if modified.elapsed().unwrap_or_default().as_secs() > 120 {
                    let _ = std::fs::remove_file(e.path());
                } else {
                    old.push((modified, e.path()));
                }
            }
        }
        old.sort();
        let excess = old.len().saturating_sub(31);
        for (_, p) in old.into_iter().take(excess) {
            let _ = std::fs::remove_file(p);
        }
        let id = uuid::Uuid::new_v4().to_string();
        let ids: Vec<Value> = sessions.iter().map(|s| s["session_id"].clone()).collect();
        if ids.len() > 100_000 {
            bail!("会话数量超出查询上限，请缩小项目范围");
        }
        write_json(
            &snapshots.join(format!("{id}.json")),
            &json!({"ids":ids,"expires_at":now()+120,"generation":generation,"key":key}),
        )?;
        (ids, 0, id)
    };
    if offset > ids.len() {
        return Err(coded("search_expired", "分页位置无效，请刷新"));
    }
    let limit = v["limit"].as_u64().unwrap_or(30).clamp(1, 50) as usize;
    let page: Vec<_> = ids
        .iter()
        .skip(offset)
        .take(limit)
        .filter_map(|id| sessions.iter().find(|s| s["session_id"] == *id).cloned())
        .collect();
    Ok(
        json!({"sessions":page,"snapshot_id":snapshot_id,"next_cursor":if offset+limit<ids.len(){Some(format!("{snapshot_id}:{}",offset+limit))}else{None},"total":ids.len(),"incomplete":unreadable>0,"unreadable_count":unreadable,"read_at":now(),"diagnostics":diagnostics}),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn search_metadata_pagination_and_conflicts() {
        let root = std::env::temp_dir().join(uuid::Uuid::new_v4().to_string());
        private_dir(&root).unwrap();
        let session = uuid::Uuid::new_v4().to_string();
        for i in 0..105 {
            let id = uuid::Uuid::new_v4().to_string();
            private_dir(&root.join(&id)).unwrap();
            write_json(&root.join(&id).join("task.json"),&json!({"id":id,"session_id":if i<2{session.clone()}else{uuid::Uuid::new_v4().to_string()},"workspace":"/home/a/p","prompt":if i==0{"\n首轮标题"}else{"后续问题"},"created_at":i,"updated_at":i,"state":"completed","result":{"result":"权限校验 <script>"}})).unwrap();
        }
        let r = rpc_at(&root, &json!({"op":"session_search","q":"权限"})).unwrap();
        assert_eq!(r["total"], 104);
        assert_eq!(r["sessions"].as_array().unwrap().len(), 30);
        let r2 = rpc_at(
            &root,
            &json!({"op":"session_search","q":"权限","cursor":r["next_cursor"]}),
        )
        .unwrap();
        assert_eq!(r2["sessions"].as_array().unwrap().len(), 30);
        let m=rpc_at(&root,&json!({"op":"session_meta_set","session_id":session,"title":"新名称","archived":true,"expected_revision":0})).unwrap();
        assert_eq!(m["revision"], 1);
        assert!(rpc_at(&root,&json!({"op":"session_meta_set","session_id":session,"title":"覆盖","expected_revision":0})).unwrap_err().downcast_ref::<CodedError>().is_some_and(|e|e.code=="session_conflict"));
        assert!(
            rpc_at(
                &root,
                &json!({"op":"session_search","q":"权限","cursor":r["next_cursor"]})
            )
            .unwrap_err()
            .downcast_ref::<CodedError>()
            .is_some_and(|e| e.code == "search_expired")
        );
        let archived = rpc_at(
            &root,
            &json!({"op":"session_list","archive_filter":"archived"}),
        )
        .unwrap();
        assert_eq!(archived["sessions"][0]["title"], "新名称");
        assert_eq!(archived["sessions"][0]["turn_count"], 2);
        assert!(rpc_at(&root,&json!({"op":"session_meta_set","session_id":session,"title":"bad\nname","expected_revision":1})).is_err());
        std::fs::remove_dir_all(root).unwrap();
    }
}
