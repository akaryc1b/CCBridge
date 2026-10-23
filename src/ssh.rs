use crate::common::*;
use anyhow::{Context, Result, bail};
use russh::{ChannelMsg, client, keys::HashAlg};
use serde_json::{Value, json};
use std::sync::{Arc, Mutex};
use std::time::Duration;

pub struct HostCheck {
    pub expected: String,
    pub seen: Arc<Mutex<String>>,
}
impl client::Handler for HostCheck {
    type Error = russh::Error;
    async fn check_server_key(
        &mut self,
        key: &russh::keys::PublicKeyOrCertificate,
    ) -> std::result::Result<bool, Self::Error> {
        let fp = key.public_key().fingerprint(HashAlg::Sha256).to_string();
        *self.seen.lock().unwrap() = fp.clone();
        Ok(!self.expected.is_empty() && self.expected == fp)
    }
}
#[derive(Clone, serde::Serialize, serde::Deserialize)]
pub struct RemoteEnvironment {
    pub home: String,
    pub user: String,
    pub claude: String,
    pub platform: String,
    #[serde(default)]
    pub local_app_data: String,
}
#[derive(Clone)]
pub struct Connection {
    pub connection_id: String,
    pub handle: Arc<client::Handle<HostCheck>>,
    pub profile: Profile,
    pub probe: Value,
    pub environment: RemoteEnvironment,
    pub runner_path: String,
    pub claude_path: String,
}
pub async fn connect(p: Profile, password: String) -> Result<Connection> {
    p.validate()?;
    let seen = Arc::new(Mutex::new(String::new()));
    let config = client::Config {
        keepalive_interval: Some(Duration::from_secs(20)),
        keepalive_max: 3,
        ..Default::default()
    };
    let r = tokio::time::timeout(
        Duration::from_secs(20),
        client::connect(
            Arc::new(config),
            (p.host.as_str(), p.port),
            HostCheck {
                expected: p.fingerprint.clone(),
                seen: seen.clone(),
            },
        ),
    )
    .await
    .context("SSH 连接超时")?;
    let mut handle = match r {
        Ok(h) => h,
        Err(e) => {
            let fp = seen.lock().unwrap().clone();
            if !fp.is_empty() && fp != p.fingerprint {
                bail!(
                    "HOST_KEY:{fp}\n服务器指纹未确认或与已保存值不一致。请通过管理员核对；不要盲目接受变化。"
                )
            }
            return Err(e.into());
        }
    };
    let auth = tokio::time::timeout(
        Duration::from_secs(20),
        handle.authenticate_password(&p.user, &password),
    )
    .await
    .context("SSH 认证超时")??;
    drop(password);
    if !auth.success() {
        bail!(
            "SSH 密码认证失败。检查账号密码和服务端 PasswordAuthentication；首版不支持 MFA/keyboard-interactive。"
        )
    }
    let mut conn = Connection {
        connection_id: uuid::Uuid::new_v4().to_string(),
        handle: Arc::new(handle),
        profile: p,
        probe: Value::Null,
        environment: RemoteEnvironment {
            home: String::new(),
            user: String::new(),
            claude: String::new(),
            platform: String::new(),
            local_app_data: String::new(),
        },
        runner_path: String::new(),
        claude_path: String::new(),
    };
    let script = r#"# ccbridge:discover
h=$(cd "$HOME" && pwd -P) || exit 1
c=$(command -v claude 2>/dev/null || true)
if [ -z "$c" ]; then
  for f in "$h/.local/bin/claude" "$h/.claude/local/claude" /opt/homebrew/bin/claude /usr/local/bin/claude; do
    if [ -x "$f" ]; then c=$f; break; fi
  done
fi
printf '%s\000%s\000%s\000%s\000' "$h" "$(id -un)" "$c" "$(uname -sm)"
"#;
    let (out, err, status) = conn.execute(script.into(), &[]).await?;
    conn.environment = if status == Some(0) && out.first() == Some(&b'/') {
        let parts: Vec<_> = out.split(|b| *b == 0).collect();
        if parts.len() != 5 {
            bail!("远端登录环境返回无效；请检查 shell 是否向 stdout 输出额外文本")
        }
        let values: Vec<String> = parts[..4]
            .iter()
            .map(|s| String::from_utf8_lossy(s).to_string())
            .collect();
        if !values[0].starts_with('/') || values[0].contains('\n') {
            bail!("远端 HOME 不是有效绝对路径")
        }
        let environment = RemoteEnvironment {
            home: values[0].clone(),
            user: values[1].clone(),
            claude: values[2].clone(),
            platform: values[3].clone(),
            local_app_data: String::new(),
        };
        environment
    } else {
        let (win, winerr, code) = conn
            .execute(
                crate::remote_platform::powershell(crate::remote_platform::DISCOVER_WINDOWS),
                &[],
            )
            .await?;
        if code != Some(0) {
            bail!(
                "远端系统探测失败；POSIX：{}；Windows：{}",
                String::from_utf8_lossy(&err),
                String::from_utf8_lossy(&winerr)
            );
        }
        let env: RemoteEnvironment =
            serde_json::from_slice(&win).context("Windows 环境响应无效")?;
        if !crate::remote_platform::windows_absolute(&env.home)
            || !crate::remote_platform::windows_absolute(&env.local_app_data)
        {
            bail!("Windows 用户目录无效");
        }
        env
    };
    conn.runner_path = if conn.profile.runner.is_empty() {
        if conn.is_windows() {
            format!(
                "{}\\CCBridge\\runner\\{}\\ccbridge.exe",
                conn.environment.local_app_data,
                env!("CARGO_PKG_VERSION")
            )
        } else {
            format!("{}/.local/bin/ccbridge", conn.environment.home)
        }
    } else {
        conn.profile.runner.clone()
    };
    conn.claude_path = if conn.profile.claude.is_empty() {
        conn.environment.claude.clone()
    } else {
        conn.profile.claude.clone()
    };
    conn.refresh().await;
    Ok(conn)
}
impl Connection {
    fn is_windows(&self) -> bool {
        self.environment.platform.starts_with("Windows ")
    }
    fn state_path(&self) -> String {
        if self.is_windows() {
            format!("{}\\CCBridge\\state", self.environment.local_app_data)
        } else {
            format!("{}/.local/state/ccbridge", self.environment.home)
        }
    }
    fn rpc_command(&self) -> Result<String> {
        if self.is_windows() {
            crate::remote_platform::runner_command(&self.runner_path, &self.state_path())
        } else {
            Ok(format!(
                "env CCBRIDGE_RUNNER_HOME={} {} rpc",
                shell_quote(&self.state_path()),
                shell_quote(&self.runner_path)
            ))
        }
    }
    pub async fn refresh(&mut self) {
        self.probe = match self
            .rpc(json!({"op":"probe","workspace":self.profile.workspace,"claude":self.claude_path}))
            .await
        {
            Ok(v) => v,
            Err(e) => json!({"probe_error":format!("{e:#}")}),
        };
        if let Some(w) = self.probe["workspace"].as_str() {
            self.profile.workspace = w.into();
        }
    }
    pub fn info(&self) -> Value {
        json!({"connection_id":self.connection_id,"app_version":env!("CARGO_PKG_VERSION"),"bridge_protocol":1,"bridge_capabilities":["client_activity_lease","update_drain","models_v1"],"connected":!self.handle.is_closed(),"profile":self.profile,"probe":self.probe,"environment":self.environment,"resolved_paths":{"runner":self.runner_path,"claude":self.claude_path,"state":self.state_path()}})
    }
    pub fn permits_workspace(&self, path: &str) -> bool {
        (!self.profile.workspace.is_empty() && path == self.profile.workspace)
            || crate::remote_platform::within(path, &self.environment.home)
    }
    pub async fn match_repository(&self, local_path: &str) -> Result<Value> {
        let local = crate::repository::inspect(std::path::Path::new(local_path))
            .context("本地目录不是可读取的 Git 仓库")?;
        if local.identities.is_empty() {
            bail!(
                "本地仓库没有可匹配的网络 Git remote；请配置 remote，或在高级设置明确指定远端目录"
            )
        }
        if let Some(binding) = self.profile.bindings.iter().find(|b| {
            b.account == self.profile.account_key()
                && b.local_path == local.path
                && local.identities.contains(&b.identity)
        }) {
            let remote: crate::repository::Repository = serde_json::from_value(
                self.rpc(json!({"op":"repository_inspect","path":binding.remote_path}))
                    .await
                    .context("已保存的仓库映射无法验证，请在 App 清除映射后重新查找")?,
            )?;
            if !crate::repository::same(&local, &remote) {
                bail!("已保存的远端目录 Git 身份发生变化，请清除映射后重新选择")
            }
            return Ok(
                json!({"state":"matched","local":local,"candidates":[remote],"selected_path":binding.remote_path,"binding_verified":true,"scan":{"root":binding.remote_path,"scope":"saved_mapping","visited":1,"truncated":false,"unreadable_directories":0,"failed_repositories":0},"notice":"验证已保存的账号仓库映射；不重新扫描整个 HOME，不同步文件或切换分支"}),
            );
        }
        let scan = self
            .rpc(json!({"op":"repositories","root":self.profile.search_root}))
            .await?;
        let all: Vec<crate::repository::Repository> =
            serde_json::from_value(scan["repositories"].clone())?;
        let candidates: Vec<_> = all
            .into_iter()
            .filter(|r| crate::repository::same(&local, r))
            .collect();
        let binding = self.profile.bindings.iter().find(|b| {
            b.account == self.profile.account_key()
                && b.local_path == local.path
                && local.identities.contains(&b.identity)
                && candidates.iter().any(|r| r.path == b.remote_path)
        });
        let selected = if let Some(b) = binding {
            Some(b.remote_path.clone())
        } else if candidates.len() == 1
            && scan["truncated"] != true
            && scan["unreadable_directories"] == 0
            && scan["failed_repositories"] == 0
        {
            Some(candidates[0].path.clone())
        } else {
            None
        };
        let state = if selected.is_some() {
            "matched"
        } else if candidates.is_empty() {
            "not_found"
        } else {
            "selection_required"
        };
        Ok(
            json!({"state":state,"local":local,"candidates":candidates,"selected_path":selected,"scan":{"root":scan["root"],"visited":scan["visited"],"truncated":scan["truncated"],"unreadable_directories":scan["unreadable_directories"],"failed_repositories":scan["failed_repositories"]},"notice":"按 Git remote 身份匹配；本地未提交改动不会上传，远端分支不会自动切换"}),
        )
    }

