//! Per-SSH-user database profiles and exact workspace bindings. Secrets never leave RPC input.
use crate::common::*;
use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::{
    collections::BTreeSet,
    path::{Path, PathBuf},
};
#[derive(Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Database {
    pub id: String,
    pub name: String,
    pub kind: String,
    pub host: String,
    pub port: u16,
    pub user: String,
    pub database: String,
    pub enabled: bool,
    pub package_root: String,
    pub node: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub password: String,
}
#[derive(Clone, Default, Serialize, Deserialize)]
pub struct Binding {
    pub alias: String,
    pub connection_id: String,
}
#[derive(Default, Serialize, Deserialize)]
struct Store {
    #[serde(default)]
    projects: std::collections::BTreeMap<String, String>,
    connections: Vec<Database>,
    bindings: std::collections::BTreeMap<String, Vec<Binding>>,
}
fn project_list(s: &Store) -> Vec<Value> {
    let keys: BTreeSet<_> = s
        .projects
        .keys()
        .chain(s.bindings.keys())
        .cloned()
        .collect();
    keys.into_iter().map(|w|{let name=s.projects.get(&w).cloned().unwrap_or_else(||Path::new(&w).file_name().unwrap_or_default().to_string_lossy().into());
        let mappings:Vec<_>=s.bindings.get(&w).into_iter().flatten().map(|b|{
            let d=s.connections.iter().find(|d|d.id==b.connection_id);
            json!({"alias":b.alias,"connection_id":b.connection_id,"connection_name":d.map(|d|d.name.as_str()),"enabled":d.is_some_and(|d|d.enabled)})
        }).collect();json!({"name":name,"path":w,"databases":mappings})
    }).collect()
}
fn path() -> Result<PathBuf> {
    Ok(runner_dir()?.join("databases.json"))
}
fn load() -> Result<Store> {
    let p = path()?;
    if p.exists() {
        read_json(&p)
    } else {
        Ok(Store::default())
    }
}
fn private_json(p: &Path, value: &impl Serialize) -> Result<()> {
    crate::common::write_json(p, value)
}

