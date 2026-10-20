use super::*;
use crate::conversation::{self, excerpt, terminal};
use egui_commonmark::CommonMarkViewer;
use shadcn_egui::icons::lucide::{PLUS, REFRESH_CW, SEND, SQUARE};
impl App {
    pub(super) fn chat_rounds(&self) -> Vec<Value> {
        let mut rounds: Vec<_> = self
            .tasks
            .iter()
            .filter(|t| t["session_id"].as_str() == Some(self.chat_session.as_str()))
            .cloned()
            .collect();
        rounds.sort_by_key(|t| (t["created_at"].as_u64().unwrap_or(0), t["resume"] == true));
        rounds
    }
    pub(super) fn switch_chat(&mut self, session: String) {
        let key = if self.chat_session.is_empty() {
            "__new__".into()
        } else {
            self.chat_session.clone()
        };
        self.drafts.insert(key, std::mem::take(&mut self.prompt));
        self.chat_session = session;
        self.new_chat = self.chat_session.is_empty();
        let key = if self.new_chat {
            "__new__"
        } else {
            &self.chat_session
        };
        self.prompt = self.drafts.remove(key).unwrap_or_default();
        let rounds = self.chat_rounds();
        self.mode_index = usize::from(rounds.last().is_some_and(|t| t["mode"] == "automatic"));
        self.selected = rounds
            .last()
            .and_then(|t| t["id"].as_str())
            .unwrap_or("")
            .into();
        self.task = self
            .turns
            .get(&self.selected)
            .cloned()
            .unwrap_or(Value::Null);
        for t in rounds {
            let id = t["id"].as_str().unwrap_or("");
            if !self.turns.contains_key(id) {
                self.query_task(id.into());
            }
        }
    }
    pub(super) fn chat_ui(&mut self, ui: &mut egui::Ui) {
        egui::Panel::left("chat_sessions")
            .default_size(225.)
            .min_size(200.)
            .max_size(340.)
            .resizable(true)
            .show_inside(ui, |ui| {
                ui.add_space(12.);
                ui.heading("对话");
                if ui
                    .add(Button::new("新建会话").icon(&PLUS).enabled(self.busy == 0))
                    .clicked()
                {
                    self.switch_chat(String::new());
                }
                ui.add(
                    Input::new(&mut self.chat_search)
                        .hint("搜索会话或项目")
                        .max_width(f32::INFINITY),
                );
                ui.add_space(8.);
                let mut sessions: Vec<(String, Vec<Value>)> = Vec::new();
                for t in &self.tasks {
                    let sid = t["session_id"].as_str().unwrap_or("").to_string();
                    if let Some((_, rounds)) = sessions.iter_mut().find(|(s, _)| *s == sid) {
                        rounds.push(t.clone())
                    } else {
                        sessions.push((sid, vec![t.clone()]))
                    }
                }
                egui::ScrollArea::vertical()
                    .id_salt("conversation_list")
                    .show(ui, |ui| {
                        for (sid, mut rounds) in sessions {
                            rounds.sort_by_key(|t| t["created_at"].as_u64().unwrap_or(0));
                            let first = &rounds[0];
                            let last = rounds.last().unwrap();
                            let root_id = first["id"].as_str().unwrap_or("");
                            let title = self
                                .turns
                                .get(root_id)
                                .and_then(|t| t["prompt"].as_str())
                                .or_else(|| first["title"].as_str())
                                .unwrap_or("远端会话")
                                .to_string();
                            let workspace = first["workspace"].as_str().unwrap_or("");
                            if !self.chat_search.is_empty()
                                && !format!("{title} {workspace}")
                                    .to_lowercase()
                                    .contains(&self.chat_search.to_lowercase())
                            {
                                continue;
                            }
                            ui.add_space(6.);
                            if ui
                                .add(
                                    Button::new(excerpt(&title, 17))
                                        .variant(if sid == self.chat_session {
                                            ButtonVariant::Secondary
                                        } else {
                                            ButtonVariant::Ghost
                                        })
                                        .enabled(self.busy == 0),
                                )
                                .clicked()
                            {
                                self.switch_chat(sid);
                            }
                            let folder = std::path::Path::new(workspace)
                                .file_name()
                                .unwrap_or_default()
                                .to_string_lossy();
                            description(
                                ui,
                                &format!(
                                    "{folder} · {} 轮 · {}",
                                    rounds.len(),
                                    state_label(last["state"].as_str().unwrap_or(""))
                                ),
                            );
                        }
                    });
                ui.separator();
                if ui
                    .add(
                        Button::new("刷新会话")
                            .variant(ButtonVariant::Ghost)
                            .icon(&REFRESH_CW)
                            .enabled(self.busy == 0),
                    )
                    .clicked()
                {
                    self.quiet(json!({"op":"list"}));
                }
                ui.collapsing("连接诊断", |ui| {
                    if ui
                        .add(
                            Button::new("测试 CC 连接")
                                .variant(ButtonVariant::Outline)
                                .enabled(self.connected && self.busy == 0),
                        )
                        .clicked()
                    {
                        self.send(json!({"op":"cc_diagnostic"}));
                    }
                    description(ui, "已完成的任务不会自动重新派发。");
                });
            });
        let rounds = self.chat_rounds();
        let workspace = rounds
            .last()
            .and_then(|t| t["workspace"].as_str())
            .unwrap_or(&self.profile.active_remote_workspace)
            .to_string();
        let latest = rounds
            .last()
            .and_then(|t| t["id"].as_str())
            .unwrap_or("")
            .to_string();
        let state = self
            .turns
            .get(&latest)
            .and_then(|t| t["state"].as_str())
            .or_else(|| rounds.last().and_then(|t| t["state"].as_str()))
            .unwrap_or("")
            .to_string();
        egui::Panel::top("chat_heading").show_inside(ui, |ui| {
            ui.horizontal(|ui| {
                ui.heading(if self.chat_session.is_empty() {
                    "新会话"
                } else {
                    "Claude Code"
                });
                if !state.is_empty() {
                    ui.add(Badge::new(state_label(&state)).variant(BadgeVariant::Secondary));
                }
            });
            if !workspace.is_empty() {
                ui.horizontal_wrapped(|ui| {
                    description(ui, "远端目录");
                    ui.monospace(&workspace);
                });
            }
        });
        egui::Panel::bottom("chat_composer").resizable(true).default_size(158.).min_size(130.).max_size(300.).show_inside(ui,|ui|{
            ui.add_space(8.);
            let active_round=!latest.is_empty()&&!terminal(&state);
            let mut send=false;
            ui.add_enabled_ui(self.busy==0,|ui|{
                let response=ui.add(egui::TextEdit::multiline(&mut self.prompt).hint_text(if active_round{"CC 正在回复，你可以先写好下一条消息"}else{"描述任务或继续追问…"}).desired_width(f32::INFINITY).desired_rows(3).font(egui::TextStyle::Body));
                if response.has_focus()&&ui.input(|i|i.modifiers.command&&i.key_pressed(egui::Key::Enter)){send=true;}
            });
            ui.horizontal_wrapped(|ui|{
                ui.add_enabled_ui(!active_round && self.busy==0, |ui| {ui.add(Select::new("chat_mode",&mut self.mode_index,&["审批模式","全自动模式"]).width(180.));});
                if active_round {description(ui,"本轮权限已固定；完成或停止后可为下一轮切换。");}
                else if self.mode_index==1 {description(ui,if self.profile.allow_auto {"下一轮不再逐项审批，可直接修改远端文件和执行命令。"} else {"请先在连接与权限中开启全自动授权。"});}
                let workspace_ready=!rounds.is_empty()||!self.profile.active_remote_workspace.is_empty();
                let ready=self.connected&&self.busy==0&&!active_round&&!self.prompt.trim().is_empty()&&workspace_ready&&(self.mode_index==0||self.profile.allow_auto);
                if ui.add(Button::new(if rounds.is_empty(){"发送任务"}else{"发送追问"}).icon(&SEND).enabled(ready)).clicked(){send=true;}
                if active_round&&ui.add(Button::new("停止本轮").variant(ButtonVariant::Destructive).icon(&SQUARE).enabled(self.connected&&self.busy==0)).clicked(){self.send(json!({"op":"cancel","task_id":latest}));}
                if !workspace_ready {if ui.add(Button::new("选择远端文件夹").variant(ButtonVariant::Link)).clicked(){self.page=1;}}
                if ready{description(ui,"⌘ Enter 发送");}
                if send&&ready {
                    if latest.is_empty(){self.send(json!({"op":"start","task_id":uuid::Uuid::new_v4().to_string(),"prompt":self.prompt,"mode":if self.mode_index==0{"approval"}else{"automatic"}}));}
                    else{self.send(json!({"op":"reply","task_id":uuid::Uuid::new_v4().to_string(),"reply_to":latest,"prompt":self.prompt,"mode":if self.mode_index==0{"approval"}else{"automatic"}}));}
                }
            });
        });
        egui::CentralPanel::default().show_inside(ui,|ui|{
            ui.style_mut().text_styles.get_mut(&egui::TextStyle::Body).unwrap().size=16.;
            ui.style_mut().spacing.item_spacing.y=8.;
            ui.style_mut().wrap_mode=Some(egui::TextWrapMode::Wrap);
            let message_width=ui.available_width().min(820.);
            egui::ScrollArea::vertical().id_salt(("chat_messages",&self.chat_session)).stick_to_bottom(true).show(ui,|ui|{
                ui.set_width(message_width);ui.add_space(16.);
                if rounds.is_empty(){ui.heading("在远端项目中开始对话");description(ui,if workspace.is_empty(){"先在远端工作区选择一个文件夹，再描述你想完成的任务。"}else{"可以先让 CC 阅读 README、解释项目结构，或直接描述要完成的修改。"});}
                for metadata in rounds {
                    let id=metadata["id"].as_str().unwrap_or("").to_string();
                    let turn=self.turns.get(&id).cloned().unwrap_or(metadata.clone());
                    ui.push_id(&id,|ui|{
                        ui.set_width(message_width);
                        ui.strong("你");
                        let prompt=turn["prompt"].as_str().or_else(||metadata["title"].as_str()).unwrap_or("正在读取历史消息…");
                        CommonMarkViewer::new().show(ui,&mut self.markdown,prompt);
                        ui.add_space(14.);ui.strong("Claude Code");
                        let text=conversation::assistant_text(&turn);
                        if !text.is_empty(){CommonMarkViewer::new().show(ui,&mut self.markdown,&text);}
                        let status=turn["state"].as_str().unwrap_or("");
                        if !terminal(status)&&!status.is_empty(){ui.horizontal(|ui|{ui.spinner();description(ui,if status=="waiting_approval"{"等待你确认操作"}else{"正在处理…"});});}
                        let tools=conversation::tools(&turn);
                        if !tools.is_empty(){ui.collapsing(format!("执行过程 · {} 次工具操作",tools.len()),|ui|{for t in tools{let name=t["name"].as_str().unwrap_or("工具");ui.label(conversation::tool_label(name));if let Some(p)=t["input"]["file_path"].as_str().or_else(||t["input"]["pattern"].as_str()){ui.monospace(p);}if let Some(cmd)=t["input"]["command"].as_str(){ui.monospace(cmd);}}});}
                        for a in turn["approvals"].as_array().into_iter().flatten(){Card::new().show(ui,|ui|{
                            card_header(ui,"需要你确认",conversation::tool_label(a["tool_name"].as_str().unwrap_or("工具操作")));
                            if let Some(p)=a["input"]["file_path"].as_str(){ui.monospace(p);}if let Some(cmd)=a["input"]["command"].as_str(){ui.monospace(cmd);}
                            ui.collapsing("查看完整操作内容",|ui|{ui.monospace(serde_json::to_string_pretty(&a["input"]).unwrap_or_default());});
                            ui.horizontal(|ui|{for (label,allow) in [("允许本次",true),("拒绝本次",false)]{if ui.add(Button::new(label).variant(if allow{ButtonVariant::Default}else{ButtonVariant::Outline}).enabled(self.busy==0)).clicked(){self.send(json!({"op":"approve","task_id":id,"request_id":a["request_id"],"allow":allow}));}}});
                        });}
                        if let Some(error)=turn["error"].as_str(){ui.label(error_summary(error));ui.collapsing("错误详情",|ui|{ui.label(error);});}
                        ui.collapsing("技术详情",|ui|{ui.monospace(format!("任务：{id}"));ui.monospace(format!("会话：{}",turn["session_id"].as_str().unwrap_or("")));ui.collapsing("原始事件",|ui|{ui.monospace(serde_json::to_string_pretty(&turn["events"]).unwrap_or_default());});});
                        ui.add_space(18.);ui.separator();ui.add_space(18.);
                    });
                }
            });
        });
    }
}