    async fn execute(
        &self,
        command: String,
        input: &[u8],
    ) -> Result<(Vec<u8>, Vec<u8>, Option<u32>)> {
        self.execute_for(command, input, Duration::from_secs(60))
            .await
    }
    async fn execute_for(
        &self,
        command: String,
        input: &[u8],
        duration: Duration,
    ) -> Result<(Vec<u8>, Vec<u8>, Option<u32>)> {
        self.execute_reader(command, input, duration).await
    }
    async fn execute_reader<R: tokio::io::AsyncRead + Unpin>(
        &self,
        command: String,
        input: R,
        duration: Duration,
    ) -> Result<(Vec<u8>, Vec<u8>, Option<u32>)> {
        self.execute_streaming(command, input, duration, None).await
    }
    async fn execute_streaming<R: tokio::io::AsyncRead + Unpin>(
        &self,
        command: String,
        input: R,
        duration: Duration,
        progress: Option<Arc<dyn Fn(Value) + Send + Sync>>,
    ) -> Result<(Vec<u8>, Vec<u8>, Option<u32>)> {
        tokio::time::timeout(duration, async {
            let mut channel = self.handle.channel_open_session().await?;
            channel.exec(true, command).await?;
            channel.data(input).await?;
            channel.eof().await?;
            let mut stdout = vec![];
            let mut stderr = vec![];
            let mut exit = None;
            let mut progress_line = Vec::new();
            while let Some(msg) = channel.wait().await {
                match msg {
                    ChannelMsg::Data { data } => stdout.extend_from_slice(&data),
                    ChannelMsg::ExtendedData { data, .. } => {
                        stderr.extend_from_slice(&data);
                        if progress.is_some() {
                            for byte in data.iter() {
                                if *byte == b'\r' || *byte == b'\n' {
                                    if let Some(v) = crate::operations::git_progress(
                                        &String::from_utf8_lossy(&progress_line),
                                    ) {
                                        if let Some(f) = &progress {
                                            f(v)
                                        }
                                    }
                                    progress_line.clear();
                                } else if progress_line.len() < 2048 {
                                    progress_line.push(*byte);
                                }
                            }
                        }
                        if progress.is_some() && stderr.len() > 65536 {
                            stderr.drain(..stderr.len() - 65536);
                        }
                    }
                    ChannelMsg::ExitStatus { exit_status } => exit = Some(exit_status),
                    _ => {}
                }
                if stdout.len() + stderr.len() > 8 * 1024 * 1024 {
                    bail!("远端响应超过 8 MiB，请分段查询")
                }
            }
            Ok((stdout, stderr, exit))
        })
        .await
        .context("SSH 请求超时；操作结果不确定，请先查询，勿重复执行")?
    }
    pub async fn browse(&self, path: &str, hidden: bool) -> Result<Value> {
        let path = if path.is_empty() {
            &self.environment.home
        } else {
            path
        };
        if self.is_windows() {
            return self
                .rpc(json!({"op":"fs_list","path":path,"hidden":hidden}))
                .await;
        }
        let script = crate::remote_fs::list_script(&self.environment.home, path, hidden)?;
        let (out, err, status) = self.execute(script, &[]).await?;
        if status != Some(0) {
            bail!(
                "{}",
                crate::remote_fs::failure(&String::from_utf8_lossy(&err))
            )
        }
        crate::remote_fs::parse_list(&out)
    }
    pub async fn mkdir(&self, parent: &str, name: &str) -> Result<Value> {
        if self.is_windows() {
            return self
                .rpc(json!({"op":"fs_mkdir","path":parent,"name":name}))
                .await;
        }
        let cmd = crate::remote_fs::mkdir_script(&self.environment.home, parent, name)?;
        let (_, err, status) = self.execute(cmd, &[]).await?;
        if status != Some(0) {
            bail!(
                "{}",
                crate::remote_fs::failure(&String::from_utf8_lossy(&err))
            )
        }
        self.browse(parent, false).await
    }
    pub async fn preview_file(&self, path: &str) -> Result<Value> {
        if self.is_windows() {
            return self.rpc(json!({"op":"fs_preview","path":path})).await;
        }
        let cmd = crate::remote_fs::preview_script(&self.environment.home, path)?;
        let (out, err, status) = self.execute(cmd, &[]).await?;
        if status != Some(0) {
            bail!(
                "{}",
                crate::remote_fs::failure(&String::from_utf8_lossy(&err))
            )
        }
        if out.len() > 131072 {
            bail!("文件超过 128 KiB，内容预览未加载")
        }
        if out.contains(&0) {
            bail!("二进制文件不提供文本预览")
        }
        let text = String::from_utf8(out).context("文件不是 UTF-8 文本，未加载预览")?;
        Ok(json!({"path":path,"text":text}))
    }
    pub async fn clone_into(&self, parent: &str, urls: &str, local_auth: bool) -> Result<Value> {
        self.clone_into_progress(parent, urls, local_auth, None)
            .await
    }
    pub async fn clone_into_progress(
        &self,
        parent: &str,
        urls: &str,
        local_auth: bool,
        progress: Option<std::sync::Arc<dyn Fn(&str, usize, usize) + Send + Sync>>,
    ) -> Result<Value> {
        self.clone_observed(parent, urls, local_auth, progress, None)
            .await
    }
    pub async fn clone_observed(
        &self,
        parent: &str,
        urls: &str,
        local_auth: bool,
        progress: Option<Arc<dyn Fn(&str, usize, usize) + Send + Sync>>,
        operation: Option<crate::operations::Operation>,
    ) -> Result<Value> {
        let report = |stage: &str, completed, total| {
            if let Some(f) = &progress {
                f(stage, completed, total);
            }
        };
        report("检查目标目录", 0, 0);
        let listing = self.browse(parent, false).await?;
        let parent = listing["path"].as_str().unwrap();
        let plan = crate::provision::plan("workspace", urls)?;
        let mut prepared = vec![];
        let total = plan.len();
        for (index, (name, url)) in plan.into_iter().enumerate() {
            report(&format!("检查仓库 {name}"), index, total);
            let dest = format!("{parent}/{name}");
            if let Some(o) = &operation {
                o.patch(json!({"total":total,"processed":index,"name":name,"phase":"checking","objects":null,"bytes":null}));
            }
            if let Ok(repo) = self
                .rpc(json!({"op":"repository_inspect","path":dest}))
                .await
            {
                let id = crate::repository::identity(&url).unwrap_or_default();
                if !repo["path"].as_str().is_some_and(|p| {
                    crate::remote_platform::within(p, &dest)
                        && crate::remote_platform::within(&dest, p)
                }) || !repo["identities"]
                    .as_array()
                    .map(|a| a.iter().any(|v| v.as_str() == Some(&id)))
                    .unwrap_or(false)
                {
                    bail!("同名目录已存在，但不是指定仓库，未覆盖")
                }
                prepared.push(
                    json!({"name":name,"path":dest,"state":"already_exists","repository":repo}),
                );
                if let Some(o) = &operation {
                    o.patch(json!({"processed":index+1,"items":prepared}));
                }
                continue;
            }
            if self.is_windows() && !local_auth {
                report(&format!("{name}：使用服务器 Git 认证克隆"), index, total);
                if let Some(o) = &operation {
                    o.patch(json!({"phase":"fetch","name":name}));
                }
                let credential = self.remote_git_credential(&url).await?;
                let cb = operation.clone().map(|o| {
                    Arc::new(move |p: Value| o.patch(p)) as Arc<dyn Fn(Value) + Send + Sync>
                });
                self.rpc_progress(json!({"op":"git_native_clone","path":parent,"name":name,"url":url,"credential":credential}),cb).await?;
                let repo = self
                    .rpc(json!({"op":"repository_inspect","path":dest}))
                    .await?;
                prepared.push(json!({"name":name,"path":dest,"state":"cloned","repository":repo}));
                if let Some(o) = &operation {
                    o.patch(json!({"processed":index+1,"items":prepared}));
                }
                continue;
            }
            let (_, err, status) = if local_auth {
                report(
                    &format!("{name}：使用本机 Git 认证获取仓库（可能等待钥匙串授权）"),
                    index,
                    total,
                );
                let u = url.clone();
                let cred = if crate::git_auth::host(&url).is_ok() {
                    crate::git_auth::load("local", &self.profile).await?
                } else {
                    None
                };
                if let Some(o) = &operation {
                    o.patch(json!({"phase":"fetch","name":name}));
                }
                let bundle = tokio::task::spawn_blocking(move || {
                    crate::transfer::bundle_with_auth(&u, cred.as_ref())
                })
                .await??;
                report(&format!("{name}：通过 SSH 传输并导入仓库"), index, total);
                let cmd = if self.is_windows() {
                    crate::windows_bootstrap::import_bundle(parent, &name, &url)?
                } else {
                    crate::transfer::import_script(&self.environment.home, parent, &name, &url)?
                };
                let input = tokio::fs::File::open(&bundle.path).await?;
                let total_bytes = input.metadata().await?.len();
                if let Some(o) = &operation {
                    o.patch(json!({"phase":"upload","bytes":{"received":0,"total":total_bytes}}));
                }
                let input = CountedReader {
                    inner: input,
                    received: 0,
                    total: total_bytes,
                    operation: operation.clone(),
                };
                let result = self
                    .execute_reader(cmd, input, Duration::from_secs(1800))
                    .await?;
                drop(bundle);
                result
            } else {
                report(&format!("{name}：使用服务器 Git 认证克隆"), index, total);
                let cred = self.remote_git_credential(&url).await?;
                let host = url::Url::parse(&url)?.host_str().unwrap_or("").to_string();
                let (mut cmd, input) = crate::git_auth::remote_prefix(&host, cred.as_ref());
                cmd += &crate::remote_fs::guard(&self.environment.home, parent)?;
                cmd += &format!(
                    r#"dest="$path"/{}
if [ -e "$dest" ] || [ -L "$dest" ]; then printf CCBRIDGE_ALREADY_EXISTS >&2; exit 43; fi
git_auth clone --progress -- {} "$dest"
"#,
                    shell_quote(&name),
                    shell_quote(&url)
                );
                if let Some(o) = &operation {
                    o.patch(json!({"phase":"fetch"}));
                }
                let cb = operation.clone().map(|o| {
                    Arc::new(move |p: Value| o.patch(p)) as Arc<dyn Fn(Value) + Send + Sync>
                });
                self.execute_streaming(cmd, input.as_slice(), Duration::from_secs(600), cb)
                    .await?
            };
            if status != Some(0) {
                if let Some(o) = &operation {
                    o.patch(json!({"processed":index+1,"items":prepared}));
                }
                return Ok(
                    json!({"complete":false,"root":parent,"repositories":prepared,"error":crate::provision::failure(&String::from_utf8_lossy(&err)),"failed_repository":name}),
                );
            }
            let repo = self
                .rpc(json!({"op":"repository_inspect","path":dest}))
                .await?;
            prepared.push(json!({"name":name,"path":dest,"state":"cloned","repository":repo}));
            if let Some(o) = &operation {
                o.patch(json!({"processed":index+1,"items":prepared}));
            }
        }
        report("全部仓库处理完成", total, total);
        Ok(json!({"complete":true,"root":parent,"repositories":prepared}))
    }
    pub async fn git_access(
        &self,
        url: &str,
        cred: Option<crate::git_auth::Credential>,
    ) -> Result<Value> {
        if self.is_windows() {
            return self
                .rpc(json!({"op":"git_native_access","url":url,"credential":cred}))
                .await;
        }
        let host = crate::git_auth::host(url)?;
        let (mut cmd, input) = crate::git_auth::remote_prefix(&host, cred.as_ref());
        cmd += &format!("git_auth ls-remote --heads -- {}\n", shell_quote(url));
        let (out, err, status) = self
            .execute_for(cmd, &input, Duration::from_secs(120))
            .await?;
        if status != Some(0) {
            bail!(
                "远端 Codeup 访问失败：{}",
                crate::transfer::local_failure(&String::from_utf8_lossy(&err))
                    .replace("本机", "远端")
                    .replace("macOS 钥匙串", "钥匙串")
            )
        }
        Ok(
            json!({"accessible":true,"message":"远端 Git 已通过仓库读取检测","branches":String::from_utf8_lossy(&out).lines().count()}),
        )
    }
    async fn remote_git_credential(
        &self,
        url: &str,
    ) -> Result<Option<crate::git_auth::Credential>> {
        if crate::git_auth::host(url).is_err() {
            return Ok(None);
        }
        // A human-enabled VPS credential becomes the default for server Git. Do not shadow it
        // with an older App copy, and never retry under another identity after auth failure.
        if self.probe["runner_version"].as_str() == Some(env!("CARGO_PKG_VERSION"))
            || self.probe["capabilities"].as_array().is_some_and(|caps| {
                caps.iter()
                    .any(|cap| cap == "claude_info_v1" || cap == "vps_auth_v1")
            })
        {
            let status = self.rpc(json!({"op":"git_vps_auth_status"})).await?;
            if status["configured"] == true {
                return Ok(None);
            }
            if status["files_present"] == true {
                bail!(
                    "服务器 Codeup 凭据已失效或未接入 Git，请在 App 重新同步后检测；未改用另一份 App 凭据"
                )
            }
        }
        crate::git_auth::load("remote", &self.profile).await
    }
    pub async fn git_fetch(&self, path: &str) -> Result<Value> {
        self.git_fetch_remote(path, "origin").await
    }
    pub async fn git_fetch_remote(&self, path: &str, remote: &str) -> Result<Value> {
        self.git_fetch_remote_progress(path, remote, None).await
    }
    pub async fn git_fetch_remote_progress(
        &self,
        path: &str,
        remote: &str,
        progress: Option<Arc<dyn Fn(Value) + Send + Sync>>,
    ) -> Result<Value> {
        if remote.is_empty() || remote.starts_with('-') || remote.contains(['\n', '\r', '\0']) {
            bail!("远端名称无效")
        }
        if self.is_windows() {
            let target = self
                .rpc(json!({"op":"git_native_url","path":path,"remote":remote}))
                .await?;
            let credential = self
                .remote_git_credential(target["url"].as_str().context("仓库地址无效")?)
                .await?;
            return self.rpc_progress(json!({"op":"git_native_fetch","path":path,"remote":remote,"credential":credential}),progress).await;
        }
        self.rpc(json!({"op":"branches","path":path})).await?;
        let mut guard = crate::remote_fs::guard(&self.environment.home, path)?;
        guard += &format!(
            "git -C \"$path\" remote get-url -- {}\n",
            shell_quote(remote)
        );
        let (out, _, status) = self.execute(guard, &[]).await?;
        if status != Some(0) {
            bail!("没有可用的 origin 远端")
        }
        let url = String::from_utf8(out)?.trim().to_string();
        if let Ok(parsed) = url::Url::parse(&url) {
            if !parsed.username().is_empty() || parsed.password().is_some() {
                bail!("仓库 URL 含内嵌认证，请先改为不含凭据的地址")
            }
        }
        let host = url::Url::parse(&url)
            .ok()
            .and_then(|u| u.host_str().map(String::from))
            .unwrap_or_default();
        let cred = self.remote_git_credential(&url).await?;
        let (mut cmd, input) = crate::git_auth::remote_prefix(&host, cred.as_ref());
        cmd += &crate::remote_fs::guard(&self.environment.home, path)?;
        cmd += "export LC_ALL=C\n";
        cmd += &format!(
            "git_auth -C \"$path\" fetch --progress -- {}\n",
            shell_quote(remote)
        );
        let (_, err, status) = self
            .execute_streaming(cmd, input.as_slice(), Duration::from_secs(300), progress)
            .await?;
        if status != Some(0) {
            let raw = String::from_utf8_lossy(&err).to_ascii_lowercase();
            let code = if [
                "authentication",
                "username",
                "access denied",
                "403",
                "401",
                "repository not found",
                "terminal prompts disabled",
            ]
            .iter()
            .any(|s| raw.contains(s))
            {
                "auth"
            } else {
                "network"
            };
            return Err(coded(
                code,
                crate::transfer::local_failure(&raw).replace("本机", "远端"),
            ));
        }
        self.rpc(json!({"op":"branches","path":path})).await
    }
    pub async fn prepare_repositories(&self, group: &str, urls: &str) -> Result<Value> {
        let plan = crate::provision::plan(group, urls)?;
        if self.is_windows() {
            let projects = format!("{}\\projects", self.environment.home);
            if self.browse(&projects, false).await.is_err() {
                self.mkdir(&self.environment.home, "projects").await?;
            }
            let root = format!("{projects}\\{group}");
            if self.browse(&root, false).await.is_err() {
                self.mkdir(&projects, group).await?;
            }
            return self.clone_into(&root, urls, false).await;
        }
        let root = format!("{}/projects/{group}", self.environment.home);
        let mut prepared = vec![];
        for (name, url) in plan {
            let dest = format!("{root}/{name}");
            // A successful inspect establishes that an existing directory is the intended repo.
            if let Ok(repo) = self
                .rpc(json!({"op":"repository_inspect","path":dest}))
                .await
            {
                let id = crate::repository::identity(&url).unwrap_or_default();
                if !repo["identities"]
                    .as_array()
                    .map(|a| a.iter().any(|v| v.as_str() == Some(&id)))
                    .unwrap_or(false)
                {
                    return Ok(
                        json!({"complete":false,"root":root,"repositories":prepared,"error":"目标目录已有其他身份的 Git 仓库，未覆盖","failed_repository":name}),
                    );
                }
                prepared.push(
                    json!({"name":name,"path":dest,"state":"already_exists","repository":repo}),
                );
                continue;
            }
            let cmd = crate::provision::clone_script(&self.environment.home, group, &name, &url);
            let (_, err, status) = self.execute_for(cmd, &[], Duration::from_secs(600)).await?;
            if status != Some(0) {
                return Ok(
                    json!({"complete":false,"root":root,"repositories":prepared,"failed_repository":name,"error":crate::provision::failure(&String::from_utf8_lossy(&err))}),
                );
            }
            let repo = self
                .rpc(json!({"op":"repository_inspect","path":dest}))
                .await?;
            prepared.push(json!({"name":name,"path":dest,"state":"cloned","repository":repo}));
        }
        Ok(json!({"complete":true,"root":root,"repositories":prepared}))
    }
    pub async fn install(&mut self) -> Result<Value> {
        self.install_observed(None).await
    }
    pub async fn install_observed(
        &mut self,
        operation: Option<crate::operations::Operation>,
    ) -> Result<Value> {
        // Recheck the live executable before uploading: repeated clicks are a no-op.
        self.refresh().await;
        if self.probe["runner_version"].as_str() == Some(env!("CARGO_PKG_VERSION")) {
            return Ok(
                json!({"installed":false,"already_current":true,"path":self.runner_path,"probe":self.probe}),
            );
        }
        if let Some(remote) = self.probe["runner_version"].as_str() {
            let parse = |s: &str| {
                s.split('.')
                    .map(|x| x.parse::<u64>().unwrap_or(0))
                    .collect::<Vec<_>>()
            };
            if parse(remote) > parse(env!("CARGO_PKG_VERSION")) {
                bail!("远端 Runner 版本较新，不能自动降级；请更新 App")
            }
        }
        if self.probe["runner_version"].is_string() {
            let tasks = self
                .rpc(json!({"op":"list"}))
                .await
                .context("无法确认远端任务状态，暂不更新 Runner")?;
            let tasks = tasks["tasks"]
                .as_array()
                .context("远端任务状态无效，暂不更新 Runner")?;
            if tasks.iter().any(|t| {
                ["queued", "running", "waiting_approval"]
                    .contains(&t["state"].as_str().unwrap_or(""))
            }) {
                bail!("远端仍有任务运行或等待审批，请完成后再更新 Runner")
            }
        }
        if self.is_windows() {
            if !["Windows X64", "Windows AMD64"].contains(&self.environment.platform.as_str()) {
                bail!("尚未打包此 Windows 架构：{}", self.environment.platform);
            }
            let exe = std::env::current_exe()?;
            let resources = exe
                .parent()
                .and_then(|p| p.parent())
                .context("无法确定 App 资源目录")?
                .join("Resources");
            let artifact = std::env::var_os("CCBRIDGE_WINDOWS_RUNNER")
                .map(std::path::PathBuf::from)
                .unwrap_or_else(|| resources.join("ccbridge-windows-x86_64.exe"));
            let bytes = std::fs::read(&artifact)
                .with_context(|| format!("缺少 Windows Runner：{}", artifact.display()))?;
            if let Some(o) = &operation {
                o.patch(json!({"phase":"upload","name":"Windows Runner"}));
            }
            let command =
                crate::windows_bootstrap::install(&self.runner_path, env!("CARGO_PKG_VERSION"))?;
            let (_, err, status) = self
                .execute_for(command, &bytes, Duration::from_secs(180))
                .await?;
            if status != Some(0) {
                bail!("Windows Runner 安装失败：{}", String::from_utf8_lossy(&err));
            }
            if let Some(o) = &operation {
                o.patch(json!({"phase":"verify","name":"Windows Runner"}));
            }
            self.refresh().await;
            if self.probe["runner_version"].as_str() != Some(env!("CARGO_PKG_VERSION")) {
                bail!("Windows Runner 上传完成，但 RPC 校验失败；请重新检测");
            }
            return Ok(
                json!({"installed":true,"arch":self.environment.platform,"path":self.runner_path,"probe":self.probe}),
            );
        }
        let (out, err, code) = self.execute("uname -sm".into(), &[]).await?;
        if code != Some(0) {
            bail!("无法检测远端架构: {}", String::from_utf8_lossy(&err))
        }
        let arch = String::from_utf8_lossy(&out).trim().to_owned();
        let exe = std::env::current_exe()?;
        let resources = exe
            .parent()
            .and_then(|p| p.parent())
            .ok_or_else(|| anyhow::anyhow!("无法确定 App 安装路径"))?
            .join("Resources");
        let artifact = match arch.as_str() {
            "Linux x86_64" => resources.join("ccbridge-linux-x86_64"),
            "Linux aarch64" => resources.join("ccbridge-linux-aarch64"),
            "Darwin arm64" => exe,
            _ => bail!("尚未打包此架构：{arch}；请在远端从源码构建 headless Runner"),
        };
        let bytes = std::fs::read(&artifact)
            .with_context(|| format!("缺少对应 Runner：{}", artifact.display()))?;
        let dest = std::path::Path::new(&self.runner_path);
        let parent = dest
            .parent()
            .ok_or_else(|| anyhow::anyhow!("Runner 路径缺少父目录"))?
            .to_string_lossy();
        let tmp = format!("{}.{}.upload", self.runner_path, uuid::Uuid::new_v4());
        let command = format!(
            "umask 077; mkdir -p -- {} && cat > {} && chmod 700 {} && (test ! -e {} || cp -p -- {} {}) && mv -f -- {} {}",
            shell_quote(&parent),
            shell_quote(&tmp),
            shell_quote(&tmp),
            shell_quote(&self.runner_path),
            shell_quote(&self.runner_path),
            shell_quote(&format!("{}.previous", self.runner_path)),
            shell_quote(&tmp),
            shell_quote(&self.runner_path)
        );
        if let Some(o) = &operation {
            o.patch(json!({"phase":"upload","name":"Runner"}));
        }
        let (_, err, code) = self.execute(command, &bytes).await?;
        if code != Some(0) {
            bail!("Runner 安装失败: {}", String::from_utf8_lossy(&err))
        }
        if let Some(o) = &operation {
            o.patch(json!({"phase":"verify","name":"Runner"}));
        }
        self.refresh().await;
        if self.probe["runner_version"].as_str() != Some(env!("CARGO_PKG_VERSION")) {
            bail!("Runner已上传但版本校验未通过，请重新检测")
        }
        Ok(json!({"installed":true,"arch":arch,"path":self.runner_path,"probe":self.probe}))
    }
    async fn rpc_progress(
        &self,
        v: Value,
        progress: Option<Arc<dyn Fn(Value) + Send + Sync>>,
    ) -> Result<Value> {
        let bytes = serde_json::to_vec(&v)?;
        let (out, err, exit) = self
            .execute_streaming(
                self.rpc_command()?,
                bytes.as_slice(),
                Duration::from_secs(900),
                progress,
            )
            .await?;
        if exit != Some(0) {
            bail!("Windows Runner 请求失败：{}", String::from_utf8_lossy(&err));
        }
        unwrap(serde_json::from_slice(&out).context("Windows Runner 返回格式无效")?)
    }
    pub async fn rpc(&self, v: Value) -> Result<Value> {
        let bytes = serde_json::to_vec(&v)?;
        let (stdout, stderr, exit) = self.execute(self.rpc_command()?, &bytes).await?;
        if exit != Some(0) {
            bail!(
                "远端 Runner 调用失败 exit={exit:?}: {}",
                String::from_utf8_lossy(&stderr)
            )
        }
        unwrap(
            serde_json::from_slice(&stdout)
                .context("远端不是有效 Runner JSON 响应；检查路径和 shell 启动输出")?,
        )
    }
}