fn save(s: &Store) -> Result<()> {
    private_dir(&runner_dir()?)?;
    private_json(&path()?, s)?;
    crate::platform::private_file(&path()?)?;
    Ok(())
}
fn workspace(s: &str) -> Result<String> {
    let p = Path::new(s).canonicalize().context("项目目录不存在")?;
    if !p.is_dir() || !p.starts_with(home()?.canonicalize()?) {
        bail!("数据库映射项目必须是当前账号 HOME 下的目录")
    };
    Ok(p.to_string_lossy().into())
}
fn valid_alias(s: &str) -> bool {
    !s.is_empty() && s.len() <= 32 && s.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_')
}
fn validate(d: &Database) -> Result<()> {
    valid_id(&d.id)?;
    if d.name.trim().is_empty() {
        bail!("请填写连接名称")
    };
    if !["mysql", "postgres"].contains(&d.kind.as_str()) {
        bail!("当前支持 MySQL 与 PostgreSQL")
    };
    if d.port == 0
        || d.host.trim().is_empty()
        || d.user.trim().is_empty()
        || d.database.trim().is_empty()
    {
        bail!("请填写地址、端口、用户名和数据库名")
    };
    if !Path::new(&d.node).is_absolute() || !Path::new(&d.package_root).is_absolute() {
        bail!("请探测或填写远端 Node 和数据库 MCP 安装绝对路径")
    };
    Ok(())
}
pub fn redacted(d: &Database) -> Value {
    let mut v = serde_json::to_value(d).unwrap();
    v.as_object_mut().unwrap().remove("password");
    v["password_saved"] = json!(!d.password.is_empty());
    v["permission_mode"] = json!("safe");
    v
}
pub fn rpc(v: &Value) -> Result<Value> {
    let mut s = load()?;
    match field(v, "op")? {
        "db_projects" => Ok(json!({"projects":project_list(&s)})),
        "db_resolve" => {
            let name = field(v, "project")?;
            let list = project_list(&s);
            let found: Vec<_> = list
                .iter()
                .filter(|p| p["path"] == name || p["name"] == name)
                .collect();
            if found.len() != 1 {
                bail!("项目不存在或名称不唯一，请调用 project_list 并使用准确的项目路径")
            }
            let w = workspace(found[0]["path"].as_str().unwrap())?;
            Ok(json!({"workspace":w}))
        }
        "db_project_save" => {
            let w = workspace(field(v, "workspace")?)?;
            let name = field(v, "name")?.trim();
            if name.is_empty() {
                bail!("请填写项目名称")
            }
            if s.projects.iter().any(|(path, n)| path != &w && n == name) {
                bail!("项目名称已存在，请使用不同名称")
            }
            s.projects.insert(w.clone(), name.into());
            save(&s)?;
            Ok(json!({"workspace":w,"name":name}))
        }
        "db_list" => {
            let w = v["workspace"].as_str().unwrap_or("");
            Ok(
                json!({"projects":project_list(&s),"connections":s.connections.iter().map(|d| { let mut r=redacted(d); r["used_by"]=json!(s.bindings.iter().filter(|(_,bs)|bs.iter().any(|b|b.connection_id==d.id)).map(|(w,_)|w).collect::<Vec<_>>()); r }).collect::<Vec<_>>(),"bindings":s.bindings.get(w).cloned().unwrap_or_default(),"workspace":w,"storage":"当前 SSH 账号 HOME，私有权限 600；不修改 /etc 共享配置"}),
            )
        }
        "db_save" => {
            let mut d: Database =
                serde_json::from_value(v["connection"].clone()).context("数据库配置格式不正确")?;
            if d.id.is_empty() {
                d.id = uuid::Uuid::new_v4().to_string()
            };
            validate(&d)?;
            if d.password.is_empty() {
                if let Some(old) = s.connections.iter().find(|x| x.id == d.id) {
                    d.password = old.password.clone()
                }
            };
            s.connections.retain(|x| x.id != d.id);
            s.connections.push(d.clone());
            save(&s)?;
            Ok(redacted(&d))
        }
        "db_bind" => {
            let w = workspace(field(v, "workspace")?)?;
            let bs: Vec<Binding> = serde_json::from_value(v["bindings"].clone())?;
            if bs.len() > 8 {
                bail!("每个项目最多绑定 8 个数据库")
            };
            let mut aliases = BTreeSet::new();
            for b in &bs {
                if !valid_alias(&b.alias) || !aliases.insert(b.alias.clone()) {
                    bail!("用途别名须唯一，仅用英文字母、数字、下划线，最多32位")
                };
                if !s.connections.iter().any(|d| d.id == b.connection_id) {
                    bail!("映射引用的数据库连接不存在")
                }
            }
            s.bindings.insert(w.clone(), bs.clone());
            save(&s)?;
            Ok(json!({"workspace":w,"bindings":bs}))
        }
        "db_remove" => {
            let id = field(v, "id")?;
            if s.bindings.values().flatten().any(|b| b.connection_id == id) {
                bail!("连接仍被项目引用，请先移除映射；也可关闭此连接")
            };
            s.connections.retain(|d| d.id != id);
            save(&s)?;
            Ok(json!({"removed":true}))
        }
        "db_detect" => detect(),
        "db_test" | "db_protocol_test" => {
            let d = s
                .connections
                .iter()
                .find(|d| Some(d.id.as_str()) == v["id"].as_str())
                .context("请先保存连接")?;
            test_connection(d, v["op"] == "db_protocol_test")
        }
        _ => bail!("未知数据库操作"),
    }
}
/// Exact project mapping, never inherit a parent/sibling project implicitly.
pub fn snapshot(workspace: &str, parent: Option<&str>) -> Result<Vec<(String, Database)>> {
    if let Some(id) = parent {
        let p = task_dir(id)?.join("databases.json");
        return if p.exists() {
            read_json(&p)
        } else {
            Ok(vec![])
        };
    };
    let s = load()?;
    let mut out = vec![];
    for b in s.bindings.get(workspace).into_iter().flatten() {
        let d = s
            .connections
            .iter()
            .find(|d| d.id == b.connection_id)
            .context("项目数据库映射已失效")?;
        if d.enabled {
            validate(d)?;
            verify_package(d)?;
            out.push((b.alias.clone(), d.clone()))
        }
    }
    Ok(out)
}
pub fn save_snapshot(dir: &Path, connections: &[(String, Database)]) -> Result<()> {
    if !connections.is_empty() {
        private_json(&dir.join("databases.json"), &connections)?;
        crate::platform::private_file(&dir.join("databases.json"))?;
    }
    Ok(())
}
fn verify_package(d: &Database) -> Result<()> {
    let p = Path::new(&d.package_root);
    let v: Value = read_json(&p.join("package.json")).context("无法读取远端数据库 MCP 安装目录")?;
    if v["name"] != "universal-db-mcp"
        || v["version"] != "2.14.0"
        || !p.join("dist/mcp/mcp-server.js").is_file()
    {
        bail!("需要已安装的 universal-db-mcp 2.14.0；不自动修改服务器依赖")
    };
    if !Path::new(&d.node).is_file() {
        bail!("远端 Node 可执行文件不存在")
    };
    Ok(())
}
fn find_path(name: &str) -> Option<PathBuf> {
    std::env::split_paths(&std::env::var_os("PATH")?)
        .map(|d| d.join(name))
        .find(|p| p.is_file())
        .and_then(|p| p.canonicalize().ok())
}
fn package_candidate(root: &Path, source: &str) -> Option<Value> {
    let root = root.canonicalize().ok()?;
    let meta: Value = read_json(&root.join("package.json")).ok()?;
    if meta["name"] != "universal-db-mcp" {
        return None;
    }
    let compatible = meta["version"] == "2.14.0"
        && root.join("dist/mcp/mcp-server.js").is_file()
        && root.join("dist/utils/adapter-factory.js").is_file();
    Some(
        json!({"package_root":root,"version":meta["version"],"compatible":compatible,"source":source}),
    )
}
fn wrapper_candidates(file: &Path, roots: &mut Vec<(PathBuf, String)>) {
    // Inspect literals only; never execute a registered wrapper or return its configuration.
    if std::fs::metadata(file)
        .map(|m| m.len() > 128 * 1024)
        .unwrap_or(true)
    {
        return;
    }
    if let Ok(text) = std::fs::read_to_string(file) {
        for token in text.split(['\'', '"', '`', '\n', ' ']) {
            if let Some((root, _)) = token.split_once("/node_modules/universal-db-mcp/") {
                if root.starts_with('/') {
                    roots.push((
                        PathBuf::from(format!("{root}/node_modules/universal-db-mcp")),
                        "Claude MCP 包装器".into(),
                    ));
                }
            }
        }
    }
}
fn detect() -> Result<Value> {
    let node = find_path("node");
    let h = home()?;
    let mut roots: Vec<(PathBuf, String)> = vec![];
    let mut registered = vec![];
    let mut configs = vec![h.join(".claude.json"), h.join(".claude/.claude.json")];
    if let Some(p) = std::env::var_os("CLAUDE_CONFIG_DIR") {
        configs.insert(0, PathBuf::from(p).join(".claude.json"));
    }
    for cfg in configs {
        let Ok(value) = read_json::<Value>(&cfg) else {
            continue;
        };
        if let Some(servers) = value["mcpServers"].as_object() {
            for (name, server) in servers {
                let Some(cmd) = server["command"].as_str() else {
                    continue;
                };
                if !name.to_lowercase().contains("db") && !cmd.contains("database") {
                    continue;
                }
                let path = if Path::new(cmd).is_absolute() {
                    Some(PathBuf::from(cmd))
                } else {
                    find_path(cmd)
                };
                // Args/env are deliberately neither returned nor copied into tasks.
                registered.push(json!({"name":name,"command":path,"source":cfg,"inherited":false}));
                if let Some(p) = path {
                    wrapper_candidates(&p, &mut roots);
                }
            }
        }
    }
    if let Some(p) = find_path("claude-database-mcp") {
        wrapper_candidates(&p, &mut roots);
    }
    if let Some(p) =
        find_path("universal-db-mcp").and_then(|p| p.parent()?.parent().map(Path::to_path_buf))
    {
        roots.push((p, "PATH".into()));
    }
    for p in [
        "/opt/claude-mcp/universal-db-mcp/node_modules/universal-db-mcp",
        "/usr/local/lib/node_modules/universal-db-mcp",
        "/usr/lib/node_modules/universal-db-mcp",
        "/opt/homebrew/lib/node_modules/universal-db-mcp",
    ] {
        roots.push((PathBuf::from(p), "标准安装位置".into()));
    }
    for p in [
        h.join(".local/lib/node_modules/universal-db-mcp"),
        h.join(".npm-global/lib/node_modules/universal-db-mcp"),
    ] {
        roots.push((p, "账号安装位置".into()));
    }
    if let Ok(versions) = std::fs::read_dir(h.join(".nvm/versions/node")) {
        for p in versions.flatten().take(32) {
            roots.push((
                p.path().join("lib/node_modules/universal-db-mcp"),
                "nvm".into(),
            ));
        }
    }
    let mut seen = BTreeSet::new();
    let candidates: Vec<_> = roots
        .iter()
        .filter_map(|(p, source)| package_candidate(p, source))
        .filter(|c| seen.insert(c["package_root"].to_string()))
        .collect();
    let selected = candidates.iter().find(|c| c["compatible"] == true);
    Ok(
        json!({"node":node,"package_root":selected.map(|c|&c["package_root"]),"candidates":candidates,"registered":registered,"notice":"已检查 Claude 注册、包装器、标准目录和当前账号安装位置；只查找软件，不读取共享数据库凭据，不连接数据库。已有全局 MCP 不会自动挂入项目。"}),
    )
}

