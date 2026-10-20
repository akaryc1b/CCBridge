use crate::{common::*, ssh};
use anyhow::{Context, Result, bail};
use serde_json::{Value, json};
use std::sync::{Arc, mpsc};
use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};
use tokio::sync::Mutex;

pub(crate) type Shared = Arc<Mutex<Option<ssh::Connection>>>;
pub(crate) struct Job {
    pub value: Value,
    pub reply: mpsc::Sender<Value>,
    pub background: bool,
}
pub(crate) async fn dispatch(state: Shared, v: Value, human: bool) -> Result<Value> {
    let op = field(&v, "op")?.to_owned();
    if ["save_profile", "load_password", "forget_password"].contains(&op.as_str()) {
        if !human {
            bail!("凭据只能在 App 管理")
        }
        let p: Profile = serde_json::from_value(v["profile"].clone())?;
        p.validate()?;
        #[cfg(target_os = "macos")]
        {
            let account = serde_json::to_string(&(p.host.as_str(), p.port, p.user.as_str()))?;
            let password = v["password"].as_str().unwrap_or("").to_string();
            let credential_op = op.clone();
            let remember = p.remember_password;
            let credential_result=crate::credentials::call(move||{
                let entry=keyring::Entry::new("dev.ccbridge.desktop.ssh",&account)?;
                if credential_op=="load_password" {return Ok(json!({"password":entry.get_password().map_err(|e|anyhow::anyhow!("钥匙串读取失败：{e}"))?}));}
                if credential_op=="forget_password" || !remember {
                    match entry.delete_credential(){Ok(())|Err(keyring::Error::NoEntry)=>{},Err(e)=>return Err(e.into())}
                    return Ok(json!({"deleted":true}));
                }
                if !password.is_empty(){entry.set_password(&password)?;}else{entry.get_password().map_err(|_|anyhow::anyhow!("请先输入密码，或取消记住密码"))?;}
                Ok(json!({"saved":true}))
            }).await?;
            if op != "save_profile" {
                return Ok(credential_result);
            }
        }
        p.save()?;
        return Ok(json!({"saved":true}));
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
        let p: Profile = serde_json::from_value(v["profile"].clone())?;
        p.validate()?;
        let old = { state.lock().await.take() };
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
            let account = serde_json::to_string(&(p.host.as_str(), p.port, p.user.as_str()))?;
            crate::credentials::call(move || {
                Ok(keyring::Entry::new("dev.ccbridge.desktop.ssh", &account)?.get_password()?)
            })
            .await
            .context("读取已保存 SSH 密码失败，请处理钥匙串授权或手动输入密码")?
        } else {
            password
        };
        let remembered = if p.remember_password && typed_password {
            Some(password.clone())
        } else {
            None
        };
        let conn = ssh::connect(p, password).await?;
        let mut credential_warning = None::<String>;
        #[cfg(target_os = "macos")]
        if let Some(password) = remembered {
            let account = serde_json::to_string(&(
                conn.profile.host.as_str(),
                conn.profile.port,
                conn.profile.user.as_str(),
            ))?;
            if let Err(e) = crate::credentials::call(move || {
                Ok(keyring::Entry::new("dev.ccbridge.desktop.ssh", &account)?
                    .set_password(&password)?)
            })
            .await
            {
                credential_warning = Some(format!("SSH 已连接，但密码尚未保存：{e}"));
            }
        }
        conn.profile.save()?;
        let mut r = conn.info();
        if let Some(w) = credential_warning {
            r["credential_warning_detail"] = json!(w);
            r["credential_warning"] = json!(
                "SSH 已连接，但密码未保存。请检查 macOS 钥匙串授权；下次可手动输入 SSH 密码。"
            );
        }
        *state.lock().await = Some(conn);
        return Ok(r);
    }
    let mut guard = state.lock().await;
    if op == "disconnect" {
        if !human {
            bail!("只能在 App 断开连接")
        }
        if let Some(conn) = guard.take() {
            let _ = conn
                .handle
                .disconnect(russh::Disconnect::ByApplication, "User disconnected", "en")
                .await;
        }
        return Ok(json!({"connected":false,"note":"远端任务继续运行；重新连接后可查询"}));
    }
    if op == "connection_status" {
        return Ok(match guard.as_ref() {
            Some(c) => c.info(),
            None => {
                #[cfg(target_os = "macos")]
                let waiting = crate::credentials::pending();
                #[cfg(not(target_os = "macos"))]
                let waiting = false;
                json!({"connected":false,"waiting_for_keychain":waiting})
            }
        });
    }
    let c = guard
        .as_mut()
        .ok_or_else(|| anyhow::anyhow!("尚未连接 SSH，请在 App 登录"))?;
    if c.handle.is_closed() {
        bail!("SSH 已断开，请在 App 重新连接；不要自动重放任务")
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
                    .clone_into(
                        field(&v, "path")?,
                        field(&v, "urls")?,
                        v["local_auth"] == true,
                    )
                    .await;
            }
            "select_remote_workspace" => {
                let listing = c.browse(field(&v, "path")?, false).await?;
                let path = listing["path"].as_str().unwrap();
                c.profile.active_remote_workspace = path.into();
                c.profile.save()?;
                let repo = c
                    .rpc(json!({"op":"repository_inspect","path":path}))
                    .await
                    .ok();
                return Ok(json!({"profile":c.profile,"workspace":path,"repository":repo}));
            }
            _ => unreachable!(),
        }
    }
    if op == "db_projects" {
        return c.rpc(v.clone()).await;
    }
    if [
        "db_list",
        "db_save",
        "db_bind",
        "db_remove",
        "db_detect",
        "db_test",
        "db_project_save",
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
        return Ok(c.info());
    }
    if op == "install_runner" {
        if !human {
            bail!("只能由用户在 App 安装 Runner")
        }
        return c.install().await;
    }
    if op == "set_auto" {
        if !human {
            bail!("只能由用户在 App 开启全自动")
        }
        c.profile.allow_auto = v["allow"] == true;
        c.profile.save()?;
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
        return Ok(json!({"bound":true,"profile":c.profile,"selected_path":path}));
    }
    if !c.probe["probe_error"].is_null() {
        bail!(
            "SSH 已连接，但 Runner/工作区/CC 未就绪：{}",
            c.probe["probe_error"]
        )
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
                let resolved = c.rpc(json!({"op":"db_resolve","project":project})).await?;
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
            request["model"] = json!(c.profile.model);
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
            request["model"] = json!(c.profile.model);
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
pub(crate) fn start_backend() -> (
    tokio::sync::mpsc::UnboundedSender<Job>,
    mpsc::Receiver<Value>,
) {
    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<Job>();
    let (events, output) = mpsc::channel();
    std::thread::spawn(move || {
        let result: Result<()> = tokio::runtime::Runtime::new()
            .unwrap()
            .block_on(async move {
                let state: Shared = Arc::new(Mutex::new(None));
                let dir = app_dir()?;
                // flock prevents a second App from unlinking the first App's socket.
                let lock = std::fs::OpenOptions::new()
                    .create(true)
                    .truncate(false)
                    .write(true)
                    .open(dir.join("app.lock"))?;
                use std::os::fd::AsRawFd;
                if unsafe { libc::flock(lock.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) } != 0 {
                    bail!("CCBridge 已在运行，请使用原窗口")
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
