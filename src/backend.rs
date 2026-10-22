use crate::{common::*, ssh};
use anyhow::{Context, Result, bail};
use serde_json::{Value, json};
use std::sync::{Arc, mpsc};
use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};
use tokio::sync::Mutex;

pub struct BackendState {
    pub connection: Mutex<Option<ssh::Connection>>,
    pub operations: crate::operations::Store,
    pub clone_progress: std::sync::Mutex<Value>,
    pub gate: std::sync::Mutex<crate::update_gate::UpdateGate>,
    pub writes: Mutex<()>,
    pub auto_attempted: std::sync::atomic::AtomicBool,
}
impl BackendState {
    pub fn new(connection: Option<ssh::Connection>) -> Self {
        Self {
            connection: Mutex::new(connection),
            operations: Default::default(),
            clone_progress: std::sync::Mutex::new(json!({})),
            gate: Default::default(),
            writes: Mutex::new(()),
            auto_attempted: std::sync::atomic::AtomicBool::new(false),
        }
    }
}
/// Validate the exact persisted startup target, never an unsaved UI draft.
fn validate_auto_target_at(root: &std::path::Path, p: &Profile) -> Result<()> {
    if !p.remember_password || p.fingerprint.is_empty() {
        bail!("自动连接需要已保存密码和已确认指纹，请在服务器页设置")
    }
    if p.credential_store != "file" {
        bail!("请先在服务器页将此账号的旧密码迁移到本机保存，再开启自动连接")
    }
    let key = format!("dev.ccbridge.desktop.ssh:{}", p.account_key());
    if !crate::secret_store::load_at(root, &key)?.is_some_and(|s| !s.is_empty()) {
        bail!("此账号尚未在本机保存密码，请在服务器页保存后再开启自动连接")
    }
    Ok(())
}
fn auto_target(p: &Profile) -> Value {
    json!({"user":p.user,"host":p.host,"port":p.port})
}
fn ensure_update_tasks_idle(value: &Value) -> Result<()> {
    let tasks = value["tasks"]
        .as_array()
        .context("无法读取远端任务状态，请刷新会话后重试安装")?;
    let active: Vec<_> = tasks
        .iter()
        .filter(|t| {
            ["queued", "running", "waiting_approval"].contains(&t["state"].as_str().unwrap_or(""))
        })
        .collect();
    if !active.is_empty() {
        let names = active
            .iter()
            .take(3)
            .map(|t| {
                let title = t["title"]
                    .as_str()
                    .filter(|s| !s.is_empty())
                    .unwrap_or("未命名任务");
                let short: String = title.chars().take(36).collect();
                let state = match t["state"].as_str().unwrap_or("") {
                    "waiting_approval" => "等待审批",
                    "queued" => "排队中",
                    _ => "执行中",
                };
                format!("{short}（{state}）")
            })
            .collect::<Vec<_>>()
            .join("；");
        bail!(
            "暂不能安装：远端有 {} 个未结束任务：{}。请在“会话”中处理完成后重试。",
            active.len(),
            names
        );
    }
    Ok(())
}
pub type Shared = Arc<BackendState>;
struct Activity(Shared, String);
impl Drop for Activity {
    fn drop(&mut self) {
        self.0.gate.lock().unwrap().finish(&self.1);
    }
}
pub async fn dispatch(state: Shared, v: Value, human: bool) -> Result<Value> {
    let operation_name = field(&v, "op")?.to_owned();
    let op = operation_name.as_str();
    if op == "prepare_install" {
        if !human {
            bail!("安装只能由人在App操作")
        }
        if state.connection.lock().await.is_some() {
            return Box::pin(dispatch(state, json!({"op":"prepare_update"}), true)).await;
        }
        state.gate.lock().unwrap().prepare()?;
        return Ok(state.gate.lock().unwrap().snapshot());
    }
    if op == "auto_connect" {
        if !human {
            bail!("自动连接只能由App启动触发")
        }
        if state
            .auto_attempted
            .swap(true, std::sync::atomic::Ordering::SeqCst)
        {
            return Ok(json!({"skipped":true,"reason":"本次启动已尝试连接"}));
        }
        let prefs_path = app_dir()?.join("preferences.json");
        let prefs: Value = if prefs_path.exists() {
            read_json(&prefs_path)?
        } else {
            json!({})
        };
        if prefs["auto_connect"] != true {
            return Ok(json!({"skipped":true}));
        }
        let p: Profile = read_json(&app_dir()?.join("profile.json"))?;
        validate_auto_target_at(&app_dir()?, &p)?;
        return Box::pin(dispatch(
            state,
            json!({"op":"connect","profile":p,"password":""}),
            true,
        ))
        .await;
    }

    if !human && v["client_protocol"] != 1 && ["status", "result"].contains(&op) {
        let mut gate = state.gate.lock().unwrap();
        if gate.draining {
            bail!("App 正在准备更新，请更新完成后查询原任务")
        }
        // Old clients cannot announce a long call. Retain their maximum wait + transport timeout.
        gate.legacy_until = gate
            .legacy_until
            .max(now() + if op == "result" { 990 } else { 120 });
    }

    if [
        "client_activity_begin",
        "client_activity_renew",
        "client_activity_end",
    ]
    .contains(&op)
    {
        return state.gate.lock().unwrap().lease(op, field(&v, "lease_id")?);
    }
    if ["safe_update_state", "prepare_update", "cancel_update"].contains(&op) {
        if !human {
            bail!("更新安装只能在 App 操作")
        }
        if op == "cancel_update" {
            state.gate.lock().unwrap().draining = false;
            return Ok(json!({"draining":false}));
        }
        if op == "prepare_update" {
            #[cfg(target_os = "macos")]
            if crate::credentials::pending() {
                bail!("钥匙串授权尚未结束，请先处理系统弹窗再更新")
            }

            state.gate.lock().unwrap().prepare()?;
            let checked = async {
                let connection = state.connection.lock().await.clone();
                if let Some(c) = connection.filter(|c| !c.handle.is_closed()) {
                    let tasks = c
                        .rpc(json!({"op":"list"}))
                        .await
                        .context("无法检查远端任务，请刷新连接状态后重试安装")?;
                    ensure_update_tasks_idle(&tasks)?;
                }
                // Updating the local App does not stop detached remote Runner tasks.
                // Offline installation still observes the local operation/MCP drain barrier.
                Ok::<(), anyhow::Error>(())
            }
            .await;
            if let Err(e) = checked {
                state.gate.lock().unwrap().draining = false;
                return Err(e);
            }
        }
        return Ok(state.gate.lock().unwrap().snapshot());
    }
    if human && op == "clone_into_directory" {
        *state.clone_progress.lock().unwrap() =
            json!({"stage":"等待后台执行克隆", "completed":0,"total":0});
    }
    let writing = (human && op == "repository_match")
        || [
            "cc_diagnostic",
            "db_save",
            "db_bind",
            "db_remove",
            "db_project_save",
            "connect",
            "disconnect",
            "save_profile",
            "migrate_password",
            "preferences_save",
            "load_profile",
            "forget_password",
            "load_password",
            "configure_codex",
            "start",
            "reply",
            "clone_into_directory",
            "prepare_repositories",
            "install_runner",
            "refresh_runner",
            "select_remote_workspace",
            "create_directory",
            "git_auth_save",
            "git_auth_migrate",
            "git_auth_remove",
            "file_create",
            "file_mutate",
            "trash_restore",
            "branch_change",
            "branch_fetch",
            "git_update",
            "git_vps_auth_install",
            "git_vps_auth_remove",
            "project_save",
            "set_auto",
            "bind_repository",
            "clear_binding",
        ]
        .contains(&op);
    let _write = if writing {
        Some(state.writes.try_lock().map_err(|_| {
            anyhow::anyhow!("另一项写入操作仍在执行，请等待完成；状态查询与审批不受影响")
        })?)
    } else {
        None
    };
    let _activity = if writing || ["db_test", "db_protocol_test"].contains(&op) {
        state.gate.lock().unwrap().begin(op)?;
        Some(Activity(state.clone(), op.to_string()))
    } else {
        None
    };
    let mut v = v;
    let tracked = [
        "git_update",
        "clone_into_directory",
        "branch_fetch",
        "install_runner",
    ]
    .contains(&op);
    let operation = if tracked {
        let c = state
            .connection
            .lock()
            .await
            .clone()
            .context("请先连接 SSH")?;
        let id = v["operation_id"]
            .as_str()
            .map(str::to_string)
            .unwrap_or_else(|| uuid::Uuid::new_v4().to_string());
        uuid::Uuid::parse_str(&id).context("操作标识无效")?;
        if !crate::operations::snapshot(
            &state.operations,
            &c.profile.account_key(),
            &c.connection_id,
            Some(&id),
            None,
            None,
        )
        .is_null()
        {
            bail!("此操作已发起，请查询原操作结果")
        }
        v["operation_id"] = json!(id);
        Some(crate::operations::Operation::begin(
            state.operations.clone(),
            id,
            op,
            &c.profile.account_key(),
            &c.connection_id,
            v["path"].as_str().unwrap_or(""),
            human,
        ))
    } else {
        None
    };
    let mut result = dispatch_inner(state.clone(), v, human).await;
    if let Some(o) = operation {
        o.finish(&result);
        if let Ok(r) = &mut result {
            r["operation_id"] = json!(o.id);
        }
    }
    result
}