struct CountedReader<R> {
    inner: R,
    received: u64,
    total: u64,
    operation: Option<crate::operations::Operation>,
}
impl<R: tokio::io::AsyncRead + Unpin> tokio::io::AsyncRead for CountedReader<R> {
    fn poll_read(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        buf: &mut tokio::io::ReadBuf<'_>,
    ) -> std::task::Poll<std::io::Result<()>> {
        let before = buf.filled().len();
        let r = std::pin::Pin::new(&mut self.inner).poll_read(cx, buf);
        if let std::task::Poll::Ready(Ok(())) = &r {
            self.received += (buf.filled().len() - before) as u64;
            if let Some(o) = &self.operation {
                o.patch(json!({"phase":if self.received>=self.total{"import"}else{"upload"},"bytes":{"received":self.received,"total":self.total}}));
            }
        }
        r
    }
}

#[cfg(test)]
mod security_tests {
    use super::*;
    use russh::keys::{Algorithm, PrivateKey, ssh_key::rand_core::UnwrapErr};
    use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};

    fn string(out: &mut Vec<u8>, bytes: &[u8]) {
        out.extend_from_slice(&(bytes.len() as u32).to_be_bytes());
        out.extend_from_slice(bytes);
    }
    fn packet(payload: &[u8]) -> Vec<u8> {
        let mut padding = 8 - (payload.len() + 5) % 8;
        if padding < 4 {
            padding += 8;
        }
        let mut out = ((payload.len() + padding + 1) as u32)
            .to_be_bytes()
            .to_vec();
        out.push(padding as u8);
        out.extend_from_slice(payload);
        out.resize(out.len() + padding, 0);
        out
    }
    #[tokio::test]
    async fn malformed_x25519_is_an_error_not_a_panicked_client_task() {
        for length in [0, 31, 32, 33] {
            let key =
                PrivateKey::random(&mut UnwrapErr(getrandom::SysRng), Algorithm::Ed25519).unwrap();
            let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
            let address = listener.local_addr().unwrap();
            let server = tokio::spawn(async move {
                let (stream, _) = listener.accept().await.unwrap();
                let mut stream = BufReader::new(stream);
                let mut banner = String::new();
                stream.read_line(&mut banner).await.unwrap();
                stream
                    .get_mut()
                    .write_all(b"SSH-2.0-CCBridge-security-fixture\r\n")
                    .await
                    .unwrap();
                let mut kex = vec![20];
                kex.extend_from_slice(&[0; 16]);
                for value in [
                    "curve25519-sha256",
                    "ssh-ed25519",
                    "aes128-ctr",
                    "aes128-ctr",
                    "hmac-sha2-256",
                    "hmac-sha2-256",
                    "none",
                    "none",
                    "",
                    "",
                ] {
                    string(&mut kex, value.as_bytes());
                }
                kex.extend_from_slice(&[0; 5]);
                stream.get_mut().write_all(&packet(&kex)).await.unwrap();
                let mut reply = vec![31];
                string(&mut reply, &key.public_key().to_bytes().unwrap());
                // Includes a 32-byte zero point, which also must be rejected.
                string(&mut reply, &vec![0; length]);
                string(&mut reply, &[]);
                stream.get_mut().write_all(&packet(&reply)).await.unwrap();
                let _ = tokio::io::copy(&mut stream, &mut tokio::io::sink()).await;
            });
            let attempt = tokio::spawn(client::connect(
                Arc::new(client::Config::default()),
                address,
                HostCheck {
                    expected: String::new(),
                    seen: Arc::new(Mutex::new(String::new())),
                },
            ));
            let result = tokio::time::timeout(Duration::from_secs(5), attempt)
                .await
                .expect("bounded SSH handshake")
                .expect("client task must not panic");
            assert!(
                matches!(result, Err(russh::Error::Kex)),
                "malformed or zero-point X25519 should return Kex, length={length}"
            );
            server.abort();
            let _ = server.await;
        }
    }
}
