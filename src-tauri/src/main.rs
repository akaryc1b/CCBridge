#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
use serde_json::{Value, json};
use std::sync::{Mutex, mpsc};
use tauri::{Emitter, State};
use tauri_plugin_updater::{Update, UpdaterExt};
struct Bridge(tokio::sync::mpsc::UnboundedSender<ccbridge::backend::Job>);
struct PendingUpdate(Mutex<Option<(Update, Option<Vec<u8>>)>>);
async fn call(bridge: &Bridge, op: &str, mut request: Value) -> Result<Value, Value> {
    request["op"] = json!(op);
    let (reply, rx) = mpsc::channel();
    bridge
        .0
        .send(ccbridge::backend::Job {
            value: request,
            reply,
            background: false,
        })
        .map_err(|_| json!({"message":"App 后台已停止，请重新打开 CCBridge"}))?;
    let envelope = tauri::async_runtime::spawn_blocking(move || rx.recv())
        .await
        .map_err(|e| json!({"message":"后台请求失败","diagnostic":e.to_string()}))?
        .map_err(|_| json!({"message":"后台通信已中断；请查询原任务，不要重复派发"}))?;
    let response = &envelope["response"];
    if response["ok"] == true {
        Ok(response["data"].clone())
    } else {
        Err(
            json!({"message":if response["error"].as_str().unwrap_or("").starts_with("HOST_KEY:"){response["error"].as_str().unwrap().to_string()}else{ccbridge::common::error_summary(response["error"].as_str().unwrap_or("操作失败"))},"diagnostic":response["error"],"detail":response["error_detail"]}),
        )
    }
}
macro_rules! commands { ($($name:ident => $op:literal),* $(,)?) => {$ (
#[tauri::command]
async fn $name(bridge: State<'_, Bridge>, request: Value) -> Result<Value,Value> { call(&bridge,$op,request).await }
)*}; }
commands! {
 connection_status=>"connection_status", connect_ssh=>"connect", disconnect_ssh=>"disconnect",
 save_profile=>"save_profile", load_profile=>"load_profile", forget_password=>"forget_password",
 refresh_runner=>"refresh_runner", install_runner=>"install_runner", set_auto=>"set_auto",
 directory_list=>"directory_list", create_directory=>"create_directory", file_preview=>"file_preview",
 clone_repository=>"clone_into_directory", select_workspace=>"select_remote_workspace",
 project_list=>"project_list", project_save=>"project_save", task_list=>"list", task_status=>"status",
 task_start=>"start", task_reply=>"reply", task_cancel=>"cancel", task_approve=>"approve",
 update_safety=>"safe_update_state"
}
#[tauri::command]
fn initial_state() -> Result<Value, String> {
    let dir = ccbridge::common::app_dir().map_err(|e| e.to_string())?;
    let profile: Value = std::fs::read(dir.join("profile.json"))
        .ok()
        .and_then(|b| serde_json::from_slice(&b).ok())
        .unwrap_or(json!({}));
    let profiles: Value = std::fs::read(dir.join("profiles.json"))
        .ok()
        .and_then(|b| serde_json::from_slice(&b).ok())
        .unwrap_or(json!({}));
    Ok(json!({"profile":profile,"profiles":profiles,"version":env!("CARGO_PKG_VERSION")}))
}
#[tauri::command]
async fn configure_codex(bridge: State<'_, Bridge>) -> Result<String, String> {
    let r = call(&bridge, "configure_codex", json!({}))
        .await
        .map_err(|e| e["message"].as_str().unwrap_or("配置失败").to_string())?;
    Ok(r["message"].as_str().unwrap_or("已配置").into())
}

#[tauri::command]
async fn check_update(
    app: tauri::AppHandle,
    state: State<'_, PendingUpdate>,
) -> Result<Value, String> {
    let update = app
        .updater()
        .map_err(|e| e.to_string())?
        .check()
        .await
        .map_err(|e| format!("检查更新失败：{e}"))?;
    let info = match &update {
        Some(u) => {
            json!({"available":true,"version":u.version,"current":u.current_version,"body":u.body})
        }
        None => json!({"available":false}),
    };
    *state.0.lock().map_err(|_| "更新状态异常")? = update.map(|u| (u, None));
    Ok(info)
}
#[tauri::command]
async fn download_update(
    app: tauri::AppHandle,
    state: State<'_, PendingUpdate>,
) -> Result<Value, String> {
    let (update, _) = state
        .0
        .lock()
        .map_err(|_| "更新状态异常")?
        .take()
        .ok_or("请先检查更新")?;
    let mut downloaded = 0usize;
    let result = update
        .download(
            |n, total| {
                downloaded += n;
                let _ = app.emit(
                    "update-progress",
                    json!({"downloaded":downloaded,"total":total}),
                );
            },
            || {},
        )
        .await;
    match result {
        Ok(bytes) => {
            *state.0.lock().map_err(|_| "更新状态异常")? = Some((update, Some(bytes)));
            Ok(json!({"verified":true}))
        }
        Err(e) => {
            *state.0.lock().map_err(|_| "更新状态异常")? = Some((update, None));
            Err(format!("下载或签名验证失败，原 App 未修改：{e}"))
        }
    }
}
#[tauri::command]
async fn apply_update(
    app: tauri::AppHandle,
    bridge: State<'_, Bridge>,
    state: State<'_, PendingUpdate>,
) -> Result<Value, String> {
    call(&bridge, "prepare_update", json!({}))
        .await
        .map_err(|e| e["message"].as_str().unwrap_or("更新排空失败").to_string())?;
    let pending = state.0.lock().map_err(|_| "更新状态异常")?.take();
    let Some((update, Some(bytes))) = pending else {
        let _ = call(&bridge, "cancel_update", json!({})).await;
        return Err("请先下载并验证更新".into());
    };
    if let Err(e) = update.install(bytes) {
        let _ = call(&bridge, "cancel_update", json!({})).await;
        return Err(format!("安装失败，请继续使用当前版本：{e}"));
    }
    app.restart()
}
fn main() {
    match ccbridge::cli::run_non_gui(std::env::args().skip(1).collect()) {
        Ok(true) => return,
        Ok(false) => {}
        Err(e) => {
            eprintln!("{e:#}");
            std::process::exit(1)
        }
    }
    let (tx, events) = ccbridge::backend::start_backend();
    tauri::Builder::default()
        .plugin(tauri_plugin_updater::Builder::new().build())
        .manage(Bridge(tx))
        .manage(PendingUpdate(Mutex::new(None)))
        .setup(move |app| {
            let handle = app.handle().clone();
            std::thread::spawn(move || {
                for value in events {
                    let _ = handle.emit("backend-event", value);
                }
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            initial_state,
            connection_status,
            connect_ssh,
            disconnect_ssh,
            save_profile,
            load_profile,
            forget_password,
            refresh_runner,
            install_runner,
            set_auto,
            directory_list,
            create_directory,
            file_preview,
            clone_repository,
            select_workspace,
            project_list,
            project_save,
            task_list,
            task_status,
            task_start,
            task_reply,
            task_cancel,
            task_approve,
            update_safety,
            configure_codex,
            check_update,
            download_update,
            apply_update
        ])
        .run(tauri::generate_context!())
        .expect("CCBridge 启动失败");
}
