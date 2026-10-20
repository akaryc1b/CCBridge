use super::*;
use shadcn_egui::icons::lucide::{
    ARROW_UP, FILE_TEXT, FOLDER, FOLDER_PLUS, GIT_BRANCH, REFRESH_CW,
};
impl App {
    pub(super) fn explorer_ui(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.heading("远端工作区");
            ui.add(Badge::new("SSH 文件浏览").variant(BadgeVariant::Secondary));
        });
        if !self.connected {
            description(
                ui,
                "连接 SSH 后浏览自己的目录，克隆项目，再选择 CC 工作目录；不需要先准备本地仓库。",
            );
            if ui.add(Button::new("连接服务器")).clicked() {
                self.page = 0;
            }
            return;
        }
        let home = self.connection["environment"]["home"]
            .as_str()
            .unwrap_or("")
            .to_string();
        if self.browser_path.is_empty() {
            self.browser_path = home.clone();
        }
        ui.add_space(8.);
        ui.horizontal_wrapped(|ui| {
            if ui
                .add(
                    Button::new("HOME")
                        .variant(ButtonVariant::Outline)
                        .icon(&FOLDER)
                        .enabled(self.busy == 0),
                )
                .clicked()
            {
                self.load_directory(home.clone());
            }
            if ui
                .add(
                    Button::new("上一级")
                        .variant(ButtonVariant::Ghost)
                        .icon(&ARROW_UP)
                        .enabled(self.busy == 0 && self.browser_path != home),
                )
                .clicked()
            {
                let p = std::path::Path::new(&self.browser_path)
                    .parent()
                    .unwrap_or(std::path::Path::new(&home));
                self.load_directory(p.to_string_lossy().into());
            }
            if ui
                .add(
                    Button::new("刷新")
                        .variant(ButtonVariant::Ghost)
                        .icon(&REFRESH_CW)
                        .enabled(self.busy == 0),
                )
                .clicked()
            {
                self.load_directory(self.browser_path.clone());
            }
            if ui
                .add(
                    Button::new("新建文件夹")
                        .variant(ButtonVariant::Outline)
                        .icon(&FOLDER_PLUS)
                        .enabled(self.busy == 0),
                )
                .clicked()
            {
                self.show_mkdir = !self.show_mkdir;
                self.show_clone = false;
            }
            if ui
                .add(
                    Button::new("克隆仓库")
                        .icon(&GIT_BRANCH)
                        .enabled(self.busy == 0),
                )
                .clicked()
            {
                self.show_clone = !self.show_clone;
                self.show_mkdir = false;
            }
        });
        ui.horizontal(|ui| {
            let width = (ui.available_width() - 90.).max(180.);
            ui.add(
                Input::new(&mut self.browser_path)
                    .hint("远端文件夹绝对路径")
                    .max_width(width),
            );
            if ui
                .add(
                    Button::new("前往")
                        .variant(ButtonVariant::Outline)
                        .enabled(self.busy == 0),
                )
                .clicked()
            {
                self.load_directory(self.browser_path.clone());
            }
        });
        if self.show_mkdir {
            Card::new().show(ui,|ui|{field(ui,"新文件夹名称",&mut self.new_directory,"例如 projects 或 my-project");if ui.add(Button::new("创建文件夹").enabled(self.busy==0&&!self.new_directory.is_empty())).clicked(){self.send(json!({"op":"create_directory","path":self.browser_path,"name":self.new_directory}));}});
        }
        if self.show_clone {
            Card::new().show(ui,|ui|{
            card_header(ui,"克隆到当前文件夹","多个仓库地址用空格分隔，每个仓库建立独立子目录。");
            ui.monospace(&self.browser_path);
            field(ui,"HTTPS 仓库地址",&mut self.prepare_urls,"https://github.com/组织/仓库.git");
            ui.add(Select::new("clone_auth",&mut self.clone_auth,&["服务器 Git 认证，直接克隆","本机 Git 认证，仅传仓库数据到服务器"]).width(410.));
            description(ui,if self.clone_auth==1{"本机从 Git 服务获取最新仓库，通过现有 SSH 传输 Git 数据；不上传账号密码、token、Git 配置或本地未提交文件。"}else{"使用服务器当前账号的 Git 认证。SSH 密码不等于 Codeup 仓库凭据。"});
            if ui.add(Button::new("开始克隆").enabled(self.busy==0&&!self.prepare_urls.trim().is_empty())).clicked(){self.message="正在克隆仓库，请等待；不会覆盖已有目录。".into();self.send(json!({"op":"clone_into_directory","path":self.browser_path,"urls":self.prepare_urls,"local_auth":self.clone_auth==1}));}
            if let Some(e)=self.prepared["error"].as_str(){description(ui,e);}
            for r in self.prepared["repositories"].as_array().into_iter().flatten(){ui.label(format!("{} · {}",r["name"].as_str().unwrap_or(""),if r["state"]=="cloned"{"已克隆"}else{"已存在，未修改"}));}
        });
        }
        ui.add_space(8.);
        ui.horizontal_wrapped(|ui| {
            if self.profile.active_remote_workspace.is_empty() {
                description(ui, "尚未选择 CC 工作目录");
            } else {
                ui.add(Badge::new("CC 工作目录").variant(BadgeVariant::Secondary));
                ui.monospace(&self.profile.active_remote_workspace);
            }
            if ui
                .add(
                    Button::new("将当前文件夹用作 CC 工作目录")
                        .variant(ButtonVariant::Outline)
                        .enabled(self.busy == 0),
                )
                .clicked()
            {
                self.send(json!({"op":"select_remote_workspace","path":self.browser_path}));
            }
        });
        ui.separator();
        let width = ui.available_width();
        ui.horizontal_top(|ui|{
            ui.allocate_ui_with_layout(egui::vec2((width*0.34).max(260.),460.),egui::Layout::top_down(egui::Align::Min),|ui|{
                ui.horizontal(|ui|{ui.strong("资源管理器");let mut hidden=self.show_hidden;if ui.add(Switch::new(&mut hidden).enabled(self.busy==0)).changed(){self.show_hidden=hidden;self.directories.clear();self.load_directory(home.clone());}ui.label("显示隐藏项");});
                egui::ScrollArea::both().id_salt("remote_tree").max_height(470.).show(ui,|ui|{self.directory_tree(ui,&home,"个人目录",0);});
            });
            ui.separator();
            ui.vertical(|ui|{
                ui.set_width((width*0.63).max(300.));
                if let Some(repo)=self.selected_remote.get("repository").filter(|v|!v.is_null()){
                    ui.horizontal_wrapped(|ui|{ui.add(Badge::new("Git").variant(BadgeVariant::Secondary));ui.label(format!("{} · {}",repo["branch"].as_str().unwrap_or("—"),repo["head"].as_str().unwrap_or("—")));});
                    description(ui,if repo["dirty"]==true{"远端工作区有未提交改动"}else{"远端工作区干净"});
                }
                if let Some(path)=self.file_preview["path"].as_str(){
                    ui.strong(std::path::Path::new(path).file_name().unwrap_or_default().to_string_lossy());
                    ui.monospace(path);ui.separator();
                    egui::ScrollArea::both().id_salt("source_preview").max_height(420.).show(ui,|ui|{ui.add(egui::Label::new(egui::RichText::new(self.file_preview["text"].as_str().unwrap_or("")).monospace()).selectable(true).wrap_mode(egui::TextWrapMode::Extend));});
                }else{
                    ui.heading("选择文件查看内容");
                    description(ui,"在左侧展开目录，点击源码或 README 预览；选中文件夹后可设为 CC 工作目录。");
                    description(ui,"项目组可以包含前端、后端和移动端多个独立仓库。选择组目录适合跨仓库任务；选择具体仓库适合单项目任务。");
                }
            });
        });
        if self
            .directories
            .get(&self.browser_path)
            .is_some_and(|v| v["truncated"] == true)
        {
            description(
                ui,
                "此目录条目超过 1000 个，仅显示前 1000 个；请进入更具体的子目录。",
            );
        }
        ui.separator();
        ui.collapsing("高级 · 本地仓库身份匹配（可选）", |ui| {
            self.matching_ui(ui);
        });
    }
    fn load_directory(&mut self, path: String) {
        self.browser_path = path.clone();
        self.file_preview = Value::Null;
        self.send(json!({"op":"directory_list","path":path,"hidden":self.show_hidden}));
    }
    fn directory_tree(&mut self, ui: &mut egui::Ui, path: &str, name: &str, depth: usize) {
        if depth > 12 {
            description(ui, "请用路径栏打开更深的目录");
            return;
        }
        let cached = self.directories.get(path).cloned();
        let r = egui::CollapsingHeader::new(name)
            .id_salt(("folder", path))
            .default_open(depth == 0)
            .show(ui, |ui| {
                if let Some(v) = cached {
                    let entries = v["entries"].as_array().cloned().unwrap_or_default();
                    if entries.is_empty() {
                        description(ui, "空文件夹");
                    }
                    for e in entries {
                        let p = e["path"].as_str().unwrap_or("");
                        let n = e["name"].as_str().unwrap_or("");
                        match e["kind"].as_str().unwrap_or("") {
                            "directory" => self.directory_tree(ui, p, n, depth + 1),
                            "repository" => {
                                self.directory_tree(ui, p, &format!("{n}  · Git"), depth + 1)
                            }
                            "file" => {
                                if ui
                                    .add(
                                        Button::new(n)
                                            .variant(ButtonVariant::Ghost)
                                            .icon(&FILE_TEXT)
                                            .enabled(
                                                self.busy == 0
                                                    && crate::remote_fs::preview_allowed(p),
                                            ),
                                    )
                                    .clicked()
                                {
                                    self.send(json!({"op":"file_preview","path":p}));
                                }
                            }
                            _ => {
                                ui.add(
                                    Button::new(format!("{n} · 链接 / 特殊文件"))
                                        .variant(ButtonVariant::Ghost)
                                        .enabled(false),
                                );
                            }
                        }
                    }
                } else {
                    description(ui, "展开后加载目录");
                }
            });
        if r.header_response.clicked() && self.busy == 0 {
            self.load_directory(path.into());
        }
    }
}