pub struct Job {
    pub value: Value,
    pub reply: mpsc::Sender<Value>,
    pub background: bool,
}
async fn dispatch_inner(state: Shared, v: Value, human: bool) -> Result<Value> {
    let op = field(&v, "op")?.to_owned();
    if op == "preferences_save" {
        if !human {
            bail!("只能在App修改启动偏好")
        }
        if v["auto_connect"] == true {
            let p: Profile = read_json(&app_dir()?.join("profile.json"))?;
            validate_auto_target_at(&app_dir()?, &p)?;
        }
        write_json(
            &app_dir()?.join("preferences.json"),
            &json!({"auto_connect":v["auto_connect"]==true}),
        )?;
        let p: Profile = read_json(&app_dir()?.join("profile.json")).unwrap_or_default();
        return Ok(json!({"saved":true,"auto_target":auto_target(&p)}));
    }
    if op == "clone_progress" {
        return Ok(state.clone_progress.lock().unwrap().clone());
    }
    if [
        "git_auth_status",
        "git_auth_save",
        "git_auth_migrate",
        "git_auth_remove",
        "git_access",
    ]
    .contains(&op.as_str())
    {
        if !human {
            bail!("Git 认证只能在 App 操作")
        }
        let scope = v["scope"].as_str().unwrap_or("local");
        let p: Profile = if scope == "remote" {
            state
                .connection
                .lock()
                .await
                .as_ref()
                .context("远端认证请先连接 SSH")?
                .profile
                .clone()
        } else {
            serde_json::from_value(v["profile"].clone()).unwrap_or_default()
        };
        if op == "git_auth_status" {
            let scope = scope.to_string();
            return tokio::task::spawn_blocking(move || {
                crate::git_auth::check_configuration(&scope, &p)
            })
            .await?;
        }
        if op == "git_auth_migrate" {
            crate::secret_store::confirm_consent(v["storage_ack"] == true)?;
            return crate::git_auth::migrate(scope, &p).await;
        }
        if op == "git_auth_remove" {
            return crate::git_auth::remove(scope, &p).await;
        }
        let draft = v["secret"]
            .as_str()
            .filter(|s| !s.is_empty())
            .map(|secret| crate::git_auth::Credential {
                username: v["username"].as_str().unwrap_or("").into(),
                secret: secret.into(),
            });
        if let Some(c) = &draft {
            c.validate()?
        }
        if op == "git_auth_save" {
            crate::secret_store::confirm_consent(v["storage_ack"] == true)?;
            return crate::git_auth::save(
                scope,
                &p,
                draft.context("请输入要保存的 Codeup 密码 / 令牌")?,
                "file",
            )
            .await;
        }
        let url = field(&v, "url")?.to_string();
        crate::git_auth::host(&url)?;
        let cred = match draft {
            Some(c) => Some(c),
            None => crate::git_auth::load(scope, &p).await?,
        };
        if scope == "remote" {
            let c = state
                .connection
                .lock()
                .await
                .clone()
                .context("请先连接 SSH")?;
            return c.git_access(&url, cred).await;
        }
        tokio::task::spawn_blocking(move || crate::transfer::access(&url, cred.as_ref())).await??;
        return Ok(json!({"accessible":true,"message":"本机 Git 已通过仓库读取检测"}));
    }
    if op == "configure_codex" {
        if !human {
            bail!("只能由人在 App 配置 Codex")
        }
        return Ok(
            json!({"message":tokio::task::spawn_blocking(crate::integration::configure).await??}),
        );
    }
    if op == "migrate_password" {
        if !human {
            bail!("密码迁移只能由人在App明确操作")
        }
        let profiles: std::collections::BTreeMap<String, Profile> =
            read_json(&app_dir()?.join("profiles.json"))?;
        let account = field(&v, "account_key")?;
        let mut p = profiles
            .get(account)
            .cloned()
            .context("请先保存此账号配置")?;
        if !p.remember_password {
            bail!("此账号没有已保存密码，请在App输入")
        }
        crate::secret_store::confirm_consent(v["storage_ack"] == true)?;
        if p.credential_store == "file" {
            return Ok(json!({"saved":true,"profile":p,"message":"已使用本机保存，无需重复迁移"}));
        }
        #[cfg(target_os = "macos")]
        {
            let password =
                crate::secret_store::get("keychain", "dev.ccbridge.desktop.ssh", account)
                    .await
                    .context("读取原钥匙串密码失败；原保存方式未改变")?;
            crate::secret_store::set("file", "dev.ccbridge.desktop.ssh", account, password)
                .await
                .context("本机保存失败；原钥匙串与账号引用未改变")?;
            p.credential_store = "file".into();
            p.save()?;
            if let Some(c) = state.connection.lock().await.as_mut() {
                if c.profile.account_key() == account {
                    c.profile.credential_store = "file".into();
                }
            }
            return Ok(
                json!({"saved":true,"profile":p,"message":"当前账号已改为本机保存，原钥匙串副本保留"}),
            );
        }
        #[cfg(not(target_os = "macos"))]
        bail!("此迁移仅在Mac App可用")
    }
    if ["save_profile", "load_password", "forget_password"].contains(&op.as_str()) {
        if !human {
            bail!("凭据只能在 App 管理")
        }
        let mut p: Profile = serde_json::from_value(v["profile"].clone())?;
        p.credential_store = "file".into();
        p.validate()?;
        if p.credential_store == "file" && v["password"].as_str().is_some_and(|s| !s.is_empty()) {
            crate::secret_store::confirm_consent(v["storage_ack"] == true)?;
        }
        #[cfg(target_os = "macos")]
        {
            let account = p.account_key();
            let store = if p.credential_store.is_empty() {
                "keychain"
            } else {
                &p.credential_store
            };
            let password = v["password"].as_str().unwrap_or("");
            if op == "load_password" {
                return Ok(
                    json!({"password":crate::secret_store::get(store,"dev.ccbridge.desktop.ssh",&account).await?}),
                );
            }
            if op == "forget_password" || !p.remember_password {
                crate::secret_store::forget("dev.ccbridge.desktop.ssh", &account).await?;
                if op == "forget_password" {
                    p.remember_password = false;
                    let root = app_dir()?;
                    let active: Profile = read_json(&root.join("profile.json"))?;
                    if active.account_key() == account {
                        write_json(&root.join("profile.json"), &p)?;
                    }
                    let mut all: std::collections::BTreeMap<String, Profile> =
                        read_json(&root.join("profiles.json")).unwrap_or_default();
                    if let Some(saved) = all.get_mut(&account) {
                        saved.remember_password = false;
                    }
                    write_json(&root.join("profiles.json"), &all)?;
                    if let Some(c) = state.connection.lock().await.as_mut() {
                        if c.profile.account_key() == account {
                            c.profile.remember_password = false;
                        }
                    }
                    return Ok(json!({"deleted":true,"auto_target":auto_target(&p)}));
                }
            } else if !password.is_empty() {
                crate::secret_store::set(
                    store,
                    "dev.ccbridge.desktop.ssh",
                    &account,
                    password.into(),
                )
                .await?;
            } else {
                let previous: Profile = read_json(&app_dir()?.join("profile.json"))?;
                if !previous.remember_password
                    || previous.account_key() != account
                    || previous.credential_store != p.credential_store
                {
                    bail!("切换密码保存方式或账号时，请重新输入密码；不会自动导出原密码")
                }
            }
        }
        p.save()?;
        let target = auto_target(&p);
        if let Some(c) = state.connection.lock().await.as_mut() {
            if c.profile.account_key() == p.account_key() {
                c.profile.name = p.name;
                c.profile.model = p.model;
                c.profile.remember_password = p.remember_password;
                c.profile.credential_store = p.credential_store;
            }
        }
        return Ok(json!({"saved":true,"auto_target":target}));
    }
    if op == "inspect_local" {
        return Ok(serde_json::to_value(crate::repository::inspect(
            std::path::Path::new(field(&v, "local_repository")?),
        )?)?);
    }
    if op == "load_profile" {
        if !human {
            bail!("只能在 App 选择连接配置")
        }
        let key = field(&v, "account_key")?;
        let all: std::collections::BTreeMap<String, Profile> =
            read_json(&app_dir()?.join("profiles.json"))?;
        let p = all
            .get(key)
            .ok_or_else(|| anyhow::anyhow!("没有此账号配置"))?;
        return Ok(json!({"profile":p}));
    }
    if op == "connect" {
        if !human {
            bail!("只能在 App 登录")
        }
        let mut p: Profile = serde_json::from_value(v["profile"].clone())?;
        p.credential_store = "file".into();
        p.validate()?;
        if p.credential_store == "file" && v["password"].as_str().is_some_and(|s| !s.is_empty()) {
            crate::secret_store::confirm_consent(v["storage_ack"] == true)?;
        }

        let old = { state.connection.lock().await.take() };
        if let Some(old) = old {
            let _ = old
                .handle
                .disconnect(russh::Disconnect::ByApplication, "Switching identity", "en")
                .await;
        }
        let password = field(&v, "password")?.to_string();
        let typed_password = !password.is_empty();
        #[cfg(target_os = "macos")]
        let password = if password.is_empty() && p.remember_password {
            crate::secret_store::get("file", "dev.ccbridge.desktop.ssh", &p.account_key())
                .await
                .context("读取已保存SSH密码失败，请在App输入或检查保存位置")?
        } else {
            password
        };
        let remembered = if p.remember_password && typed_password {
            Some(password.clone())
        } else {
            None
        };
        let mut conn = ssh::connect(p, password).await?;
        let mut credential_warning = None::<String>;
        #[cfg(target_os = "macos")]
        if let Some(password) = remembered {
            if let Err(e) = crate::secret_store::set(
                "file",
                "dev.ccbridge.desktop.ssh",
                &conn.profile.account_key(),
                password,
            )
            .await
            {
                credential_warning = Some(format!("SSH已连接，但密码未保存：{e}"));
                let old: Option<Profile> = read_json(&app_dir()?.join("profile.json")).ok();
                if let Some(old) = old.filter(|old| old.account_key() == conn.profile.account_key())
                {
                    conn.profile.credential_store = old.credential_store;
                    conn.profile.remember_password = old.remember_password;
                } else {
                    conn.profile.remember_password = false;
                }
            }
        }
        conn.profile.save()?;
        let mut r = conn.info();
        if let Some(w) = credential_warning {
            r["credential_warning_detail"] = json!(w);
            r["credential_warning"] =
                json!("SSH已连接，但新密码未保存，原保存方式保持不变。请检查所选存储后重新保存。");
        }
        *state.connection.lock().await = Some(conn);
        return Ok(r);
    }
    let mut guard = state.connection.lock().await;
    if op == "disconnect" {
        if !human {
            bail!("只能在 App 断开连接")
        }
        let disconnected = guard.take();
        drop(guard);
        if let Some(conn) = disconnected {
            let _ = conn
                .handle
                .disconnect(russh::Disconnect::ByApplication, "User disconnected", "en")
                .await;
        }
        return Ok(json!({"connected":false,"note":"远端任务继续运行；重新连接后可查询"}));
    }
    if op == "connection_status" {
        return Ok(match guard.as_ref() {
            Some(c) => {
                let mut r = c.info();
                r["operations"] = json!({"git_update":crate::operations::snapshot(&state.operations,&c.profile.account_key(),&c.connection_id,None,Some("git_update"),None),"branch_fetch":crate::operations::snapshot(&state.operations,&c.profile.account_key(),&c.connection_id,None,Some("branch_fetch"),None)});
                r
            }
            None => {
                #[cfg(target_os = "macos")]
                let waiting = crate::credentials::pending();
                #[cfg(not(target_os = "macos"))]
                let waiting = false;
                json!({"connected":false,"waiting_for_keychain":waiting})
            }
        });
    }
    let mut c = guard
        .as_ref()
        .cloned()
        .ok_or_else(|| anyhow::anyhow!("尚未连接 SSH，请在 App 登录"))?;
    drop(guard);
    if human
        && v["connection_id"]
            .as_str()
            .is_some_and(|id| id != c.connection_id)
    {
        bail!("连接账号已变化，请刷新后重新操作")
    }
    if c.handle.is_closed() {
        bail!("SSH 已断开，请在 App 重新连接；不要自动重放任务")
    }
    if ["claude_models", "claude_usage"].contains(&op.as_str()) {
        if !human {
            bail!("Claude 环境信息请在 App 查看")
        }
        return c.rpc(json!({"op":op,"claude":c.claude_path})).await;
    }
    if op == "operation_progress" {
        if !human {
            bail!("操作快照仅供 App 查询")
        }
        return Ok(crate::operations::snapshot(
            &state.operations,
            &c.profile.account_key(),
            &c.connection_id,
            v["operation_id"].as_str(),
            v["kind"].as_str(),
            v["path"].as_str(),
        ));
    }
    if op == "git_vps_auth_test" {
        if !human {
            bail!("VPS 认证检测只能在 App 操作")
        }
        let mut r = c.git_access(field(&v, "url")?, None).await.map_err(|e| {
            let text = format!("{e:#}");
            if text.contains("未取得 Codeup 认证")
                || text.contains("未通过认证")
                || text.contains("读取权限")
            {
                anyhow::anyhow!("服务器尚无此仓库的访问权限，请同步认证并核对仓库地址后重试。")
            } else {
                e.context("VPS 自身 Git 检测失败，本次未使用 Mac 凭据")
            }
        })?;
        r["message"] = json!("VPS 自身 Git 认证检测通过，未注入 Mac 凭据");
        return Ok(r);
    }
    if [
        "git_vps_auth_status",
        "git_vps_auth_test",
        "git_vps_auth_install",
        "git_vps_auth_remove",
    ]
    .contains(&op.as_str())
    {
        if !human {
            bail!("VPS 认证只能由人在 App 明确确认，不允许 MCP 写入凭据")
        }
        // The RPC is the compatibility check. A patch-version difference does not
        // invalidate this unchanged protocol (e.g. App 0.10.4 / Runner 0.10.2).
        let status = c
            .rpc(json!({"op":"git_vps_auth_status"}))
            .await
            .context("无法检查 VPS 认证配置；请检查连接及 Runner 是否支持此功能")?;
        if op == "git_vps_auth_status" {
            return Ok(status);
        }
        let account = format!("{}@{}:{}", c.profile.user, c.profile.host, c.profile.port);
        if v["vps_account"] != account {
            bail!("SSH 账号已变化，请重新打开认证窗口确认")
        }
        if op == "git_vps_auth_remove" {
            return c.rpc(json!({"op":op,"confirm":v["confirm"]})).await;
        }
        if v["server_storage_ack"] != true {
            bail!("请在 App 确认凭据将保存到当前 VPS 账号")
        }
        let draft = v["secret"]
            .as_str()
            .filter(|s| !s.is_empty())
            .map(|secret| crate::git_auth::Credential {
                username: v["username"].as_str().unwrap_or("").into(),
                secret: secret.into(),
            });
        let cred = crate::git_auth::for_vps_from(
            v["scope"].as_str().unwrap_or("local"),
            &c.profile,
            v["url"]
                .as_str()
                .filter(|s| !s.trim().is_empty())
                .unwrap_or("https://codeup.aliyun.com/"),
            draft,
            v["credential_source"].as_str().unwrap_or("auto"),
        )
        .await?;
        return c.rpc(json!({"op":op,"server_storage_ack":true,"username":cred.username,"secret":cred.secret})).await;
    }
    let operation = v["operation_id"]
        .as_str()
        .map(|id| crate::operations::Operation {
            store: state.operations.clone(),
            id: id.into(),
        });
    if ["git_update_plan", "git_update"].contains(&op.as_str()) {
        if !human && op == "git_update" && !c.profile.allow_auto {
            bail!("请先在 App 开启全自动授权，再通过 MCP 更新代码")
        }
        if c.probe["runner_version"].as_str() != Some(env!("CARGO_PKG_VERSION")) {
            bail!("请先在 App 更新当前账号 Runner，再使用 Git 更新")
        }
        let path = field(&v, "path")?;
        let plan = c.rpc(json!({"op":"git_update_plan","path":path})).await?;
        if op == "git_update_plan" {
            return Ok(plan);
        }
        if let Some(expected) = v["expected"].as_array() {
            let repos = plan["repositories"].as_array().context("仓库列表无效")?;
            if expected.len() != repos.len() {
                bail!("项目内仓库列表已变化，请刷新后重新确认")
            }
            for repo in repos {
                let old = expected
                    .iter()
                    .find(|r| r["path"] == repo["path"])
                    .context("仓库目标已变化，请刷新")?;
                for key in ["head", "branch", "upstream", "remote"] {
                    if old[key] != repo[key] {
                        bail!("仓库分支或提交已变化，请刷新后重新确认")
                    }
                }
            }
        }
        let repos = plan["repositories"].as_array().context("仓库列表无效")?;
        let only = v["only"].as_array();
        if let Some(only) = only {
            if only.is_empty() || only.iter().any(|p| !repos.iter().any(|r| r["path"] == *p)) {
                bail!("重试范围已变化，请刷新")
            }
        }
        let selected: Vec<_> = repos
            .iter()
            .filter(|r| only.is_none_or(|a| a.contains(&r["path"])))
            .collect();
        if let Some(o) = &operation {
            o.patch(json!({"total":selected.len(),"items":selected}));
        }
        let mut results = vec![];
        for (i, repo) in selected.iter().enumerate() {
            let path = repo["path"].as_str().context("仓库路径无效")?;
            if let Some(o) = &operation {
                o.patch(json!({"name":repo["name"],"current_path":path,"phase":"checking","objects":null,"bytes":null}));
            }
            let result: Result<Value> = async {
                c.rpc(json!({"op":"git_update_preflight","path":path,"expected":repo}))
                    .await?;
                if let Some(o) = &operation {
                    o.patch(json!({"phase":"fetch"}));
                }
                let cb = operation.clone().map(|o| {
                    Arc::new(move |p: Value| o.patch(p)) as Arc<dyn Fn(Value) + Send + Sync>
                });
                c.git_fetch_remote_progress(
                    path,
                    repo["remote"].as_str().context("未设置远端")?,
                    cb,
                )
                .await?;
                if let Some(o) = &operation {
                    o.patch(json!({"phase":"apply"}));
                }
                c.rpc(json!({"op":"git_update_apply","path":path,"expected":repo}))
                    .await
            }
            .await;
            let mut r = match result {
                Ok(r) => r,
                Err(e) => {
                    json!({"path":path,"branch":repo["branch"],"state":"failed","code":error_code(&e),"message":crate::common::error_summary(&format!("{e:#}"))})
                }
            };
            r["url"] = repo["url"].clone();
            r["name"] = repo["name"].clone();
            r["upstream"] = repo["upstream"].clone();
            if let Some(o) = &operation {
                o.item(i, r.clone());
            }
            results.push(r);
            if c.handle.is_closed() {
                bail!("SSH已断开，更新结果未确认，请重新连接后检查仓库")
            }
        }
        let failed = results.iter().filter(|r| r["state"] == "failed").count();
        let updated = results.iter().filter(|r| r["state"] == "updated").count();
        return Ok(json!({"ok":failed==0,"updated":updated,"failed":failed,"results":results}));
    }
    if [
        "file_create",
        "file_mutate",
        "trash_list",
        "trash_restore",
        "branches",
        "branch_change",
        "branch_fetch",
    ]
    .contains(&op.as_str())
    {
        if !human {
            bail!("文件和分支管理只能由人在 App 操作")
        }
        if [
            "file_create",
            "file_mutate",
            "trash_list",
            "trash_restore",
            "branch_fetch",
            "branches",
            "branch_change",
        ]
        .contains(&op.as_str())
            && c.probe["runner_version"].as_str() != Some(env!("CARGO_PKG_VERSION"))
        {
            bail!("请先在服务器页更新当前账号 Runner，再使用文件和分支管理")
        }
        let mut req = v.clone();
        req["protected"] = json!([c.profile.active_remote_workspace, c.profile.workspace]);
        if op == "branch_fetch" {
            if let Some(o) = &operation {
                o.patch(json!({"phase":"fetch"}));
            }
            let cb = operation
                .clone()
                .map(|o| Arc::new(move |p: Value| o.patch(p)) as Arc<dyn Fn(Value) + Send + Sync>);
            return c
                .git_fetch_remote_progress(field(&v, "path")?, "origin", cb)
                .await;
        }
        return c.rpc(req).await;
    }
    if op == "directory_list" {
        return c
            .browse(v["path"].as_str().unwrap_or(""), v["hidden"] == true)
            .await;
    }
    if [
        "create_directory",
        "select_remote_workspace",
        "file_preview",
        "clone_into_directory",
    ]
    .contains(&op.as_str())
    {
        if !human {
            bail!("远端文件管理和工作区选择只能由人在 App 明确操作")
        }
        match op.as_str() {
            "create_directory" => return c.mkdir(field(&v, "path")?, field(&v, "name")?).await,
            "file_preview" => return c.preview_file(field(&v, "path")?).await,
            "clone_into_directory" => {
                return c
                    .clone_observed(
                        field(&v, "path")?,
                        field(&v, "urls")?,
                        v["local_auth"] == true,
                        Some({
                            let state = state.clone();
                            Arc::new(move |stage, completed, total| {
                                *state.clone_progress.lock().unwrap() =
                                    json!({"stage":stage,"completed":completed,"total":total});
                            })
                        }),
                        operation.clone(),
                    )
                    .await;
            }
            "select_remote_workspace" => {
                let listing = c.browse(field(&v, "path")?, false).await?;
                let path = listing["path"].as_str().unwrap();
                c.profile.active_remote_workspace = path.into();
                c.profile.save()?;
                *state.connection.lock().await = Some(c.clone());
                let repo = c
                    .rpc(json!({"op":"repository_inspect","path":path}))
                    .await
                    .ok();
                return Ok(json!({"profile":c.profile,"workspace":path,"repository":repo}));
            }
            _ => unreachable!(),
        }
    }
    if op == "db_projects" || op == "project_list" {
        return c.rpc(v.clone()).await;
    }
    if [
        "db_list",
        "db_save",
        "db_bind",
        "db_remove",
        "db_detect",
        "db_protocol_test",
        "db_test",
        "db_project_save",
        "project_save",
    ]
    .contains(&op.as_str())
    {
        if !human {
            bail!("数据库配置与凭据只能在 App 管理")
        }
        return c.rpc(v.clone()).await;
    }
    if op == "cc_diagnostic" {
        if !human {
            bail!("CC 连接自检只能由人在 App 触发")
        }
        if c.claude_path.is_empty() {
            bail!("远端未找到 Claude Code")
        }
        return c.rpc(json!({"op":"start","task_id":uuid::Uuid::new_v4().to_string(),"workspace":c.environment.home,"claude":c.claude_path,"model":c.profile.model,"mode":"approval","prompt":"这是一次连接自检。不要调用任何工具，不读取或修改任何文件，不执行命令。仅回复 CCBRIDGE_OK。"})).await;
    }
    if op == "prepare_repositories" {
        if !human {
            bail!("拉取仓库只能由人在 App 明确触发，匹配过程不会自动 clone")
        }
        return c
            .prepare_repositories(field(&v, "group")?, field(&v, "urls")?)
            .await;
    }
    if op == "refresh_runner" {
        if !human {
            bail!("只能在 App 重新检测 Runner")
        }
        c.refresh().await;
        *state.connection.lock().await = Some(c.clone());
        return Ok(c.info());
    }
    if op == "install_runner" {
        if !human {
            bail!("只能由用户在 App 安装 Runner")
        }
        let r = c.install_observed(operation.clone()).await;
        *state.connection.lock().await = Some(c);
        return r;
    }
    if op == "set_auto" {
        if !human {
            bail!("只能由用户在 App 开启全自动")
        }
        c.profile.allow_auto = v["allow"] == true;
        c.profile.save()?;
        *state.connection.lock().await = Some(c.clone());
        return Ok(json!({"allow_auto":c.profile.allow_auto}));
    }
    if op == "clear_binding" {
        if !human {
            bail!("只能在 App 清除映射")
        }
        let local =
            crate::repository::inspect(std::path::Path::new(field(&v, "local_repository")?))?;
        let account = c.profile.account_key();
        c.profile
            .bindings
            .retain(|b| !(b.account == account && b.local_path == local.path));
        c.profile.save()?;
        *state.connection.lock().await = Some(c.clone());
        return Ok(json!({"cleared":true,"profile":c.profile}));
    }
    if op == "repository_match" {
        if human {
            if let Some(root) = v["search_root"].as_str() {
                c.profile.search_root = root.into();
            }
        }
        let matched = c.match_repository(field(&v, "local_repository")?).await?;
        if human {
            c.profile.local_repository = matched["local"]["path"].as_str().unwrap_or("").into();
            c.profile.save()?;
            *state.connection.lock().await = Some(c.clone());
        }
        return Ok(matched);
    }
    if op == "bind_repository" {
        if !human {
            bail!("多个候选仓库必须由用户在 App 选择")
        }
        let matched = c.match_repository(field(&v, "local_repository")?).await?;
        let path = field(&v, "remote_path")?;
        let candidate = matched["candidates"]
            .as_array()
            .and_then(|cs| cs.iter().find(|r| r["path"] == path))
            .ok_or_else(|| anyhow::anyhow!("所选路径不是当前账号的匹配候选"))?;
        let local: crate::repository::Repository =
            serde_json::from_value(matched["local"].clone())?;
        let remote: crate::repository::Repository = serde_json::from_value(candidate.clone())?;
        let identity = local
            .identities
            .iter()
            .find(|id| remote.identities.contains(id))
            .unwrap()
            .clone();
        let account = c.profile.account_key();
        c.profile
            .bindings
            .retain(|b| !(b.account == account && b.local_path == local.path));
        c.profile.bindings.push(RepositoryBinding {
            account,
            local_path: local.path.clone(),
            identity,
            remote_path: path.into(),
        });
        c.profile.local_repository = local.path;
        c.profile.save()?;
        *state.connection.lock().await = Some(c.clone());
        return Ok(json!({"bound":true,"profile":c.profile,"selected_path":path}));
    }
    if !c.probe["probe_error"].is_null() {
        bail!(
            "SSH 已连接，但 Runner/工作区/CC 未就绪：{}",
            c.probe["probe_error"]
        )
    }
    if ["start", "reply"].contains(&op.as_str()) && c.probe["protocol_version"] != 1 {
        bail!("Runner 协议不兼容，请在服务器页更新当前账号 Runner；已有任务仍可查询")
    }
    let mut request = v.clone();
    match op.as_str() {
        "start" => {
            let mode = v["mode"].as_str().unwrap_or("approval");
            if mode == "automatic" && !c.profile.allow_auto {
                bail!("全自动尚未获用户授权，请在 App 开启，或使用 approval 模式")
            }
            request["mode"] = json!(mode);
            let explicit_local = v["local_repository"].as_str().filter(|s| !s.is_empty());
            let local = explicit_local.unwrap_or(&c.profile.local_repository);
            let remote_project = v["remote_project"].as_str().filter(|s| !s.is_empty());
            if remote_project.is_some() && explicit_local.is_some() {
                bail!("remote_project 与 local_repository 不能同时指定")
            }
            let workspace = if let Some(project) = remote_project {
                let resolved = c
                    .rpc(json!({"op":"project_resolve","project":project}))
                    .await?;
                resolved["workspace"]
                    .as_str()
                    .context("项目目录无效")?
                    .to_string()
            } else if explicit_local.is_none() && !c.profile.active_remote_workspace.is_empty() {
                let listing = c.browse(&c.profile.active_remote_workspace, false).await?;
                listing["path"].as_str().unwrap().to_string()
            } else if !local.is_empty() {
                let matched = c.match_repository(local).await?;
                matched["selected_path"].as_str().ok_or_else(||anyhow::anyhow!("仓库未唯一匹配，请调用 repository_match 并在 App 选择；不会自动 clone 或按同名目录猜测"))?.to_string()
            } else if !c.profile.workspace.is_empty() {
                c.profile.workspace.clone()
            } else {
                bail!("请先在 App 选择本地仓库，或在 cc_start 中传 local_repository")
            };
            if c.claude_path.is_empty() || !c.probe["claude_error"].is_null() {
                bail!("远端 CC 尚未就绪，请安装并登录或在高级设置指定路径")
            }
            request["workspace"] = json!(workspace);
            request["claude"] = json!(c.claude_path);
            for (key, value) in crate::models::selection(&v, &c.profile.model, None)?
                .as_object()
                .unwrap()
            {
                request[key] = value.clone();
            }
        }
        "reply" => {
            let old = c
                .rpc(json!({"op":"status","task_id":field(&v,"reply_to")?}))
                .await?;
            if !c.permits_workspace(old["workspace"].as_str().unwrap_or("")) {
                bail!("任务不属于当前工作区")
            }
            let mode = reply_mode(
                old["mode"].as_str().unwrap_or("approval"),
                v["mode"].as_str(),
                human,
                c.profile.allow_auto,
            )?;
            request["op"] = json!("start");
            request["mode"] = json!(mode);
            request["workspace"] = old["workspace"].clone();
            request["claude"] = json!(c.claude_path);
            for (key, value) in crate::models::selection(&v, &c.profile.model, Some(&old))?
                .as_object()
                .unwrap()
            {
                request[key] = value.clone();
            }
        }
        "status" | "result" | "cancel" | "approve" => {
            if op == "approve" && !human {
                bail!("审批只能由用户在 App 操作")
            }
            let old=c.rpc(json!({"op":"status","task_id":field(&v,"task_id")?,"cursor":v["cursor"].as_u64().unwrap_or(0)})).await?;
            if !c.permits_workspace(old["workspace"].as_str().unwrap_or("")) {
                bail!("任务不属于当前工作区")
            }
            if op == "status" || op == "result" {
                return Ok(old);
            }
        }
        "list" => {
            let response = c.rpc(json!({"op":"list"})).await?;
            let tasks: Vec<_> = response["tasks"]
                .as_array()
                .into_iter()
                .flatten()
                .filter(|t| c.permits_workspace(t["workspace"].as_str().unwrap_or("")))
                .cloned()
                .collect();
            return Ok(json!({"tasks":tasks}));
        }
        _ => bail!("不支持该操作"),
    }
    c.rpc(request).await
}
pub fn start_backend() -> (
    tokio::sync::mpsc::UnboundedSender<Job>,
    mpsc::Receiver<Value>,
) {
    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<Job>();
    let (events, output) = mpsc::channel();
    std::thread::spawn(move || {
        let result: Result<()> = tokio::runtime::Runtime::new()
            .unwrap()
            .block_on(async move {
                let state: Shared = Arc::new(BackendState::new(None));
                let dir = app_dir()?;
                // flock prevents a second App from unlinking the first App's socket.
                let lock = std::fs::OpenOptions::new()
                    .create(true)
                    .truncate(false)
                    .write(true)
                    .open(dir.join("app.lock"))?;
                use std::os::fd::AsRawFd;
                // LaunchServices may start the updater's new process just before the old
                // process exits. Wait for its OS-owned lock; never release it early.
                for attempt in 0..50 {
                    if unsafe { libc::flock(lock.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) } == 0 {break;}
                    if attempt==49 {bail!("CCBridge 已在运行，请使用原窗口")}
                    tokio::time::sleep(std::time::Duration::from_millis(100)).await;
                }
                let path = dir.join("bridge.sock");
                if path.exists() {
                    std::fs::remove_file(&path)?;
                }
                let listener = tokio::net::UnixListener::bind(&path)?;
                use std::os::unix::fs::PermissionsExt;
                std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600))?;
                let net_state = state.clone();
                tokio::spawn(async move {
                    while let Ok((stream, _)) = listener.accept().await {
                        let state = net_state.clone();
                        tokio::spawn(async move {
                            let (read, mut write) = stream.into_split();
                            let mut line = String::new();
                            let r: Result<Value> = async {
                                tokio::time::timeout(
                                    std::time::Duration::from_secs(10),
                                    BufReader::new(read).take(1024 * 1024).read_line(&mut line),
                                )
                                .await??;
                                dispatch(state, serde_json::from_str(&line)?, false).await
                            }
                            .await;
                            let mut bytes = serde_json::to_vec(&envelope(r)).unwrap();
                            bytes.push(b'\n');
                            let _ = write.write_all(&bytes).await;
                        });
                    }
                });
                while let Some(job) = rx.recv().await {
                    let state = state.clone();
                    tokio::spawn(async move {
                        let op = job.value["op"].clone();
                        let task_id=job.value["task_id"].clone();
                        let cursor=job.value["cursor"].as_u64().unwrap_or(0);
                        let result = envelope(dispatch(state, job.value, true).await);
                        let _ = job.reply.send(json!({"op":op,"response":result,"background":job.background,"task_id":task_id,"request_cursor":cursor}));
                    });
                }
                drop(lock);
                Ok(())
            });
        if let Err(e) = result {
            let _ = events
                .send(json!({"op":"backend","response":{"ok":false,"error":format!("{e:#}")}}));
        }
    });
    (tx, output)
}

