//! Project registry is independent of database connections. Legacy names are read, never rewritten.
use crate::common::*;
use anyhow::{Result, bail};
use serde_json::{Value, json};
use std::collections::BTreeMap;
fn registry() -> Result<BTreeMap<String, String>> {
    let file = runner_dir()?.join("projects.json");
    if file.exists() {
        return read_json(&file);
    }
    let old = runner_dir()?.join("databases.json");
    if old.exists() {
        let v: Value = read_json(&old)?;
        return Ok(v["projects"]
            .as_object()
            .into_iter()
            .flatten()
            .filter_map(|(k, v)| v.as_str().map(|name| (k.clone(), name.into())))
            .collect());
    }
    Ok(BTreeMap::new())
}
fn canonical(s: &str) -> Result<String> {
    let p = std::fs::canonicalize(s)?;
    if !p.is_dir() || !p.starts_with(home()?.canonicalize()?) {
        bail!("项目必须是当前账号 HOME 下的目录")
    }
    Ok(p.to_string_lossy().into())
}
pub fn rpc(v: &Value) -> Result<Value> {
    let mut projects = registry()?;
    match field(v, "op")? {
        "project_list" => Ok(
            json!({"projects":projects.iter().map(|(path,name)|json!({"path":path,"name":name})).collect::<Vec<_>>()}),
        ),
        "project_save" => {
            let path = canonical(field(v, "workspace")?)?;
            let name = field(v, "name")?.trim();
            if name.is_empty() || projects.iter().any(|(p, n)| p != &path && n == name) {
                bail!("项目名称不能为空或与其他项目重复")
            }
            projects.insert(path.clone(), name.into());
            write_json(&runner_dir()?.join("projects.json"), &projects)?;
            Ok(json!({"path":path,"name":name}))
        }
        "project_resolve" => {
            let name = field(v, "project")?;
            let paths: Vec<_> = projects
                .iter()
                .filter(|(p, n)| *p == name || *n == name)
                .collect();
            if paths.len() != 1 {
                bail!("项目不存在或名称不唯一，请使用 project_list 的准确路径")
            }
            Ok(json!({"workspace":canonical(paths[0].0)?}))
        }
        _ => bail!("不支持此项目操作"),
    }
}
