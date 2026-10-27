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

#[derive(Clone, Default, Serialize, Deserialize, Debug)]
#[serde(default)]
pub struct Task {
    #[serde(default = "write_access")]
    pub access: String,
    pub access_requested: Option<String>,
    pub policy_revision: u64,
    pub blocked_reason: Option<String>,
    pub id: String,
    #[serde(default)]
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
    #[serde(default)]
    pub diagnostic: bool,
    pub mode: String,
    pub prompt: String,
    pub resume: bool,
    #[serde(default)]
    pub reply_to: Option<String>,
    pub state: String,
    pub created_at: u64,
    pub updated_at: u64,
    pub worker_pid: u32,
    #[serde(default)]
    pub seq: u64,
    #[serde(default)]
    pub phase: String,
    #[serde(default = "reject_policy")]
    pub on_conflict: String,
    #[serde(default)]
    pub ownership_unknown: bool,
    pub result: Option<Value>,
    pub error: Option<String>,
}
fn write_access() -> String {
    "write".into()
}
fn reject_policy() -> String {
    "reject".into()
}
impl Task {
    fn save(&self) -> Result<()> {
        write_json(&task_dir(&self.id)?.join("task.json"), self)?;
        crate::scheduler::remember(self)
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
        "policy_get" => crate::policy::get(),
        "policy_set" => crate::policy::set(&v),
        "session_list" | "session_search" | "session_turns" | "session_meta_set"
        | "session_meta_get" | "active_tasks" => crate::sessions::rpc(&v),
        "git_update_plan"
        | "git_update_preflight"
        | "git_update_apply"
        | "git_update_begin"
        | "git_update_end" => crate::git_update::rpc(&v),
        "git_vps_auth_status" | "git_vps_auth_install" | "git_vps_auth_remove" => {
            crate::vps_auth::rpc(&v)
        }
        "file_create" | "file_mkdir" | "file_mutate" | "trash_list" | "trash_restore"
        | "branches" | "branch_change" => crate::workspace_ops::rpc(&v),
        "project_list" | "project_save" | "project_remove" | "project_resolve" => {
            crate::projects::rpc(&v)
        }
        "db_list" | "db_save" | "db_bind" | "db_remove" | "db_detect" | "db_test"
        | "db_protocol_test" | "db_project_save" | "db_projects" | "db_resolve" => {
            crate::databases::rpc(&v)
        }
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
                json!({"runner_version":env!("CARGO_PKG_VERSION"),"protocol_version":1,"capabilities":["models_v1","projects_v1","session_identity_v1","claude_info_v1","vps_auth_v1","sessions_v1","project_remove_v1","task_queue_v1","lease_v1","analysis_v1","policy_v1"],"workspace":work,"claude_version":version,"claude_error":if version.is_none(){Some("当前账号未找到可运行的 Claude，请在高级设置填写绝对路径，或在远端安装并登录")}else{None},"account_home":home()?,"state_directory":runner_dir()?,"code_access":"remote_directory","warning":"工作目录不是 OS 沙箱；全自动模式拥有该 SSH 账号的文件与命令权限"}),
            )
        }
        "diagnostic_workspace" => {
            let p = runner_dir()?.join(format!("diagnostic-{}", uuid::Uuid::new_v4()));
            private_dir(&p)?;
            Ok(json!({"path":p}))
        }
        "claude_models" => crate::claude_info::models(field(&v, "claude")?),
        "claude_usage" => {
            crate::claude_info::usage(v["claude"].as_str().unwrap_or(""), &runner_dir()?)
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
            let mut first_prompts = std::collections::BTreeMap::new();
            for t in &tasks {
                first_prompts.insert(t.session_id.clone(), t.prompt.clone());
            }
            tasks.truncate(100);
            Ok(
                json!({"tasks":tasks.into_iter().map(|t| json!({"access":t.access,"access_requested":t.access_requested,"policy_revision":t.policy_revision,"blocked_reason":t.blocked_reason,"phase":t.phase,"ownership_unknown":t.ownership_unknown,"queue":crate::scheduler::queue(&t).unwrap_or(Value::Null),"session_meta":crate::sessions::task_metadata(&t.session_id,first_prompts.get(&t.session_id).map(String::as_str).unwrap_or(&t.prompt)),"id":t.id,"session_id":t.session_id,"state":if t.ownership_unknown{"interrupted"}else{&t.state},"mode":t.mode,"workspace":t.workspace,"created_at":t.created_at,"updated_at":t.updated_at,"model_requested":t.model_requested,"model_resolved":t.model_resolved,"title":t.prompt.chars().take(64).collect::<String>(),"resume":t.resume,"reply_to":t.reply_to})).collect::<Vec<_>>()}),
            )
        }
        "task_release" => {
            let _guard = registry_guard()?;
            let dir = task_dir(field(&v, "task_id")?)?;
            let mut t: Task = read_json(&dir.join("task.json"))?;
            if v["confirmed"] != true || !t.ownership_unknown {
                bail!("只能确认释放状态未知的任务")
            }
            let _free =
                crate::scheduler::worker_lock(&t.id)?.context("任务执行进程仍在运行，不能释放")?;
            let marker = read_json::<Value>(&dir.join("spawn-marker")).unwrap_or(Value::Null);
            if let Some(pgid) = marker["pgid"].as_i64() {
                let rc = unsafe { libc::kill(-(pgid as i32), 0) };
                if rc == 0 || std::io::Error::last_os_error().raw_os_error() != Some(libc::ESRCH) {
                    return Err(coded(
                        "process_group_alive",
                        "仍检测到原任务进程组或无法确认，请先确认进程结束后重新检查",
                    ));
                }
            }
            write_json(
                &dir.join("release-audit.json"),
                &json!({"at":now(),"reason":"human_confirmed_stopped","task_id":t.id,"process_evidence":marker}),
            )?;
            t.ownership_unknown = false;
            t.state = "interrupted".into();
            t.updated_at = now();
            t.error = Some("已由用户确认结束并释放占用；未删除文件或历史会话".into());
            t.save()?;
            Ok(json!({"released":true,"task_id":t.id}))
        }
        "cancel" => {
            let dir = task_dir(field(&v, "task_id")?)?;
            let _guard = registry_guard()?;
            let mut t: Task = read_json(&dir.join("task.json"))?;
            if t.ownership_unknown {
                return Err(coded(
                    "ownership_unknown",
                    "任务状态未知，请先重新检查，不能假称已取消",
                ));
            }
            if t.state == "queued" && t.phase == "waiting" {
                t.state = "cancelled".into();
                t.updated_at = now();
                t.save()?;
            } else if t.state == "running" && !t.ownership_unknown {
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
    crate::scheduler::check_access(&v)?;
    let conflict_policy = v["on_conflict"].as_str().unwrap_or("reject");
    if !["reject", "queue"].contains(&conflict_policy) {
        bail!("on_conflict 必须为 reject 或 queue")
    };
    let _guard = registry_guard()?;
    let id = field(&v, "task_id")?.to_owned();
    valid_id(&id)?;
    crate::models::validate(v["model"].as_str().unwrap_or(""))?;
    let dir = task_dir(&id)?;
    // Caller-generated UUID makes an uncertain SSH response safely recoverable.
    if dir.exists() {
        let t: Task = read_json(&dir.join("task.json"))?;
        if t.access_requested.as_deref() != v["access"].as_str().filter(|s| !s.is_empty())
            || t.diagnostic != (v["diagnostic"] == true)
            || t.model != v["model"].as_str().unwrap_or("")
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
    let mut parent_access = None;
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
        valid_id(&old.session_id).context("旧记录缺少有效会话标识，请新建任务")?;
        parent_access = Some(old.access.clone());
        session_id = old.session_id;
        resume = true;
    }
    let policy = crate::policy::read()?;
    let access = crate::policy::scope(&policy, v["access"].as_str(), parent_access.as_deref())?;
    let databases = if access == "analysis" {
        Vec::new()
    } else {
        crate::databases::snapshot(&workspace, v["reply_to"].as_str())?
    };
    let tasks = crate::scheduler::tasks()?;
    if tasks
        .iter()
        .any(|t| t.session_id == session_id && crate::scheduler::pending(t))
    {
        return Err(coded(
            "session_busy",
            "此会话已有一轮在排队或运行，请结束后再续聊",
        ));
    }
    if conflict_policy == "queue"
        && tasks
            .iter()
            .filter(|t| t.state == "queued" && t.phase == "waiting")
            .count()
            >= 32
    {
        return Err(coded(
            "queue_full",
            "排队任务已达32个，请等待或取消不需要的任务",
        ));
    }
    let blocked = crate::scheduler::blockers_for(Path::new(&workspace), "", &access)?;
    let earlier = tasks.iter().any(|t| {
        t.state == "queued"
            && crate::scheduler::conflicts(&access, &t.access)
            && crate::scheduler::overlaps(Path::new(&workspace), Path::new(&t.workspace))
    });
    if conflict_policy == "reject" && (!blocked.is_empty() || earlier) {
        return Err(crate::scheduler::busy(blocked));
    }
    let seq = crate::scheduler::next_sequence()?;
    private_dir(&dir)?;
    let mut t = Task {
        access,
        access_requested: v["access"]
            .as_str()
            .filter(|s| !s.is_empty())
            .map(str::to_owned),
        policy_revision: policy.revision,
        blocked_reason: None,
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
        diagnostic: v["diagnostic"] == true,
        mode,
        prompt,
        resume,
        reply_to: v["reply_to"].as_str().map(String::from),
        state: "queued".into(),
        created_at: now(),
        updated_at: now(),
        worker_pid: 0,
        seq,
        phase: "waiting".into(),
        on_conflict: conflict_policy.into(),
        ownership_unknown: false,
        result: None,
        error: None,
    };
    crate::databases::save_snapshot(&dir, &databases)?;
    t.save()?;
    launch_worker(&mut t)?;
    Ok(
        json!({"task_id":id,"session_id":t.session_id,"state":"queued","access":t.access,"mode":t.mode,"workspace":t.workspace,"phase":t.phase,"queue":crate::scheduler::queue(&t)?}),
    )
}
fn launch_worker(t: &mut Task) -> Result<()> {
    let log = OpenOptions::new()
        .create(true)
        .append(true)
        .open(task_dir(&t.id)?.join("worker.log"))?;
    let mut cmd = Command::new(std::env::current_exe()?);
    cmd.args(["worker", &t.id])
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

            return Err(e.into());
        }
    }
    t.updated_at = now();
    t.save()?;
    Ok(())
}
fn registry_guard() -> Result<File> {
    crate::scheduler::registry()
}
fn unlock(_t: &Task) {}
pub fn worker(id: &str) -> Result<()> {
    let dir = task_dir(id)?;
    let deadline = std::time::Instant::now() + Duration::from_secs(2);
    let _worker = loop {
        if let Some(f) = crate::scheduler::worker_lock(id)? {
            break f;
        }
        if std::time::Instant::now() >= deadline {
            bail!("任务已有执行进程")
        }
        std::thread::sleep(Duration::from_millis(50));
    };
    let mut t;
    loop {
        let guard = registry_guard()?;
        t = read_json::<Task>(&dir.join("task.json"))?;
        if t.state != "queued" {
            return Ok(());
        }
        if dir.join("cancel").exists() {
            t.state = "cancelled".into();
            t.updated_at = now();
            t.save()?;
            return Ok(());
        }
        let policy = crate::policy::read()?;
        if t.access == "write" && policy.read_only && !t.diagnostic {
            if t.blocked_reason.as_deref() != Some("policy") {
                t.blocked_reason = Some("policy".into());
                t.updated_at = now();
                t.save()?;
            }
            drop(guard);
            std::thread::sleep(Duration::from_millis(400));
            continue;
        }
        if t.blocked_reason.take().is_some() {
            t.updated_at = now();
            t.save()?;
        }
        t.policy_revision = policy.revision;
        let queue = crate::scheduler::queue(&t)?;
        if queue["blocked_by"].as_array().is_some_and(|b| b.is_empty()) {
            t.phase = "executing".into();
            t.worker_pid = std::process::id();
            t.state = "running".into();
            t.updated_at = now();
            t.save()?;
            break;
        }
        drop(guard);
        std::thread::sleep(Duration::from_millis(400));
    }
    let result = run_cc(&mut t, &dir);
    match result {
        Ok(()) => {}
        Err(e) => {
            // Returning an I/O error does not prove that a spawned child tree is gone.
            let marker = dir.join("spawn-marker");
            t.ownership_unknown = marker.exists() && !process_group_ended(&marker);
            t.state = if t.ownership_unknown {
                "running"
            } else {
                "failed"
            }
            .into();
            t.error = Some(format!("{e:#}"));
        }
    }
    let _guard = registry_guard()?;
    t.updated_at = now();
    t.save()?;
    unlock(&t);
    Ok(())
}
fn process_group_ended(marker: &Path) -> bool {
    let v = read_json::<Value>(marker).unwrap_or(Value::Null);
    let Some(pgid) = v["pgid"]
        .as_i64()
        .filter(|p| *p > 0 && *p <= i32::MAX as i64)
    else {
        return false;
    };
    unsafe {
        libc::kill(-(pgid as i32), 0) < 0
            && std::io::Error::last_os_error().raw_os_error() == Some(libc::ESRCH)
    }
}
fn run_cc(t: &mut Task, dir: &Path) -> Result<()> {
    let exe = std::env::current_exe()?;
    let analysis = t.access == "analysis" && !t.diagnostic;
    if analysis {
        crate::analysis::preflight(&t.claude, Path::new(&t.workspace))?;
    }
    let mut servers = if analysis {
        serde_json::Map::new()
    } else {
        crate::databases::mcp_servers(dir, &exe)?
    };
    servers.insert(
        "ccbridge_permissions".into(),
        json!({"command":exe,"args":["permission-server",t.id]}),
    );
    if t.diagnostic || analysis {
        servers.clear();
    }
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
    if t.diagnostic {
        cmd.args(["--tools", ""]);
    }
    if analysis {
        cmd.args(crate::analysis::arguments());
    } else if t.diagnostic {
    } else if t.mode == "automatic" {
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
    cmd.arg("--append-system-prompt").arg(if analysis {"Analyze the current workspace using only read and search tools. Do not modify project files, run commands, delegate tasks, invoke external tools or access credentials. Other sessions may change these files while you analyze. Return findings and file locations to Codex, not edits. CLI login/session maintenance and administrator scripts are outside this project-file tool policy."} else {"You are running through CCBridge. Work in the selected remote repository. Do not claim completion without evidence. Finish with a clear result, changed files, checks and unresolved issues. Do not deploy, publish or access unrelated resources unless the user's task authorizes it. Database tools named db_* come from this task's project mapping. Before querying, inspect their connection status and schema; use the correct purpose alias and never obtain credentials or switch to an unrelated database."});
    cmd.stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(File::create(dir.join("stderr.log"))?);
    cmd.process_group(0);
    std::fs::write(dir.join("spawn-marker"), b"starting")?;
    let mut child = match cmd.spawn() {
        Ok(c) => c,
        Err(e) => {
            let _ = std::fs::remove_file(dir.join("spawn-marker"));
            return Err(e).context("无法启动远端 CC");
        }
    };
    let group = child.id() as i32;
    let (reader_tx, reader_rx) = std::sync::mpsc::channel::<String>();
    write_json(
        &dir.join("spawn-marker"),
        &json!({"pgid":child.id(),"at":now()}),
    )?;
    let mut input = child.stdin.take().unwrap();
    input.write_all(t.prompt.as_bytes())?;
    drop(input);
    let stdout = child.stdout.take().unwrap();
    let event_path = dir.join("events.jsonl");
    let reader = std::thread::spawn(move || -> Result<()> {
        let result = (|| -> Result<()> {
            let mut verified = !analysis;
            let mut out = OpenOptions::new()
                .create(true)
                .append(true)
                .open(event_path)?;
            for line in BufReader::new(stdout).lines() {
                let line = line?;
                if line.len() > 2 * 1024 * 1024 {
                    bail!("CC 单条事件超过 2 MiB，已停止读取")
                }
                // Timestamp only quota events at receipt, not when a later task ends.
                // Other protocol frames remain byte-for-byte unchanged.
                if let Ok(mut event) = serde_json::from_str::<Value>(&line) {
                    if analysis
                        && event["type"] == "system"
                        && event["subtype"] == "init"
                        && event["parent_tool_use_id"].is_null()
                    {
                        if let Err(e) = crate::analysis::verify_runtime()
                            .and_then(|_| crate::analysis::verify_init(&event))
                        {
                            let _ = reader_tx.send(e.to_string());
                            unsafe {
                                libc::kill(-group, libc::SIGTERM);
                            }
                            return Err(e);
                        }
                        verified = true;
                    }
                    if analysis
                        && !verified
                        && ["assistant", "result"].contains(&event["type"].as_str().unwrap_or(""))
                    {
                        let e = coded(
                            "analysis_unsupported",
                            "Claude 未返回可验证的分析初始化，任务已停止",
                        );
                        let _ = reader_tx.send(e.to_string());
                        unsafe {
                            libc::kill(-group, libc::SIGTERM);
                        }
                        return Err(e);
                    }
                    if event["type"] == "rate_limit_event" {
                        event["_ccbridge_observed_at"] = json!(now());
                        writeln!(out, "{event}")?;
                    } else {
                        writeln!(out, "{line}")?;
                    }
                } else {
                    writeln!(out, "{line}")?;
                }
                out.flush()?;
            }
            Ok(())
        })();
        if let Err(e) = &result {
            let _ = reader_tx.send(e.to_string());
        }
        result
    });
    let mut cancelled = false;
    let status = loop {
        if let Ok(error) = reader_rx.try_recv() {
            t.error = Some(error);
        }
        if dir.join("cancel").exists() || t.error.is_some() {
            cancelled = dir.join("cancel").exists();
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
            let group_deadline = std::time::Instant::now() + Duration::from_secs(3);
            while !process_group_ended(&dir.join("spawn-marker"))
                && std::time::Instant::now() < group_deadline
            {
                std::thread::sleep(Duration::from_millis(50));
            }
            if !process_group_ended(&dir.join("spawn-marker")) {
                unsafe {
                    libc::kill(-(child.id() as i32), libc::SIGKILL);
                }
            }
            break cancelled_status;
        }
        if let Some(s) = child.try_wait()? {
            break s;
        }
        std::thread::sleep(Duration::from_millis(100));
    };
    let reader_deadline = std::time::Instant::now() + Duration::from_secs(2);
    while !reader.is_finished() {
        if std::time::Instant::now() >= reader_deadline {
            return Err(coded(
                "ownership_unknown",
                "Claude 输出仍被子进程占用，请重新检查任务",
            ));
        }
        std::thread::sleep(Duration::from_millis(50));
    }
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
    if !process_group_ended(&dir.join("spawn-marker")) {
        return Err(coded(
            "ownership_unknown",
            "Claude 主进程已退出，但子进程仍在运行；保留工作区占用，请重新检查任务",
        ));
    }
    if analysis {
        crate::analysis::verify_runtime()?;
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
    {
        let _guard = registry_guard()?;
        *t = read_json(&task_dir(&t.id)?.join("task.json"))?;
        let legacy_live = t.phase.is_empty()
            && t.worker_pid > 0
            && unsafe { libc::kill(t.worker_pid as i32, 0) == 0 };
        if !legacy_live && ["running", "queued"].contains(&t.state.as_str()) {
            if let Some(free) = crate::scheduler::worker_lock(&t.id)? {
                if t.phase.is_empty() || task_dir(&t.id)?.join("spawn-marker").exists() {
                    t.ownership_unknown = true;
                    t.error = Some("未能确认 Claude 已结束，目录仍被占用".into());
                    t.save()?;
                } else if t.state == "queued"
                    && t.phase == "waiting"
                    && now().saturating_sub(t.updated_at) > 15
                {
                    drop(free);
                    launch_worker(t)?;
                } else if t.phase != "waiting" && now().saturating_sub(t.created_at) > 15 {
                    t.state = "interrupted".into();
                    t.error = Some("任务未启动，执行进程已退出".into());
                    t.save()?;
                }
            }
        }
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
    let display_state = if t.ownership_unknown {
        "interrupted"
    } else if !approvals.is_empty() && t.state == "running" {
        "waiting_approval"
    } else {
        &t.state
    };
    Ok(
        json!({"access":t.access,"access_requested":t.access_requested,"policy_revision":t.policy_revision,"blocked_reason":t.blocked_reason,"phase":t.phase,"ownership_unknown":t.ownership_unknown,"queue":crate::scheduler::queue(&t)?,"database_snapshot":crate::databases::task_summary(&dir, &t.workspace)?,"model":t.model,"model_requested":t.model_requested,"model_source":t.model_source,"model_resolved":t.model_resolved,"model_status":t.model_status,"observed_session_id":t.observed_session_id,"task_id":t.id,"session_id":t.session_id,"prompt":t.prompt,"created_at":t.created_at,"resume":t.resume,"reply_to":t.reply_to,"has_more":events.len()>=80 || next.saturating_sub(cursor)>=512*1024,"state":display_state,"mode":t.mode,"workspace":t.workspace,"result":t.result,"error":t.error,"events":events,"next_cursor":next,"approvals":approvals,"updated_at":t.updated_at}),
    )
}
