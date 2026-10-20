//! UI is composed from shadcn-egui's public widgets and theme; no custom-painted controls.
mod chat_ui;
mod databases_ui;
mod explorer_ui;
use crate::{
    backend::{Job, start_backend},
    common::*,
};
use anyhow::Result;
use eframe::egui;
use serde_json::{Value, json};
use shadcn_egui::{
    Theme,
    components::{
        badge::{Badge, BadgeVariant},
        button::{Button, ButtonVariant},
        card::{Card, card_header},
        input::Input,
        label::{Label, description},
        select::Select,
        switch::Switch,
        tabs::Tabs,
    },
};
use std::sync::{Arc, mpsc};

struct App {
    db: databases_ui::DatabaseUi,
    profile: Profile,
    port_input: String,
    password: String,
    host_candidate: String,
    message: String,
    diagnostic: String,
    connected: bool,
    tx: tokio::sync::mpsc::UnboundedSender<Job>,
    rx: mpsc::Receiver<Value>,
    reply: mpsc::Sender<Value>,
    tasks: Vec<Value>,
    selected: String,
    task: Value,
    prompt: String,
    mode_index: usize,
    busy: usize,
    background_busy: usize,
    last_connection_poll: std::time::Instant,
    pending_status: std::collections::BTreeSet<String>,
    turns: std::collections::BTreeMap<String, Value>,
    chat_session: String,
    new_chat: bool,
    drafts: std::collections::BTreeMap<String, String>,
    chat_search: String,
    markdown: egui_commonmark::CommonMarkCache,
    last_poll: std::time::Instant,
    page: usize,
    auto_confirm: bool,
    install_confirm: bool,
    connection: Value,
    matched: Value,
    local: Value,
    saved: Vec<Profile>,
    saved_index: usize,
    browser_path: String,
    directories: std::collections::BTreeMap<String, Value>,
    show_hidden: bool,
    new_directory: String,
    show_mkdir: bool,
    show_clone: bool,
    clone_auth: usize,
    file_preview: Value,
    selected_remote: Value,
    prepare_urls: String,
    prepared: Value,
}
impl App {
    fn new(cc: &eframe::CreationContext<'_>) -> Self {
        let mut fonts = egui::FontDefinitions::default();
        for p in [
            "/System/Library/Fonts/PingFang.ttc",
            "/System/Library/Fonts/STHeiti Light.ttc",
        ] {
            if let Ok(data) = std::fs::read(p) {
                fonts
                    .font_data
                    .insert("Chinese".into(), Arc::new(egui::FontData::from_owned(data)));
                fonts
                    .families
                    .get_mut(&egui::FontFamily::Proportional)
                    .unwrap()
                    .push("Chinese".into());
                fonts
                    .families
                    .get_mut(&egui::FontFamily::Monospace)
                    .unwrap()
                    .push("Chinese".into());
                break;
            }
        }
        Theme::vega_light().apply_with_fonts(&cc.egui_ctx, fonts);
        let mut profile: Profile = app_dir()
            .ok()
            .and_then(|d| read_json(&d.join("profile.json")).ok())
            .unwrap_or_default();
        profile.migrate();
        let saved = load_saved();
        let port_input = profile.port.to_string();
        let (tx, backend_rx) = start_backend();
        let (reply, rx) = mpsc::channel();
        let reply2 = reply.clone();
        std::thread::spawn(move || {
            for v in backend_rx {
                let _ = reply2.send(v);
            }
        });
        Self {
            db: Default::default(),
            profile,
            port_input,
            password: String::new(),
            host_candidate: String::new(),
            message: "连接自己的 SSH 账号，然后在远端工作区浏览、克隆并打开项目。".into(),
            diagnostic: String::new(),
            connected: false,
            tx,
            rx,
            reply,
            tasks: vec![],
            selected: String::new(),
            task: Value::Null,
            prompt: String::new(),
            mode_index: 0,
            busy: 0,
            background_busy: 0,
            last_connection_poll: std::time::Instant::now(),
            pending_status: Default::default(),
            turns: Default::default(),
            chat_session: String::new(),
            new_chat: false,
            drafts: Default::default(),
            chat_search: String::new(),
            markdown: Default::default(),
            last_poll: std::time::Instant::now(),
            page: 0,
            auto_confirm: false,
            install_confirm: false,
            connection: Value::Null,
            matched: Value::Null,
            local: Value::Null,
            saved,
            saved_index: 0,
            browser_path: String::new(),
            directories: Default::default(),
            show_hidden: false,
            new_directory: String::new(),
            show_mkdir: false,
            show_clone: false,
            clone_auth: 0,
            file_preview: Value::Null,
            selected_remote: Value::Null,
            prepare_urls: String::new(),
            prepared: Value::Null,
        }
    }
    fn send(&mut self, v: Value) {
        if v["op"] == "connect" {
            self.message = if v["profile"]["remember_password"] == true
                && v["password"].as_str().unwrap_or("").is_empty()
            {
                "正在读取 macOS 钥匙串；若系统请求授权，请自行完成。"
            } else {
                "正在连接 SSH，请稍候。"
            }
            .into();
        }
        self.enqueue(v, false);
    }
    fn quiet(&mut self, v: Value) {
        self.enqueue(v, true);
    }
    fn enqueue(&mut self, v: Value, background: bool) {
        if background {
            self.background_busy += 1;
        } else {
            self.busy += 1;
        }
        if self
            .tx
            .send(Job {
                value: v,
                reply: self.reply.clone(),
                background,
            })
            .is_err()
        {
            if background {
                self.background_busy = self.background_busy.saturating_sub(1);
            } else {
                self.busy = self.busy.saturating_sub(1);
            }
            self.message = "后台已停止，请重新打开 App".into();
        }
    }
    fn query_task(&mut self, id: String) {
        if id.is_empty() || !self.pending_status.insert(id.clone()) {
            return;
        }
        let cursor = self
            .turns
            .get(&id)
            .and_then(|t| t["next_cursor"].as_u64())
            .unwrap_or(0);
        self.quiet(json!({"op":"status","task_id":id,"cursor":cursor}));
    }
    fn port_valid(&mut self) -> bool {
        match self.port_input.parse::<u16>() {
            Ok(n) if n > 0 => {
                self.profile.port = n;
                true
            }
            _ => {
                self.message = "端口必须是 1–65535 的整数。".into();
                false
            }
        }
    }
    fn receive(&mut self) {
        while let Ok(v) = self.rx.try_recv() {
            if v["background"] == true {
                self.background_busy = self.background_busy.saturating_sub(1);
            } else {
                self.busy = self.busy.saturating_sub(1);
            }
            if let Some(id) = v["task_id"].as_str() {
                self.pending_status.remove(id);
            }
            let r = &v["response"];
            if r["ok"] != true {
                let e = r["error"]
                    .as_str()
                    .unwrap_or("操作失败，服务未提供具体原因");
                if let Some(rest) = e.strip_prefix("HOST_KEY:") {
                    self.host_candidate = rest.lines().next().unwrap_or("").into();
                }
                if e.contains("SSH 已断开") {
                    self.connected = false;
                }
                self.diagnostic = e.into();
                self.message = error_summary(e);
                continue;
            }
            let d = r["data"].clone();
            match v["op"].as_str().unwrap_or("") {
                "db_list" => {
                    let path = d["workspace"].as_str().unwrap_or("");
                    self.db.bindings = self.db.mapping_drafts.remove(path).unwrap_or_else(|| {
                        serde_json::from_value(d["bindings"].clone()).unwrap_or_default()
                    });
                    if let Some(p) = d["projects"]
                        .as_array()
                        .and_then(|ps| ps.iter().find(|p| p["path"] == path))
                    {
                        self.db.project_name = p["name"].as_str().unwrap_or("").into();
                    }
                    self.db.data = d;
                    self.db.loaded = true;
                    self.message = "已载入当前账号的数据库和项目映射。".into();
                }
                "db_detect" => {
                    self.db.draft.node = d["node"].as_str().unwrap_or("").into();
                    self.db.draft.package_root = d["package_root"].as_str().unwrap_or("").into();
                    if self.db.draft.kind.is_empty() {
                        self.db.draft.kind = "mysql".into();
                        self.db.draft.port = 3306;
                    }
                    self.message = d["notice"].as_str().unwrap_or("探测完成").into();
                }
                "db_save" => {
                    self.db.draft = serde_json::from_value(d).unwrap_or_default();
                    self.message = "数据库连接已保存；尚未测试真实连接。".into();
                    self.send(json!({"op":"db_list","workspace":self.db.workspace}));
                }
                "db_test" => {
                    self.message = d["notice"].as_str().unwrap_or("数据库测试完成").into();
                }
                "db_bind" => {
                    self.db.mapping_drafts.remove(&self.db.workspace);
                    self.message = "项目数据库映射已保存，下次新会话生效。".into();
                    self.send(json!({"op":"db_list","workspace":self.db.workspace}));
                }
                "db_project_save" => {
                    self.db.workspace = d["workspace"].as_str().unwrap_or("").into();
                    self.message = "项目已保存，可继续配置数据库映射。".into();
                    self.send(json!({"op":"db_list","workspace":self.db.workspace}));
                }
                "connect" => {
                    self.db = Default::default();
                    self.connected = true;
                    self.tasks.clear();
                    self.turns.clear();
                    self.pending_status.clear();
                    self.chat_session.clear();
                    self.drafts.clear();
                    self.new_chat = false;
                    self.selected.clear();
                    self.task = Value::Null;
                    self.matched = Value::Null;
                    self.profile = serde_json::from_value(d["profile"].clone())
                        .unwrap_or(self.profile.clone());
                    self.connection = d;
                    self.directories.clear();
                    self.browser_path = String::new();
                    self.file_preview = Value::Null;
                    let home = self.connection["environment"]["home"].clone();
                    self.send(json!({"op":"directory_list","path":home,"hidden":self.show_hidden}));
                    self.message = if let Some(e) = self.connection["probe"]["probe_error"].as_str()
                    {
                        format!("SSH 已登录；请安装 / 更新 Runner。{e}")
                    } else {
                        "SSH 已连接。打开远端工作区，选择或克隆项目即可开始。".into()
                    };
                    if let Some(w) = self.connection["credential_warning"].as_str() {
                        self.message = w.into();
                        self.diagnostic = self.connection["credential_warning_detail"]
                            .as_str()
                            .unwrap_or("")
                            .into();
                    }
                    self.saved = load_saved();
                    if self.connection["probe"]["probe_error"].is_null() {
                        self.send(json!({"op":"list"}));
                    }
                }
                "connection_status" => {
                    let was = self.connected;
                    self.connected = d["connected"] == true;
                    self.connection = d;
                    if was && !self.connected {
                        self.message = "SSH 已断开，请重新登录；原任务不会自动重放。".into();
                    }
                }
                "disconnect" => {
                    self.connected = false;
                    self.tasks.clear();
                    self.turns.clear();
                    self.pending_status.clear();
                    self.chat_session.clear();
                    self.drafts.clear();
                    self.new_chat = false;
                    self.selected.clear();
                    self.task = Value::Null;
                    self.local = Value::Null;
                    self.connection = Value::Null;
                    self.matched = Value::Null;
                    self.message = "已断开；远端任务继续运行，重连后可查询。".into();
                }
                "save_profile" => {
                    self.saved = load_saved();
                    self.message =
                        "配置已按服务器、端口、账号保存；记住的密码仅在 macOS 钥匙串中。".into();
                }
                "load_profile" => {
                    self.profile = serde_json::from_value(d["profile"].clone()).unwrap();
                    self.profile.migrate();
                    self.port_input = self.profile.port.to_string();
                    self.password.clear();
                    self.host_candidate.clear();
                    self.matched = Value::Null;
                    self.local = Value::Null;
                    self.message = "已载入此账号配置，连接时使用该账号的远端 HOME。".into();
                }
                "load_password" => {
                    self.password = d["password"].as_str().unwrap_or("").into();
                    self.message = "已从钥匙串读取当前账号密码。".into();
                }
                "forget_password" => {
                    self.password.clear();
                    self.profile.remember_password = false;
                    self.message = "已删除当前账号的已存密码。".into();
                }
                "refresh_runner" => {
                    self.connection["probe"] = d["probe"].clone();
                    self.message = "Runner 状态已重新检测。".into();
                }
                "install_runner" => {
                    self.connection["probe"] = d["probe"].clone();
                    self.turns.clear();
                    self.quiet(json!({"op":"list"}));
                    self.message = format!(
                        "Runner 已就绪：{}。",
                        d["path"].as_str().unwrap_or("账号目录")
                    );
                    self.send(json!({"op":"connection_status"}));
                }
                "directory_list" | "create_directory" => {
                    let path = d["path"].as_str().unwrap_or("").to_string();
                    self.browser_path = path.clone();
                    self.directories.insert(path, d);
                    if v["op"] == "create_directory" {
                        self.new_directory.clear();
                        self.show_mkdir = false;
                        self.message = "文件夹已创建。".into();
                    }
                }
                "file_preview" => {
                    self.file_preview = d;
                }
                "select_remote_workspace" => {
                    self.profile = serde_json::from_value(d["profile"].clone())
                        .unwrap_or(self.profile.clone());
                    self.selected_remote = d;
                    self.message = "CC 工作目录已选择；新任务会在此目录执行。".into();
                }
                "clone_into_directory" => {
                    self.message = if d["complete"] == true {
                        "仓库已准备完成，展开目录即可查看和使用。".into()
                    } else {
                        d["error"].as_str().unwrap_or("仓库准备未完成").into()
                    };
                    let parent = d["root"].clone();
                    self.prepared = d;
                    self.send(
                        json!({"op":"directory_list","path":parent,"hidden":self.show_hidden}),
                    );
                }
                "cc_diagnostic" => {
                    self.switch_chat(d["session_id"].as_str().unwrap_or("").into());
                    self.selected = d["task_id"].as_str().unwrap_or("").into();
                    self.task = Value::Null;
                    self.message = "正在测试 CC 连接。".into();
                    self.query_task(self.selected.clone());
                    self.send(json!({"op":"list"}));
                }
                "prepare_repositories" => {
                    self.message = if d["complete"] == true {
                        "远端项目已准备完成，可以匹配本地仓库。".into()
                    } else {
                        d["error"].as_str().unwrap_or("远端项目准备未完成").into()
                    };
                    self.prepared = d;
                }
                "inspect_local" => {
                    self.local = d;
                    self.matched = Value::Null;
                    self.message = "已识别本地 Git 仓库；尚未同步或上传任何代码。".into();
                }
                "repository_match" => {
                    self.local = d["local"].clone();
                    self.message = match d["state"].as_str() {
                        Some("matched") => "已按 Git remote 身份匹配现有远端仓库。",
                        Some("not_found") => {
                            "当前搜索范围没有匹配仓库。不会自动 clone 或按同名目录猜测。"
                        }
                        _ => "请在候选列表中选择；多个候选或搜索不完整时不自动决定。",
                    }
                    .into();
                    self.matched = d;
                }
                "clear_binding" => {
                    self.profile = serde_json::from_value(d["profile"].clone()).unwrap();
                    self.matched = Value::Null;
                    self.send(json!({"op":"repository_match","local_repository":self.profile.local_repository,"search_root":self.profile.search_root}));
                }
                "bind_repository" => {
                    self.profile = serde_json::from_value(d["profile"].clone()).unwrap();
                    self.matched["selected_path"] = d["selected_path"].clone();
                    self.matched["state"] = json!("matched");
                    self.message = "仓库映射已保存到当前 SSH 账号。".into();
                }
                "list" => {
                    self.tasks = d["tasks"].as_array().cloned().unwrap_or_default();
                    if self.chat_session.is_empty() && !self.new_chat {
                        if let Some(t) = self.tasks.first() {
                            self.chat_session = t["session_id"].as_str().unwrap_or("").into();
                        }
                    }
                    let rounds = self.chat_rounds();
                    if let Some(t) = rounds.last() {
                        let id = t["id"].as_str().unwrap_or("").to_string();
                        if self.selected != id {
                            self.mode_index = usize::from(t["mode"] == "automatic");
                            self.selected = id.clone();
                            self.task = self.turns.get(&id).cloned().unwrap_or(Value::Null);
                        }
                    }
                    for t in rounds {
                        let id = t["id"].as_str().unwrap_or("");
                        if !self.turns.contains_key(id) {
                            self.query_task(id.into());
                        }
                    }
                }
                "start" | "reply" => {
                    self.selected = d["task_id"].as_str().unwrap_or("").into();
                    self.prompt.clear();
                    self.chat_session = d["session_id"].as_str().unwrap_or("").into();
                    self.new_chat = false;
                    self.task = Value::Null;
                    self.message = "CC 正在处理你的消息。".into();
                    self.send(json!({"op":"list"}));
                    self.query_task(self.selected.clone());
                }
                "status" | "result" => {
                    let id = d["task_id"].as_str().unwrap_or("").to_string();
                    let more = d["has_more"] == true;
                    let merged = crate::conversation::merge(
                        self.turns.get(&id),
                        d,
                        v["request_cursor"].as_u64().unwrap_or(0),
                    );
                    self.turns.insert(id.clone(), merged.clone());
                    if id == self.selected {
                        self.task = merged;
                        if self.busy == 0 {
                            self.message = match self.task["state"].as_str().unwrap_or("") {
                                "completed" => "CC 已完成，可以继续追问。",
                                "failed" => "本轮执行失败，请查看对话中的错误信息。",
                                "cancelled" => "本轮已停止。",
                                "interrupted" => "本轮已中断，不会自动重新派发。",
                                "waiting_approval" => "需要你确认一项操作。",
                                _ => "CC 正在处理你的消息。",
                            }
                            .into();
                        }
                    }
                    if more {
                        self.query_task(id);
                    }
                }
                "approve" | "cancel" => {
                    self.message = "操作已提交，等待远端状态更新。".into();
                    self.query_task(self.selected.clone());
                }
                "set_auto" => {
                    self.message = "权限已更新；正在运行的任务不会追溯降权。".into();
                }
                _ => {}
            }
        }
    }
    fn connection_ui(&mut self, ui: &mut egui::Ui) {
        Card::new().show(ui,|ui|{
            card_header(ui,"连接开发服务器","每个人使用自己的 SSH 账号；配置、密码和仓库映射按服务器 / 端口 / 账号区分。");
            if !self.saved.is_empty(){let labels:Vec<String>=self.saved.iter().map(|p|format!("{}@{}:{}",p.user,p.host,p.port)).collect();let options:Vec<&str>=labels.iter().map(String::as_str).collect();ui.horizontal(|ui|{ui.add(Select::new("saved_accounts",&mut self.saved_index,&options).width(360.).enabled(!self.connected));if ui.add(Button::new("载入账号").variant(ButtonVariant::Outline).enabled(!self.connected && self.busy==0)).clicked(){self.send(json!({"op":"load_profile","account_key":self.saved[self.saved_index.min(self.saved.len()-1)].account_key()}));}});}
            ui.add_enabled_ui(!self.connected && self.busy==0,|ui|{
                ui.columns(2,|cols|{field(&mut cols[0],"服务器",&mut self.profile.host,"例如 dev.example.com");field(&mut cols[1],"SSH 端口",&mut self.port_input,"填写服务器实际端口");});
                description(ui,"22 是 SSH 标准默认端口，可改为 2222 等实际端口；程序不会猜测或扫描端口。");
                ui.columns(2,|cols|{field(&mut cols[0],"用户名",&mut self.profile.user,"你自己的 SSH 用户名");cols[1].add(Label::new("密码"));cols[1].add(Input::new(&mut self.password).password(true).hint("输入或使用钥匙串已存密码").max_width(f32::INFINITY));});
                ui.horizontal(|ui|{ui.add(Switch::new(&mut self.profile.remember_password));ui.add(Label::new("记住密码到 macOS 钥匙串"));});
            });
            ui.horizontal_wrapped(|ui|{
                if ui.add(Button::new(if self.connected{"已连接"}else{"连接 SSH"}).enabled(!self.connected && self.busy==0)).clicked() && self.port_valid(){let password=std::mem::take(&mut self.password);self.send(json!({"op":"connect","profile":self.profile,"password":password}));}
                if ui.add(Button::new("保存配置").variant(ButtonVariant::Outline).enabled(!self.connected && self.busy==0)).clicked() && self.port_valid(){self.send(json!({"op":"save_profile","profile":self.profile,"password":self.password}));}
                if ui.add(Button::new("读取密码").variant(ButtonVariant::Ghost).enabled(!self.connected && self.busy==0)).clicked() && self.port_valid(){self.send(json!({"op":"load_password","profile":self.profile}));}
                if ui.add(Button::new("删除已存密码").variant(ButtonVariant::Ghost).enabled(!self.connected && self.busy==0)).clicked() && self.port_valid(){self.send(json!({"op":"forget_password","profile":self.profile}));}
                if ui.add(Button::new("断开").variant(ButtonVariant::Outline).enabled(self.connected && self.busy==0)).clicked(){self.send(json!({"op":"disconnect"}));}
                if self.busy>0{ui.spinner();}
            });
            if !self.host_candidate.is_empty(){ui.add(Badge::new("需要核对服务器身份").variant(BadgeVariant::Outline));ui.monospace(&self.host_candidate);description(ui,"请向管理员独立核对；指纹变化不能盲目接受。");if ui.add(Button::new("已核对，信任此指纹").variant(ButtonVariant::Outline)).clicked(){self.profile.fingerprint=std::mem::take(&mut self.host_candidate);self.message="指纹已设置，请重新输入密码或使用已存密码连接。".into();}}
        });
        Card::new().show(ui,|ui|{
            card_header(ui,"账号环境","登录后从远端获取真实 HOME，不拼接 /home/用户名，也不共享他人的任务目录。");
            if self.connected {for (label,key) in [("远端 HOME","home"),("实际用户","user"),("系统架构","platform")]{ui.horizontal(|ui|{ui.add(Label::new(label));ui.monospace(self.connection["environment"][key].as_str().unwrap_or("—"));});}for (label,key) in [("Runner","runner"),("任务数据","state"),("Claude Code","claude")]{ui.horizontal_wrapped(|ui|{ui.add(Label::new(label));ui.monospace(self.connection["resolved_paths"][key].as_str().filter(|s|!s.is_empty()).unwrap_or("未找到，请安装或设置路径"));});}
                if let Some(e)=self.connection["probe"]["claude_error"].as_str(){description(ui,e);}
            }else{description(ui,"连接成功后自动识别。Runner 默认：$HOME/.local/bin/ccbridge；任务数据：$HOME/.local/state/ccbridge。");}
            let installed = self.connection["probe"]["runner_version"].as_str();
            let current = installed == Some(env!("CARGO_PKG_VERSION"));
            if self.connected {
                if let Some(version) = installed {
                    ui.add(Badge::new(format!("Runner 已安装 · v{version}")).variant(BadgeVariant::Secondary));
                    description(ui, if current {"已是当前版本，无需重复安装。"} else {"与 App 配套版本不同；更新只替换 Runner 程序，保留项目、配置和任务记录。"});
                } else {
                    description(ui,"尚未确认 Runner 可用；可安装，或重新检测现有程序。");
                }
            }
            let install_label = if current {"已安装，无需更新"} else if installed.is_some() {"更新当前账号 Runner"} else {"安装当前账号 Runner"};
            ui.horizontal(|ui|{
                if ui.add(Button::new(install_label).variant(ButtonVariant::Outline).enabled(self.connected && self.busy==0 && !current)).clicked(){self.install_confirm=true;}
                if ui.add(Button::new("重新检测").variant(ButtonVariant::Ghost).enabled(self.connected && self.busy==0)).clicked(){self.send(json!({"op":"refresh_runner"}));}
            });
            if self.install_confirm{description(ui,"仅上传到上方显示的路径；原文件备份为 .previous。不使用 sudo，不安装或认证 Claude Code。");ui.horizontal(|ui|{if ui.add(Button::new("确认安装")).clicked(){self.install_confirm=false;self.send(json!({"op":"install_runner"}));}if ui.add(Button::new("取消").variant(ButtonVariant::Ghost)).clicked(){self.install_confirm=false;}});}
            ui.collapsing("高级设置 · 通常留空",|ui|{ui.add_enabled_ui(!self.connected && self.busy==0,|ui|{field(ui,"Runner 自定义绝对路径",&mut self.profile.runner,"留空，使用登录账号 HOME");field(ui,"CC 自定义绝对路径",&mut self.profile.claude,"留空，自动查找当前账号的 claude");field(ui,"CC 模型",&mut self.profile.model,"留空，沿用远端配置");field(ui,"手动远端工作区（兼容旧配置）",&mut self.profile.workspace,"优先使用仓库匹配，无需手填");field(ui,"已信任的服务器 SHA256 指纹",&mut self.profile.fingerprint,"首次连接后核对填写");});});
        });
        Card::new().show(ui,|ui|{card_header(ui,"执行权限","这里授权是否可选全自动；每轮实际模式在任务输入框选择。");let mut enabled=self.profile.allow_auto;ui.horizontal(|ui|{if ui.add(Switch::new(&mut enabled).enabled(self.busy==0)).changed(){if enabled{self.auto_confirm=true}else{self.profile.allow_auto=false;if self.connected{self.send(json!({"op":"set_auto","allow":false}));}}}ui.add(Label::new("允许选择全自动模式"));});description(ui,"全自动跳过 CC 工具审批，可使用此 SSH 账号的文件、命令和网络权限。工作目录不是 OS 沙箱。");if self.auto_confirm{description(ui,"仅对可信仓库和专用低权限账号开启。关闭开关不会追溯降权，停止已有任务请取消。");ui.horizontal(|ui|{if ui.add(Button::new("确认开启全自动")).clicked(){self.profile.allow_auto=true;self.auto_confirm=false;if self.connected{self.send(json!({"op":"set_auto","allow":true}));}}if ui.add(Button::new("保持审批").variant(ButtonVariant::Ghost)).clicked(){self.auto_confirm=false;}});}});
    }
    fn matching_ui(&mut self, ui: &mut egui::Ui) {
        Card::new().show(ui,|ui|{card_header(ui,"匹配现有仓库","选择 Mac 上当前开发的 Git 仓库，在已登录账号的远端目录中查找相同 Git remote。");
            field(ui,"本地仓库",&mut self.profile.local_repository,"选择现有仓库或粘贴绝对路径");
            ui.horizontal(|ui|{if ui.add(Button::new("选择本地仓库…").variant(ButtonVariant::Outline).enabled(self.busy==0)).clicked(){if let Some(p)=rfd::FileDialog::new().set_title("选择当前 Git 仓库").pick_folder(){self.profile.local_repository=p.to_string_lossy().into();self.send(json!({"op":"inspect_local","local_repository":self.profile.local_repository}));}}
                if ui.add(Button::new("识别本地仓库").variant(ButtonVariant::Outline).enabled(self.busy==0 && !self.profile.local_repository.trim().is_empty())).clicked(){self.send(json!({"op":"inspect_local","local_repository":self.profile.local_repository}));}
                if ui.add(Button::new("查找对应远端仓库").enabled(self.connected && self.busy==0 && !self.profile.local_repository.is_empty())).clicked(){self.send(json!({"op":"repository_match","local_repository":self.profile.local_repository,"search_root":self.profile.search_root}));}
            });
            ui.collapsing("搜索范围",|ui|{field(ui,"远端搜索根目录",&mut self.profile.search_root,"留空搜索此账号 HOME；可缩小到 HOME 下某个目录");description(ui,"只搜索当前账号 HOME 内；不跟随目录符号链接。最多 8 层 / 25,000 个目录 / 20 秒；搜索不完整时必须人工确认候选。");});
            if !self.local.is_null(){ui.add(Badge::new(self.local["name"].as_str().unwrap_or("Git 仓库")));ui.label(format!("本地分支：{} · {}",self.local["branch"].as_str().unwrap_or("—"),self.local["head"].as_str().unwrap_or("—")));for id in self.local["identities"].as_array().into_iter().flatten(){ui.monospace(id.as_str().unwrap_or(""));}if self.local["dirty"]==true{description(ui,"本地存在未提交改动；不会自动上传，远端代码可能不同。");}}
        });
        if !self.matched.is_null() {
            Card::new().show(ui,|ui|{card_header(ui,"远端匹配结果","按 Git remote 身份匹配，不以文件夹同名作为依据。分支与工作区状态会分别显示。");ui.add(Badge::new(match self.matched["state"].as_str(){Some("matched")=>"已匹配",Some("not_found")=>"未找到",_=>"需要选择"}).variant(BadgeVariant::Secondary));if self.matched["scan"]["truncated"]==true || (self.matched["scan"]["unreadable_directories"].as_u64().unwrap_or(0)>0 || self.matched["scan"]["failed_repositories"].as_u64().unwrap_or(0)>0){description(ui,"搜索未完整覆盖；请缩小范围或明确选择候选，程序不会默认采用第一个。");}
            if self.matched["binding_verified"]==true {description(ui,"已验证此账号之前保存的映射。若要换另一个 clone / worktree，请清除映射重新搜索。");if ui.add(Button::new("清除映射并重新查找").variant(ButtonVariant::Ghost).enabled(self.busy==0)).clicked(){self.send(json!({"op":"clear_binding","local_repository":self.profile.local_repository}));}}
            let candidates=self.matched["candidates"].as_array().cloned().unwrap_or_default();for c in candidates{ui.separator();ui.monospace(c["path"].as_str().unwrap_or(""));ui.label(format!("分支：{} · {} · {}",c["branch"].as_str().unwrap_or("—"),c["head"].as_str().unwrap_or("—"),if c["dirty"]==true{"有未提交改动"}else{"工作区干净"}));if ui.add(Button::new(if self.matched["selected_path"]==c["path"]{"确认并保存此映射"}else{"使用这个仓库"}).variant(ButtonVariant::Outline).enabled(self.busy==0)).clicked(){self.send(json!({"op":"bind_repository","local_repository":self.profile.local_repository,"remote_path":c["path"]}));}}
            if self.matched["state"]=="not_found"{description(ui,"当前范围内没有对应仓库。先在远端准备该仓库，或调整搜索范围后重试；不会自动 clone、切分支或同步文件。");}
        });
        }
    }
    fn integration_ui(&mut self, ui: &mut egui::Ui) {
        let exe = crate::integration::executable();
        let config = crate::integration::snippet(&exe);
        Card::new().show(ui,|ui|{card_header(ui,"1. 安装并连接","把 App 放进 /Applications；登录 SSH，在远端工作区浏览或克隆项目，选择 CC 工作目录。");description(ui,"远端 CC 需已安装并完成认证。App 可以安装自己的 Runner，但不会复制你的 Claude 凭据。");});
        Card::new().show(ui,|ui|{card_header(ui,"2. 给 Codex 添加 MCP","Codex → Settings / 设置 → MCP servers。添加 stdio 服务，命令使用下方路径，参数填写 mcp。");if ui.add(Button::new("一键配置 Codex")).clicked(){self.message=match crate::integration::configure(){Ok(m)=>m,Err(e)=>format!("配置失败：{e:#}")};}description(ui,"固定使用 /Applications/CCBridge.app；移除旧的本地 cwd，不绑定用户名或项目路径。原配置自动备份，SSH 保持连接。");ui.monospace(&config);if ui.add(Button::new("复制 config.toml 配置")).clicked(){ui.ctx().copy_text(config.clone());self.message="已复制。合并到 ~/.codex/config.toml，不覆盖已有内容；重新加载 MCP 工具后验证。".into();}description(ui,"也可把片段合并到 ~/.codex/config.toml；不要覆盖整个文件。修改后重新加载工具或新开一个 Codex 会话。");ui.collapsing("其他 MCP 客户端 / CLI",|ui|{let cmd=format!("codex mcp add ccbridge -- {} mcp",shell_quote(&exe.to_string_lossy()));ui.monospace(&cmd);if ui.add(Button::new("复制 CLI 命令").variant(ButtonVariant::Outline)).clicked(){ui.ctx().copy_text(cmd);}description(ui,"其他客户端选择 stdio：相同可执行文件 + mcp 参数。也可使用上方一键配置安全合并 Codex 全局配置。");});});
        Card::new().show(ui,|ui|{card_header(ui,"3. 验证并派发任务","让 Codex 先调用 connection_status，确认 active_remote_workspace，再调用 cc_start，无需传本地路径。");let prompt="使用 CCBridge：先调用 connection_status，确认已连接以及 App 选中的 active_remote_workspace。用 cc_start 派发审批模式任务，阅读这个远端工作区的 README，不修改文件；不传 local_repository。用 cc_result 等待结果；需要审批时提示我在 App 操作。";ui.label(prompt);if ui.add(Button::new("复制首次调用示例").variant(ButtonVariant::Outline)).clicked(){ui.ctx().copy_text(prompt.into());}description(ui,"共 9 个工具：connection_status、project_list、repository_match、cc_start、cc_status、cc_list、cc_reply、cc_cancel、cc_result。续聊用 cc_reply；全自动可用 cc_result(wait_seconds=900) 等到最终结果。");});
        Card::new().show(ui,|ui|{card_header(ui,"代码在哪里执行？","CC 在远端工作区当前选中的目录执行；可以选择具体仓库，也可以选择包含多个仓库的项目组目录。不会自动上传本地未提交文件。");description(ui,"多个匹配候选时在仓库页选择并保存；找不到时明确报告，不按目录同名猜测，不擅自 clone 或切分支。账号隔离来自独立 SSH 用户与 HOME，CC 全自动仍拥有该用户权限。");});
    }
}
fn state_label(state: &str) -> &str {
    match state {
        "queued" => "排队中",
        "running" => "执行中",
        "waiting_approval" | "awaiting_approval" => "等待审批",
        "completed" => "已完成",
        "failed" => "执行失败",
        "cancelled" => "已取消",
        "cancelling" => "正在取消",
        "interrupted" => "已中断",
        _ => "正在查询",
    }
}
fn field(ui: &mut egui::Ui, label: &str, value: &mut String, hint: &str) {
    ui.add(Label::new(label));
    ui.add(Input::new(value).hint(hint).max_width(f32::INFINITY));
}
fn load_saved() -> Vec<Profile> {
    app_dir()
        .ok()
        .and_then(|d| {
            read_json::<std::collections::BTreeMap<String, Profile>>(&d.join("profiles.json")).ok()
        })
        .map(|m| m.into_values().collect())
        .unwrap_or_default()
}
impl eframe::App for App {
    fn ui(&mut self, root: &mut egui::Ui, _: &mut eframe::Frame) {
        self.receive();
        let ctx = root.ctx().clone();
        ctx.request_repaint_after(std::time::Duration::from_millis(250));
        if self.connected && self.busy == 0 && self.background_busy == 0 {
            if self.last_connection_poll.elapsed() > std::time::Duration::from_secs(10) {
                self.last_connection_poll = std::time::Instant::now();
                self.quiet(json!({"op":"connection_status"}));
                self.quiet(json!({"op":"list"}));
            }
            if self.last_poll.elapsed() > std::time::Duration::from_secs(2) {
                self.last_poll = std::time::Instant::now();
                let state = self.task["state"].as_str().unwrap_or("");
                if !self.selected.is_empty() && !crate::conversation::terminal(state) {
                    self.query_task(self.selected.clone());
                }
            }
        }
        egui::Panel::top("header").show_inside(root, |ui| {
            ui.add_space(12.);
            ui.horizontal(|ui| {
                ui.heading("CCBridge");
                ui.add(
                    Badge::new(format!("{} · Rust", env!("CARGO_PKG_VERSION")))
                        .variant(BadgeVariant::Secondary),
                );
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.add(
                        Badge::new(if self.connected {
                            "SSH 已连接"
                        } else {
                            "未连接"
                        })
                        .variant(BadgeVariant::Outline),
                    );
                });
            });
            ui.add_space(12.);
            ui.add(
                Tabs::new(
                    &mut self.page,
                    &[
                        "连接与权限",
                        "远端工作区",
                        "任务与对话",
                        "Codex 接入",
                        "项目数据库",
                    ],
                )
                .width(f32::INFINITY),
            );
            ui.add_space(8.);
        });
        egui::Panel::bottom("status").show_inside(root, |ui| {
            ui.horizontal(|ui| {
                if self.busy > 0 {
                    ui.spinner();
                }
                description(ui, &self.message);
            });
        });
        egui::CentralPanel::default().show_inside(root, |ui| {
            if self.page == 2 {
                self.chat_ui(ui);
                return;
            }
            if !self.diagnostic.is_empty() {
                ui.collapsing("诊断详情（原始错误）", |ui| {
                    ui.label(&self.diagnostic);
                });
            }
            egui::ScrollArea::vertical().show(ui, |ui| {
                egui::Frame::NONE.inner_margin(16.).show(ui, |ui| {
                    ui.set_max_width(1050.);
                    match self.page {
                        0 => self.connection_ui(ui),
                        1 => self.explorer_ui(ui),
                        2 => self.chat_ui(ui),
                        4 => self.databases_ui(ui),
                        _ => self.integration_ui(ui),
                    }
                });
            });
        });
    }
}
pub fn run() -> Result<()> {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1100., 860.])
            .with_min_inner_size([850., 680.]),
        ..Default::default()
    };
    eframe::run_native(
        "CCBridge",
        options,
        Box::new(|cc| Ok(Box::new(App::new(cc)))),
    )
    .map_err(|e| anyhow::anyhow!("{e}"))
}
