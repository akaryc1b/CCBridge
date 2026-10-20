use crate::common::*;
use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::os::unix::process::CommandExt;
use std::{
    fs::{File, OpenOptions},
    io::{BufRead, BufReader, Read, Write},
    path::Path,
    process::{Command, Stdio},
    time::Duration,
};

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct Task {
    pub id: String,
    pub session_id: String,
    pub workspace: String,
    pub claude: String,
    #[serde(default)]
    pub model: String,
    #[serde(default)]
    pub model_requested: String,
    #[serde(default)]
    pub model_source: String,
    #[serde(default)]
    pub model_resolved: Option<String>,
    #[serde(default)]
    pub model_status: String,
    #[serde(default)]
    pub observed_session_id: Option<String>,
    pub mode: String,
    pub prompt: String,
    pub resume: bool,
    #[serde(default)]
    pub reply_to: Option<String>,
    pub state: String,
    pub created_at: u64,
    pub updated_at: u64,
    pub worker_pid: u32,
    pub result: Option<Value>,
    pub error: Option<String>,
}
impl Task {
    fn save(&self) -> Result<()> {
        write_json(&task_dir(&self.id)?.join("task.json"), self)
    }
}
pub fn rpc_stdio() -> Result<()> {
    let mut s = String::new();
    std::io::stdin().take(1024 * 1024).read_to_string(&mut s)?;
    let r = (|| dispatch(serde_json::from_str(&s)?))();
    println!("{}", envelope(r));
    Ok(())
}
pub fn dispatch(v: Value) -> Result<Value> {
    match field(&v, "op")? {
        "project_list" | "project_save" | "project_resolve" => crate::projects::rpc(&v),
        "db_list" | "db_save" | "db_bind" | "db_remove" | "db_detect" | "db_test"
        | "db_project_save" | "db_projects" | "db_resolve" => crate::databases::rpc(&v),
        "repository_inspect" => {
            let path = std::fs::canonicalize(field(&v, "path")?)?;
            if !path.starts_with(home()?.canonicalize()?) {
                bail!("仓库必须在当前账号 HOME 下")
            }
            Ok(serde_json::to_value(crate::repository::inspect(&path)?)?)
        }
        "repositories" => {
            let h = home()?.canonicalize()?;
            let root = v["root"]
                .as_str()
                .filter(|s| !s.is_empty())
                .map(std::path::PathBuf::from)
                .unwrap_or(h.clone());
            Ok(serde_json::to_value(crate::repository::scan(&root, &h)?)?)
        }
        "probe" => {
            let work = if let Some(w) = v["workspace"].as_str().filter(|s| !s.is_empty()) {
                Some(std::fs::canonicalize(w).context("远端代码目录不存在")?)
            } else {
                None
            };
            let claude = v["claude"].as_str().unwrap_or("");
            let out = if claude.starts_with('/') {
                Command::new(claude).arg("--version").output().ok()
            } else {
                None
            };
            let version = out
                .as_ref()
                .filter(|o| o.status.success())
                .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string());
            Ok(
                json!({"runner_version":env!("CARGO_PKG_VERSION"),"protocol_version":1,"capabilities":["models_v1","projects_v1","session_identity_v1"],"workspace":work,"claude_version":version,"claude_error":if version.is_none(){Some("当前账号未找到可运行的 Claude，请在高级设置填写绝对路径，或在远端安装并登录")}else{None},"account_home":home()?,"state_directory":runner_dir()?,"code_access":"remote_directory","warning":"工作目录不是 OS 沙箱；全自动模式拥有该 SSH 账号的文件与命令权限"}),
            )
        }
        "start" => start(v),
        "status" | "result" => snapshot(field(&v, "task_id")?, v["cursor"].as_u64().unwrap_or(0)),
        "list" => {
            let workspace = v["workspace"].as_str();
            let mut tasks = vec![];
            for entry in std::fs::read_dir(runner_dir()?)? {
                let entry = entry?;
                if let Some(id) = entry.file_name().to_str() {
                    if valid_id(id).is_ok() {
                        if let Ok(mut t) = read_json::<Task>(&entry.path().join("task.json")) {
                            reconcile(&mut t)?;
                            if workspace.is_none_or(|w| w == t.workspace) {
                                tasks.push(t);
                            }
                        }
                    }
                }
            }
            tasks.sort_by_key(|t| std::cmp::Reverse(t.created_at));
            tasks.truncate(100);
            Ok(
                json!({"tasks":tasks.into_iter().map(|t| json!({"id":t.id,"session_id":t.session_id,"state":t.state,"mode":t.mode,"workspace":t.workspace,"created_at":t.created_at,"title":t.prompt.chars().take(64).collect::<String>(),"resume":t.resume,"reply_to":t.reply_to})).collect::<Vec<_>>()}),
            )
        }
        "cancel" => {
            let dir = task_dir(field(&v, "task_id")?)?;
            let t: Task = read_json(&dir.join("task.json"))?;
            if matches!(t.state.as_str(), "queued" | "running") {
                std::fs::write(dir.join("cancel"), b"cancel")?;
            }
            Ok(json!({"task_id":t.id,"cancel_requested":true}))
        }
        "approve" => {
            let dir = task_dir(field(&v, "task_id")?)?;
            let rid = field(&v, "request_id")?;
            valid_id(rid)?;
            let p = dir.join(format!("approval-{rid}.json"));
            let req: Value = read_json(&p)?;
            let task: Task = read_json(&dir.join("task.json"))?;
            if task.state != "running" || dir.join("cancel").exists() {
                bail!("任务已经结束或正在取消，不能再审批")
            };
            if req["resolved"] == true {
                bail!("审批已经处理")
            }
            if dir.join(format!("decision-{rid}.json")).exists() {
                bail!("审批已经提交")
            }
            write_json(
                &dir.join(format!("decision-{rid}.json")),
                &json!({"allow":v["allow"] == true}),
            )?;
            Ok(json!({"submitted":true}))
        }
        _ => bail!("远端 Runner 不支持此操作，请检查客户端与 Runner 版本"),
    }
}
fn start(v: Value) -> Result<Value> {
    let _guard = registry_guard()?;
    let id = field(&v, "task_id")?.to_owned();
    valid_id(&id)?;
    crate::models::validate(v["model"].as_str().unwrap_or(""))?;
    let dir = task_dir(&id)?;
    // Caller-generated UUID makes an uncertain SSH response safely recoverable.
    if dir.exists() {
        let t: Task = read_json(&dir.join("task.json"))?;
        if t.model != v["model"].as_str().unwrap_or("")
            || t.reply_to.as_deref() != v["reply_to"].as_str()
            || t.prompt != field(&v, "prompt")?
            || t.mode != field(&v, "mode")?
            || t.workspace != std::fs::canonicalize(field(&v, "workspace")?)?.to_string_lossy()
        {
            bail!("任务 ID 已用于不同参数")
        };
        drop(_guard);
        return snapshot(&id, 0);
    }
    let prompt = field(&v, "prompt")?.to_owned();
    if prompt.trim().is_empty() || prompt.len() > 256 * 1024 {
        bail!("任务描述不能为空，且不能超过 256 KiB")
    }
    let workspace = std::fs::canonicalize(field(&v, "workspace")?)?;
    if !workspace.is_dir() {
        bail!("工作区不是目录")
    }
    let workspace = workspace.to_string_lossy().to_string();
    let mode = field(&v, "mode")?.to_owned();
    if !["approval", "automatic"].contains(&mode.as_str()) {
        bail!("执行模式必须为 approval（审批）或 automatic（全自动）")
    }
    let mut session_id = uuid::Uuid::new_v4().to_string();
    let mut resume = false;
    if let Some(parent) = v["reply_to"].as_str() {
        let old: Task = read_json(&task_dir(parent)?.join("task.json"))?;
        if old.workspace != workspace {
            bail!("不能跨工作区续接会话")
        }
        if matches!(old.state.as_str(), "running" | "queued") {
            bail!("上一轮仍在运行，请先等待或取消")
        }
        if old.result.is_none() {
            bail!("上一轮没有有效 CC 会话结果，无法保证续接")
        }
        session_id = old.session_id;
        resume = true;
    }
    let databases = crate::databases::snapshot(&workspace, v["reply_to"].as_str())?;
    // One active task per canonical workspace; lock survives SSH disconnection.
    let lock = workspace_lock(&workspace)?;
    if lock.exists() {
        let owner = std::fs::read_to_string(&lock)?;
        if let Ok(t) = read_json::<Task>(&task_dir(owner.trim())?.join("task.json")) {
            if matches!(t.state.as_str(), "queued" | "running")
                && ((t.worker_pid == 0 && now().saturating_sub(t.created_at) < 15)
                    || alive(t.worker_pid))
            {
                bail!("工作区已有任务 {}，请先查询状态", t.id)
            }
        }
        std::fs::remove_file(&lock)?;
    }
    let mut f = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&lock)
        .context("工作区已被另一任务占用")?;
    f.write_all(id.as_bytes())?;
    private_dir(&dir)?;
    let mut t = Task {
        id: id.clone(),
        session_id,
        workspace,
        claude: field(&v, "claude")?.to_owned(),
        model: v["model"].as_str().unwrap_or("").into(),
        model_requested: v["model"].as_str().unwrap_or("").into(),
        model_source: v["model_source"].as_str().unwrap_or("legacy").into(),
        model_resolved: None,
        model_status: "pending".into(),
        observed_session_id: None,
        mode,
        prompt,
        resume,
        reply_to: v["reply_to"].as_str().map(String::from),
        state: "queued".into(),
        created_at: now(),
        updated_at: now(),
        worker_pid: 0,
        result: None,
        error: None,
    };
    crate::databases::save_snapshot(&dir, &databases)?;
    t.save()?;
    let log = OpenOptions::new()
        .create(true)
        .append(true)
        .open(dir.join("worker.log"))?;
    let mut cmd = Command::new(std::env::current_exe()?);
    cmd.args(["worker", &id])
        .stdin(Stdio::null())
        .stdout(log.try_clone()?)
        .stderr(log);
    unsafe {
        cmd.pre_exec(|| {
            if libc::setsid() < 0 {
                return Err(std::io::Error::last_os_error());
            }
            Ok(())
        });
    }
    match cmd.spawn() {
        Ok(child) => {
            t.worker_pid = child.id(); /* worker owns subsequent writes */
        }
        Err(e) => {
            t.state = "failed".into();
            t.error = Some(e.to_string());
            t.save()?;
            let _ = std::fs::remove_file(lock);
            return Err(e.into());
        }
    }
    Ok(
        json!({"task_id":id,"session_id":t.session_id,"state":"queued","mode":t.mode,"workspace":t.workspace}),
    )
}
fn registry_guard() -> Result<File> {
    use std::os::fd::AsRawFd;
    let f = OpenOptions::new()
        .create(true)
        .truncate(false)
        .write(true)
        .open(runner_dir()?.join("registry.lock"))?;
    if unsafe { libc::flock(f.as_raw_fd(), libc::LOCK_EX) } != 0 {
        return Err(std::io::Error::last_os_error().into());
    }
    Ok(f)
}
fn alive(pid: u32) -> bool {
    pid > 0 && unsafe { libc::kill(pid as i32, 0) == 0 }
}
fn workspace_lock(workspace: &str) -> Result<std::path::PathBuf> {
    // Reversible encoding, not a security digest; lock identity uses exact canonical path bytes.
    let name: String = workspace
        .as_bytes()
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect();
    let dir = runner_dir()?.join("locks");
    private_dir(&dir)?;
    // Very long paths use a directory registry instead of an oversized file name.
    let registry = dir.join("registry.json");
    let mut map: std::collections::BTreeMap<String, String> = if registry.exists() {
        read_json(&registry)?
    } else {
        Default::default()
    };
    if !map.contains_key(&name) {
        map.insert(name.clone(), uuid::Uuid::new_v4().to_string());
        write_json(&registry, &map)?;
    }
    Ok(dir.join(map[&name].clone()))
}
fn unlock(t: &Task) {
    let _guard = registry_guard().ok();
    if let Ok(p) = workspace_lock(&t.workspace) {
        if std::fs::read_to_string(&p).ok().as_deref() == Some(&t.id) {
            let _ = std::fs::remove_file(p);
        }
    }
}
pub fn worker(id: &str) -> Result<()> {
    let dir = task_dir(id)?;
    let mut t: Task = read_json(&dir.join("task.json"))?;
    t.worker_pid = std::process::id();
    t.state = "running".into();
    t.updated_at = now();
    t.save()?;
    let result = run_cc(&mut t, &dir);
    match result {
        Ok(()) => {}
        Err(e) => {
            t.state = "failed".into();
            t.error = Some(format!("{e:#}"));
        }
    }
    t.updated_at = now();
    t.save()?;
    unlock(&t);
    Ok(())
}
fn run_cc(t: &mut Task, dir: &Path) -> Result<()> {
    let exe = std::env::current_exe()?;
    let mut servers = crate::databases::mcp_servers(dir, &exe)?;
    servers.insert(
        "ccbridge_permissions".into(),
        json!({"command":exe,"args":["permission-server",t.id]}),
    );
    let config = json!({"mcpServers":servers});
    write_json(&dir.join("mcp.json"), &config)?;
    let mut cmd = Command::new(&t.claude);
    cmd.current_dir(&t.workspace)
        .args([
            "-p",
            "--output-format",
            "stream-json",
            "--verbose",
            "--strict-mcp-config",
        ])
        .arg("--mcp-config")
        .arg(dir.join("mcp.json"));
    if t.mode == "automatic" {
        cmd.arg("--dangerously-skip-permissions");
    } else {
        cmd.args([
            "--permission-mode",
            "default",
            "--permission-prompt-tool",
            "mcp__ccbridge_permissions__approve",
        ]);
    }
    if !t.model.is_empty() {
        cmd.args(["--model", &t.model]);
    }
    if t.resume {
        cmd.args(["--resume", &t.session_id]);
    } else {
        cmd.args(["--session-id", &t.session_id]);
    }
    cmd.arg("--append-system-prompt").arg("You are running through CCBridge. Work in the selected remote repository. Do not claim completion without evidence. Finish with a clear result, changed files, checks and unresolved issues. Do not deploy, publish or access unrelated resources unless the user's task authorizes it. Database tools named db_* come from this task's project mapping. Before querying, inspect their connection status and schema; use the correct purpose alias and never obtain credentials or switch to an unrelated database.");
    cmd.stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(File::create(dir.join("stderr.log"))?);
    cmd.process_group(0);
    let mut child = cmd.spawn().context("无法启动远端 CC")?;
    let mut input = child.stdin.take().unwrap();
    input.write_all(t.prompt.as_bytes())?;
    drop(input);
    let stdout = child.stdout.take().unwrap();
    let event_path = dir.join("events.jsonl");
    let reader = std::thread::spawn(move || -> Result<()> {
        let mut out = OpenOptions::new()
            .create(true)
            .append(true)
            .open(event_path)?;
        for line in BufReader::new(stdout).lines() {
            let line = line?;
            if line.len() > 2 * 1024 * 1024 {
                bail!("CC 单条事件超过 2 MiB，已停止读取")
            }
            writeln!(out, "{line}")?;
            out.flush()?;
        }
        Ok(())
    });
    let mut cancelled = false;
    let status = loop {
        if dir.join("cancel").exists() {
            cancelled = true;
            unsafe {
                libc::kill(-(child.id() as i32), libc::SIGTERM);
            }
            let deadline = std::time::Instant::now() + Duration::from_secs(3);
            let cancelled_status = loop {
                if let Some(s) = child.try_wait()? {
                    break s;
                }
                if std::time::Instant::now() > deadline {
                    unsafe {
                        libc::kill(-(child.id() as i32), libc::SIGKILL);
                    }
                    break child.wait()?;
                }
                std::thread::sleep(Duration::from_millis(50));
            };
            break cancelled_status;
        }
        if let Some(s) = child.try_wait()? {
            break s;
        }
        std::thread::sleep(Duration::from_millis(100));
    };
    reader
        .join()
        .map_err(|_| anyhow::anyhow!("CC 输出读取线程异常结束"))??;
    if let Ok(file) = File::open(dir.join("events.jsonl")) {
        for line in BufReader::new(file).lines() {
            if let Ok(v) = serde_json::from_str::<Value>(&line?) {
                if v["type"] == "system"
                    && v["subtype"] == "init"
                    && v["parent_tool_use_id"].is_null()
                {
                    if let Some(model) = v["model"].as_str() {
                        t.model_resolved = Some(model.into());
                        t.model_status = "reported".into();
                    }
                    if let Some(session) = v["session_id"].as_str() {
                        if session != t.session_id {
                            t.observed_session_id = Some(session.into());
                            t.error = Some("远端返回了不同的会话 ID，续聊一致性校验失败".into());
                        }
                    }
                }
                if v["type"] == "result" && v["parent_tool_use_id"].is_null() {
                    if let Some(session) = v["session_id"].as_str() {
                        if session != t.session_id {
                            t.observed_session_id = Some(session.into());
                            t.error = Some("最终结果的会话 ID 不匹配".into());
                        }
                    }
                    t.result = Some(v);
                }
            }
        }
    }
    if cancelled {
        t.state = "cancelled".into();
    } else if t.observed_session_id.is_some()
        || !status.success()
        || t.result.as_ref().is_none_or(|v| {
            v["is_error"] == true || v["subtype"].as_str().is_some_and(|s| s != "success")
        })
    {
        t.state = "failed".into();
        if t.error.is_none() {
            t.error = Some(format!(
                "CC exit={status}; {}",
                std::fs::read_to_string(dir.join("stderr.log"))
                    .unwrap_or_default()
                    .chars()
                    .take(4000)
                    .collect::<String>()
            ));
        }
    } else {
        t.state = "completed".into();
    }
    Ok(())
}
fn reconcile(t: &mut Task) -> Result<()> {
    if (t.state == "running" && !alive(t.worker_pid))
        || (t.state == "queued" && now().saturating_sub(t.created_at) > 15)
    {
        t.state = "interrupted".into();
        t.error = Some("Runner worker 已退出；不会自动重放任务".into());
        t.save()?;
        unlock(&t);
    }

    if t.model_resolved.is_none() {
        if let Ok(file) = File::open(task_dir(&t.id)?.join("events.jsonl")) {
            for line in BufReader::new(file).lines().take(80) {
                let line = line?;
                if line.len() > 512 * 1024 {
                    break;
                }
                if let Ok(v) = serde_json::from_str::<Value>(&line) {
                    if v["type"] == "system"
                        && v["subtype"] == "init"
                        && v["parent_tool_use_id"].is_null()
                    {
                        if let Some(model) = v["model"].as_str() {
                            t.model_resolved = Some(model.into());
                            t.model_status = "reported".into();
                        }
                        break;
                    }
                }
            }
        }
    }
    Ok(())
}
pub fn snapshot(id: &str, cursor: u64) -> Result<Value> {
    let dir = task_dir(id)?;
    let mut t: Task = read_json(&dir.join("task.json"))?;
    reconcile(&mut t)?;
    let mut approvals = vec![];
    for e in std::fs::read_dir(&dir)? {
        let e = e?;
        if e.file_name().to_string_lossy().starts_with("approval-") {
            if let Ok(v) = read_json::<Value>(&e.path()) {
                if v["resolved"] != true {
                    approvals.push(v);
                }
            }
        }
    }
    let mut events = vec![];
    let mut next = cursor;
    if let Ok(mut f) = File::open(dir.join("events.jsonl")) {
        use std::io::{Seek, SeekFrom};
        let len = f.metadata()?.len();
        if cursor > len {
            bail!("事件 cursor 超出文件长度")
        }
        f.seek(SeekFrom::Start(cursor))?;
        let mut r = BufReader::new(f);
        for _ in 0..80 {
            if next.saturating_sub(cursor) >= 512 * 1024 {
                break;
            }
            let mut line = String::new();
            if r.read_line(&mut line)? == 0 || !line.ends_with('\n') {
                break;
            }
            next += line.len() as u64;
            if let Ok(v) = serde_json::from_str::<Value>(&line) {
                events.push(v);
            }
        }
    }
    if t.state != "running" {
        approvals.clear();
    }
    let display_state = if !approvals.is_empty() && t.state == "running" {
        "waiting_approval"
    } else {
        &t.state
    };
    Ok(
        json!({"model":t.model,"model_requested":t.model_requested,"model_source":t.model_source,"model_resolved":t.model_resolved,"model_status":t.model_status,"observed_session_id":t.observed_session_id,"task_id":t.id,"session_id":t.session_id,"prompt":t.prompt,"created_at":t.created_at,"resume":t.resume,"reply_to":t.reply_to,"has_more":events.len()>=80 || next.saturating_sub(cursor)>=512*1024,"state":display_state,"mode":t.mode,"workspace":t.workspace,"result":t.result,"error":t.error,"events":events,"next_cursor":next,"approvals":approvals,"updated_at":t.updated_at}),
    )
}