// Existing MCP continuations inherit their mode. Only an explicit human App choice may change it.
fn reply_mode<'a>(
    old: &'a str,
    requested: Option<&'a str>,
    human: bool,
    allow_auto: bool,
) -> Result<&'a str> {
    if requested.is_some() && !human {
        bail!("续聊模式只能由用户在 App 切换");
    }
    let mode = requested.unwrap_or(old);
    if !["approval", "automatic"].contains(&mode) {
        bail!("请选择审批模式或全自动模式");
    }
    if mode == "automatic" && !allow_auto {
        bail!("全自动尚未授权，请在连接与权限页面开启，或选择审批模式");
    }
    Ok(mode)
}
#[cfg(test)]
mod mode_tests {
    use super::*;
    #[tokio::test]
    async fn offline_update_needs_no_ssh_but_still_protects_local_work() {
        let state = Arc::new(BackendState::new(None));
        let ready = dispatch(state.clone(), json!({"op":"prepare_update"}), true)
            .await
            .unwrap();
        assert_eq!(ready["draining"], true);
        dispatch(state.clone(), json!({"op":"cancel_update"}), true)
            .await
            .unwrap();
        state.gate.lock().unwrap().begin("git_update").unwrap();
        let error = dispatch(state.clone(), json!({"op":"prepare_update"}), true)
            .await
            .unwrap_err()
            .to_string();
        assert!(error.contains("Git 更新"));
        assert!(!state.gate.lock().unwrap().draining);
        state.gate.lock().unwrap().finish("git_update");
        let lease = uuid::Uuid::new_v4().to_string();
        state
            .gate
            .lock()
            .unwrap()
            .lease("client_activity_begin", &lease)
            .unwrap();
        let error = dispatch(state.clone(), json!({"op":"prepare_update"}), true)
            .await
            .unwrap_err()
            .to_string();
        assert!(error.contains("Codex 正在等待"));
        assert!(
            dispatch(state.clone(), json!({"op":"prepare_update"}), false)
                .await
                .is_err()
        );
    }
    #[test]
    fn update_blockers_name_tasks_and_next_action() {
        let error = ensure_update_tasks_idle(&json!({"tasks":[{"title":"只读文档分析","state":"running"},{"title":"审批验证","state":"waiting_approval"}]})).unwrap_err().to_string();
        assert!(error.contains("2 个未结束任务"));
        assert!(error.contains("只读文档分析（执行中）"));
        assert!(error.contains("审批验证（等待审批）"));
        assert!(error.contains("会话"));
        assert!(ensure_update_tasks_idle(&json!({"tasks":[{"state":"completed"}]})).is_ok());
        assert!(ensure_update_tasks_idle(&json!({})).is_err());
    }
    #[test]
    fn startup_requires_actual_local_password_for_exact_account() {
        let root = std::env::temp_dir().join(format!("ccb-startup-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir(&root).unwrap();
        let mut p = Profile::default();
        p.user = "startup-test".into();
        p.host = "test.invalid".into();
        p.remember_password = true;
        p.fingerprint = "test-fingerprint".into();
        p.credential_store = "keychain".into();
        assert!(validate_auto_target_at(&root, &p).is_err());
        p.credential_store = "file".into();
        assert!(validate_auto_target_at(&root, &p).is_err());
        let key = format!("dev.ccbridge.desktop.ssh:{}", p.account_key());
        crate::secret_store::update_at(&root, &key, Some("test-only-not-a-real-password")).unwrap();
        assert!(validate_auto_target_at(&root, &p).is_ok());
        p.user = "another-account".into();
        assert!(validate_auto_target_at(&root, &p).is_err());
        p.user = "startup-test".into();
        p.fingerprint.clear();
        assert!(validate_auto_target_at(&root, &p).is_err());
        std::fs::remove_dir_all(root).unwrap();
    }
    #[tokio::test]
    async fn mcp_cannot_migrate_or_trigger_gui_auto_connect() {
        let state = std::sync::Arc::new(BackendState::new(None));
        for op in ["migrate_password", "auto_connect"] {
            assert!(
                dispatch(state.clone(), json!({"op":op}), false)
                    .await
                    .is_err()
            );
        }
        assert!(
            !state
                .auto_attempted
                .load(std::sync::atomic::Ordering::SeqCst)
        );
    }

    #[test]
    fn human_turn_mode_and_legacy_inheritance() {
        assert_eq!(
            reply_mode("approval", None, false, true).unwrap(),
            "approval"
        );
        assert_eq!(
            reply_mode("automatic", None, false, true).unwrap(),
            "automatic"
        );
        assert!(reply_mode("approval", Some("automatic"), false, true).is_err());
        assert!(reply_mode("approval", Some("automatic"), true, false).is_err());
        assert_eq!(
            reply_mode("approval", Some("automatic"), true, true).unwrap(),
            "automatic"
        );
        assert_eq!(
            reply_mode("automatic", Some("approval"), true, false).unwrap(),
            "approval"
        );
        assert!(reply_mode("approval", Some("invalid"), true, true).is_err());
    }
}