pub fn task_summary(dir: &Path, workspace: &str) -> Result<Value> {
    let file = dir.join("databases.json");
    let snapshot: Vec<(String, Database)> = if file.exists() {
        read_json(&file)?
    } else {
        vec![]
    };
    let store = load()?;
    let current: Vec<(String, Database)> = store
        .bindings
        .get(workspace)
        .into_iter()
        .flatten()
        .filter_map(|b| {
            store
                .connections
                .iter()
                .find(|d| d.id == b.connection_id && d.enabled)
                .map(|d| (b.alias.clone(), d.clone()))
        })
        .collect();
    let changed = serde_json::to_value(&snapshot)? != serde_json::to_value(&current)?;
    Ok(
        json!({"mappings":snapshot.iter().map(|(alias,d)|json!({"alias":alias,"name":d.name,"connection_id":d.id,"database":d.database,"permission_mode":"safe"})).collect::<Vec<_>>(),"changed":changed}),
    )
}
pub fn mcp_servers(dir: &Path, exe: &Path) -> Result<serde_json::Map<String, Value>> {
    let p = dir.join("databases.json");
    let mut out = serde_json::Map::new();
    if !p.exists() {
        return Ok(out);
    };
    let ds: Vec<(String, Database)> = read_json(&p)?;
    for (alias, d) in ds {
        if !valid_alias(&alias) {
            bail!("数据库用途别名无效")
        };
        out.insert(format!("db_{alias}"),json!({"command":exe,"args":["database-server",dir.file_name().unwrap().to_string_lossy(),d.id]}));
    }
    Ok(out)
}
pub fn serve(task: &str, id: &str) -> Result<()> {
    valid_id(id)?;
    let dir = task_dir(task)?;
    let ds: Vec<(String, Database)> = read_json(&dir.join("databases.json"))?;
    let (_, d) = ds
        .into_iter()
        .find(|(_, d)| d.id == id)
        .context("本任务没有该数据库映射")?;
    verify_package(&d)?;
    let script = dir.join("database-launch.mjs");
    std::fs::write(&script, include_str!("database-launch.mjs"))?;
    #[cfg(unix)]
    let error = std::os::unix::process::CommandExt::exec(
        std::process::Command::new(&d.node)
            .arg(script)
            .arg(dir.join("databases.json"))
            .arg(id),
    );
    #[cfg(unix)]
    return Err(error.into());
    #[cfg(windows)]
    {
        let status = std::process::Command::new(&d.node)
            .arg(script)
            .arg(dir.join("databases.json"))
            .arg(id)
            .status()?;
        if !status.success() {
            bail!("数据库 MCP 子进程已退出：{status}");
        }
        Ok(())
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn detects_wrapper_package_without_executing_or_returning_secrets() {
        let root = std::env::temp_dir().join(format!("ccbridge-runtime-{}", uuid::Uuid::new_v4()));
        let package = root.join("node_modules/universal-db-mcp");
        std::fs::create_dir_all(package.join("dist/mcp")).unwrap();
        std::fs::create_dir_all(package.join("dist/utils")).unwrap();
        std::fs::write(
            package.join("package.json"),
            r#"{"name":"universal-db-mcp","version":"2.14.0"}"#,
        )
        .unwrap();
        std::fs::write(package.join("dist/mcp/mcp-server.js"), "").unwrap();
        std::fs::write(package.join("dist/utils/adapter-factory.js"), "").unwrap();
        let wrapper = root.join("wrapper");
        std::fs::write(
            &wrapper,
            format!(
                "const password='must-not-return'; const entry='{}';",
                package.join("dist/index.js").display()
            ),
        )
        .unwrap();
        let mut roots = vec![];
        wrapper_candidates(&wrapper, &mut roots);
        assert_eq!(roots.len(), 1);
        let c = package_candidate(&roots[0].0, &roots[0].1).unwrap();
        assert_eq!(c["compatible"], true);
        assert!(!c.to_string().contains("must-not-return"));
        std::fs::write(
            package.join("package.json"),
            r#"{"name":"universal-db-mcp","version":"3.0.0"}"#,
        )
        .unwrap();
        assert_eq!(
            package_candidate(&package, "test").unwrap()["compatible"],
            false
        );
        let secret = root.join("secret.json");
        private_json(&secret, &json!({"password":"fixture-secret"})).unwrap();
        #[cfg(unix)]
        use std::os::unix::fs::PermissionsExt;
        #[cfg(unix)]
        assert_eq!(
            std::fs::metadata(secret).unwrap().permissions().mode() & 0o777,
            0o600
        );
        std::fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn aliases_and_redaction() {
        assert!(valid_alias("business_dev"));
        assert!(!valid_alias("../../x"));
        assert!(!valid_alias("业务库"));
        let d = Database {
            password: "secret-test".into(),
            ..Default::default()
        };
        let v = redacted(&d);
        assert!(v.get("password").is_none());
        assert_eq!(v["password_saved"], true);
        assert!(!v.to_string().contains("secret-test"));
    }
}

fn test_connection(d: &Database, protocol_only: bool) -> Result<Value> {
    verify_package(d)?;
    let dir = runner_dir()?
        .join("database-tests")
        .join(uuid::Uuid::new_v4().to_string());
    private_dir(&dir)?;
    let result = (|| -> Result<Value> {
        save_snapshot(&dir, &[("test".into(), d.clone())])?;
        let script = dir.join("launch.mjs");
        std::fs::write(&script, include_str!("database-launch.mjs"))?;
        use std::process::{Command, Stdio};
        let mut command = Command::new(&d.node);
        command
            .arg(script)
            .arg(dir.join("databases.json"))
            .arg(&d.id)
            .arg(if protocol_only {
                "--protocol-test"
            } else {
                "--test"
            })
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null());
        let mut child =
            crate::platform::ProcessTree::spawn(command).context("无法启动远端 Node")?;
        let end = std::time::Instant::now() + std::time::Duration::from_secs(30);
        loop {
            if let Some(status) = child.try_wait()? {
                if status.success() {
                    return Ok(if protocol_only {
                        json!({"mcp_connected":true,"database_connected":false,"notice":"MCP 握手与只读工具枚举成功；未连接数据库"})
                    } else {
                        json!({"connected":true,"permission_mode":"safe","notice":"数据库认证与连接成功；未读取业务表，不代表所有表均有访问权限"})
                    });
                } else {
                    bail!(if protocol_only {
                        "MCP 握手失败，请检查 Node、安装目录与适配版本；未连接数据库"
                    } else {
                        "数据库连接失败，请检查地址、认证、网络和数据库权限；凭据不返回客户端"
                    })
                }
            };
            if std::time::Instant::now() > end {
                let _ = child.terminate(true);
                let _ = child.wait();
                bail!("数据库连接测试超过30秒，已停止本次测试")
            };
            std::thread::sleep(std::time::Duration::from_millis(100));
        }
    })();
    let _ = std::fs::remove_dir_all(&dir);
    result
}
