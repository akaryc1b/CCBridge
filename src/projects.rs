//! Project registry is independent of database connections. Legacy names are read, never rewritten.
use crate::common::*;
use anyhow::{Result, bail};
use serde_json::{Value, json};
use std::collections::BTreeMap;
// Read-only compatibility adapter: retain the existing redacted `databases` field.
// No database probe, credential mutation or connection is performed here.
fn registry(legacy: &Value) -> Result<BTreeMap<String, String>> {
    let mut names: BTreeMap<String, String> = legacy["projects"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|p| Some((p["path"].as_str()?.into(), p["name"].as_str()?.into())))
        .collect();
    let file = runner_dir()?.join("projects.json");
    if file.exists() {
        names.extend(read_json::<BTreeMap<String, String>>(&file)?);
    }
    Ok(names)
}
fn canonical(s: &str) -> Result<String> {
    let p = std::fs::canonicalize(s)?;
    if !p.is_dir() || !p.starts_with(home()?.canonicalize()?) {
        bail!("项目必须是当前账号 HOME 下的目录")
    }
    Ok(p.to_string_lossy().into())
}
pub fn rpc(v: &Value) -> Result<Value> {
    let legacy = crate::databases::rpc(&json!({"op":"db_projects"}))?;
    let mut projects = registry(&legacy)?;
    match field(v, "op")? {
        "project_list" => Ok(
            json!({"projects":projects.iter().map(|(path,name)|json!({"path":path,"name":name,"databases":legacy["projects"].as_array().and_then(|ps|ps.iter().find(|p|p["path"]==*path)).map(|p|p["databases"].clone()).unwrap_or_else(||json!([]))})).collect::<Vec<_>>()}),
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
