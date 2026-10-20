use super::*;
pub(super) struct DatabaseUi {
    pub data: Value,
    pub draft: crate::databases::Database,
    pub workspace: String,
    pub project_name: String,
    pub mapping_drafts: std::collections::BTreeMap<String, Vec<crate::databases::Binding>>,
    pub alias: String,
    pub index: usize,
    pub bindings: Vec<crate::databases::Binding>,
    pub loaded: bool,
}
impl Default for DatabaseUi {
    fn default() -> Self {
        Self {
            data: Value::Null,
            draft: crate::databases::Database {
                kind: "mysql".into(),
                port: 3306,
                ..Default::default()
            },
            workspace: String::new(),
            project_name: String::new(),
            mapping_drafts: Default::default(),
            alias: String::new(),
            index: 0,
            bindings: vec![],
            loaded: false,
        }
    }
}
impl App {
    pub(super) fn databases_ui(&mut self, ui: &mut egui::Ui) {
        ui.heading("项目与数据库");
        description(
            ui,
            "每个 SSH 账号独立保存连接；一个项目可映射多个数据库。仅将本项目启用的连接交给 CC，固定只读，不改 /etc 共享配置。",
        );
        description(
            ui,
            "需要新版 Runner。连接密码通过 SSH 保存到当前账号 HOME 的私有文件，不放进聊天、任务指令或 MCP 参数；同一 SSH 用户仍能读取自己的文件。",
        );
        if self.db.workspace.is_empty() {
            self.db.workspace = self.profile.active_remote_workspace.clone();
        }
        let projects = self.db.data["projects"]
            .as_array()
            .cloned()
            .unwrap_or_default();
        ui.heading("项目列表");
        for project in &projects {
            ui.horizontal_wrapped(|ui| {
                let path = project["path"].as_str().unwrap_or("");
                let name = project["name"].as_str().unwrap_or("");
                if ui
                    .add(
                        Button::new(name)
                            .variant(if path == self.db.workspace {
                                ButtonVariant::Default
                            } else {
                                ButtonVariant::Outline
                            })
                            .enabled(self.busy == 0),
                    )
                    .clicked()
                {
                    self.db
                        .mapping_drafts
                        .insert(self.db.workspace.clone(), self.db.bindings.clone());
                    self.db.workspace = path.into();
                    self.db.project_name = name.into();
                    self.db.loaded = false;
                    self.send(json!({"op":"db_list","workspace":path}));
                }
                description(
                    ui,
                    &format!(
                        "{} · {} 个数据库",
                        path,
                        project["databases"].as_array().map_or(0, Vec::len)
                    ),
                );
            });
        }
        description(
            ui,
            &format!("当前 CC 项目：{}", self.profile.active_remote_workspace),
        );
        ui.horizontal(|ui| {
            ui.label("项目名称");
            ui.add(Input::new(&mut self.db.project_name).max_width(450.));
        });
        ui.horizontal(|ui| {
            ui.label("远端项目目录");
            ui.add(Input::new(&mut self.db.workspace).max_width(650.));
        });
        ui.horizontal_wrapped(|ui|{
            if ui.add(Button::new("保存 / 添加项目").enabled(self.connected&&self.busy==0&&!self.db.project_name.trim().is_empty())).clicked(){self.send(json!({"op":"db_project_save","workspace":self.db.workspace,"name":self.db.project_name}));}
            if ui.add(Button::new("用此项目执行新 CC 任务").variant(ButtonVariant::Outline).enabled(self.connected&&self.busy==0)).clicked(){self.send(json!({"op":"select_remote_workspace","path":self.db.workspace}));}
        });
        description(
            ui,
            "项目名称与目录独立保存，下面编辑该项目的数据库映射。切换项目保留未保存的映射草稿；保存项目映射后新会话才生效。",
        );
        ui.horizontal(|ui| {
            if ui
                .add(Button::new("载入项目映射").enabled(self.connected && self.busy == 0))
                .clicked()
            {
                self.send(json!({"op":"db_list","workspace":self.db.workspace}));
            }
            if ui
                .add(
                    Button::new("探测远端 MCP 安装")
                        .variant(ButtonVariant::Outline)
                        .enabled(self.connected && self.busy == 0),
                )
                .clicked()
            {
                self.send(json!({"op":"db_detect"}));
            }
            if ui
                .add(Button::new("新建连接").variant(ButtonVariant::Outline))
                .clicked()
            {
                self.db.draft = crate::databases::Database {
                    kind: "mysql".into(),
                    port: 3306,
                    node: self.db.draft.node.clone(),
                    package_root: self.db.draft.package_root.clone(),
                    ..Default::default()
                };
            }
        });
        ui.separator();
        let ds = self.db.data["connections"]
            .as_array()
            .cloned()
            .unwrap_or_default();
        if ds.is_empty() {
            description(
                ui,
                "尚未载入连接，或当前账号没有配置。先探测安装，再添加连接。",
            );
        }
        for d in &ds {
            ui.horizontal(|ui| {
                if ui
                    .add(
                        Button::new(d["name"].as_str().unwrap_or("未命名"))
                            .variant(ButtonVariant::Outline),
                    )
                    .clicked()
                {
                    self.db.draft = serde_json::from_value(d.clone()).unwrap_or_default();
                    self.db.draft.password.clear();
                }
                description(
                    ui,
                    &format!(
                        "{} · {}:{} / {} · {}",
                        d["kind"].as_str().unwrap_or(""),
                        d["host"].as_str().unwrap_or(""),
                        d["port"],
                        d["database"].as_str().unwrap_or(""),
                        if d["enabled"] == true {
                            "已启用"
                        } else {
                            "未启用"
                        }
                    ),
                );
            });
        }
        Card::new().show(ui,|ui|{
   card_header(ui,"连接配置","密码留空会保留原密码；仅保存不会测试连接。首次启用后，新任务才会使用它。");
   egui::Grid::new("db_fields").num_columns(2).spacing([20.,12.]).show(ui,|ui|{
    ui.label("连接名称");ui.add(Input::new(&mut self.db.draft.name).max_width(450.));ui.end_row();
    ui.label("数据库类型");let mut kind=usize::from(self.db.draft.kind=="postgres");let before=kind;ui.add(Select::new("db_kind",&mut kind,&["MySQL","PostgreSQL"]).width(260.));if kind!=before{self.db.draft.kind=if kind==0{"mysql"}else{"postgres"}.into();self.db.draft.port=if kind==0{3306}else{5432};}ui.end_row();
    for (label,value) in [("数据库地址（从服务器访问）",&mut self.db.draft.host),("数据库用户名",&mut self.db.draft.user),("数据库名称",&mut self.db.draft.database)]{ui.label(label);ui.add(Input::new(value).max_width(450.));ui.end_row();}
    ui.label("端口");ui.add(egui::DragValue::new(&mut self.db.draft.port).range(1..=65535));ui.end_row();
    ui.label("数据库密码（不回显）");ui.add(egui::TextEdit::singleline(&mut self.db.draft.password).password(true).desired_width(450.));ui.end_row();
   });
   ui.collapsing("远端运行环境（探测结果 / 手动修正）",|ui|{for (label,value) in [("Node 绝对路径",&mut self.db.draft.node),("universal-db-mcp 2.14.0 安装目录",&mut self.db.draft.package_root)]{ui.label(label);ui.add(Input::new(value).max_width(650.));}});
   ui.horizontal(|ui|{ui.add(Switch::new(&mut self.db.draft.enabled));ui.label("启用此连接 · 固定只读 safe");});
   description(ui,"建议使用数据库自身只有 SELECT 权限的账号。全自动任务也不能通过此实例切换连接或提高数据库权限。数据库自身权限是最终边界。");
   if ui.add(Button::new("测试已保存连接").variant(ButtonVariant::Outline).enabled(self.connected&&self.busy==0&&!self.db.draft.id.is_empty())).clicked(){self.send(json!({"op":"db_test","id":self.db.draft.id}));}
   if ui.add(Button::new("保存到当前 SSH 账号").enabled(self.connected&&self.busy==0)).clicked(){self.send(json!({"op":"db_save","connection":self.db.draft}));self.db.draft.password.clear();}
  });
        ui.add_space(12.);
        Card::new().show(ui,|ui|{
   card_header(ui,"项目数据库映射","用途别名会变成 CC 工具前缀：例如 business_dev → db_business_dev。绑定到项目组时跨三个仓库可用；绑定具体仓库时仅该仓库任务可用，不自动继承父目录。");
   let mut remove=None;
   for (i,b) in self.db.bindings.iter().enumerate(){ui.horizontal(|ui|{ui.label(&b.alias);ui.label(ds.iter().find(|d|d["id"]==b.connection_id).and_then(|d|d["name"].as_str()).unwrap_or("连接已失效"));if ui.add(Button::new("移除映射").variant(ButtonVariant::Ghost)).clicked(){remove=Some(i);}});}
   if let Some(i)=remove{self.db.bindings.remove(i);}
   ui.label("用途别名（英文、数字、下划线）");ui.add(Input::new(&mut self.db.alias).max_width(300.));
   if !ds.is_empty(){self.db.index=self.db.index.min(ds.len()-1);let names:Vec<_>=ds.iter().map(|d|d["name"].as_str().unwrap_or("未命名")).collect();ui.add(Select::new("db_binding_connection",&mut self.db.index,&names).width(400.));if ui.add(Button::new("添加到映射草稿").variant(ButtonVariant::Outline)).clicked(){self.db.bindings.push(crate::databases::Binding{alias:std::mem::take(&mut self.db.alias),connection_id:ds[self.db.index]["id"].as_str().unwrap_or("").into()});}}
   if ui.add(Button::new("保存项目映射").enabled(self.connected&&self.busy==0&&self.db.loaded&&self.db.data["workspace"].as_str()==Some(self.db.workspace.as_str()))).clicked(){self.send(json!({"op":"db_bind","workspace":self.db.workspace,"bindings":self.db.bindings}));}
   description(ui,"映射保存后用于新会话；续聊固定使用首轮的连接快照。改连接或项目后要新建会话，避免同一对话突然换库。未启用连接不启动数据库 MCP。");
  });
    }
}
