use crate::common::*;
use anyhow::{Context, Result, bail};
use russh::{
    ChannelMsg, client,
    keys::{HashAlg, ssh_key},
};
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
        key: &ssh_key::PublicKey,
    ) -> std::result::Result<bool, Self::Error> {
        let fp = key.fingerprint(HashAlg::Sha256).to_string();
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
}
#[derive(Clone)]
pub struct Connection {
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
            if !fp.is_empty() {
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
        handle: Arc::new(handle),
        profile: p,
        probe: Value::Null,
        environment: RemoteEnvironment {
            home: String::new(),
            user: String::new(),
            claude: String::new(),
            platform: String::new(),
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
    if status != Some(0) {
        bail!("获取远端账号 HOME 失败：{}", String::from_utf8_lossy(&err))
    }
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
    conn.environment = RemoteEnvironment {
        home: values[0].clone(),
        user: values[1].clone(),
        claude: values[2].clone(),
        platform: values[3].clone(),
    };
    conn.runner_path = if conn.profile.runner.is_empty() {
        format!("{}/.local/bin/ccbridge", conn.environment.home)
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
        json!({"app_version":env!("CARGO_PKG_VERSION"),"bridge_protocol":1,"bridge_capabilities":["client_activity_lease","update_drain","models_v1"],"connected":!self.handle.is_closed(),"profile":self.profile,"probe":self.probe,"environment":self.environment,"resolved_paths":{"runner":self.runner_path,"claude":self.claude_path,"state":format!("{}/.local/state/ccbridge",self.environment.home)}})
    }
    pub fn permits_workspace(&self, path: &str) -> bool {
        (!self.profile.workspace.is_empty() && path == self.profile.workspace)
            || std::path::Path::new(path).starts_with(&self.environment.home)
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
        tokio::time::timeout(duration, async {
            let mut channel = self.handle.channel_open_session().await?;
            channel.exec(true, command).await?;
            channel.data(input).await?;
            channel.eof().await?;
            let mut stdout = vec![];
            let mut stderr = vec![];
            let mut exit = None;
            while let Some(msg) = channel.wait().await {
                match msg {
                    ChannelMsg::Data { data } => stdout.extend_from_slice(&data),
                    ChannelMsg::ExtendedData { data, .. } => stderr.extend_from_slice(&data),
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
            if let Ok(repo) = self
                .rpc(json!({"op":"repository_inspect","path":dest}))
                .await
            {
                let id = crate::repository::identity(&url).unwrap_or_default();
                if repo["path"].as_str() != Some(&dest)
                    || !repo["identities"]
                        .as_array()
                        .map(|a| a.iter().any(|v| v.as_str() == Some(&id)))
                        .unwrap_or(false)
                {
                    bail!("同名目录已存在，但不是指定仓库，未覆盖")
                }
                prepared.push(
                    json!({"name":name,"path":dest,"state":"already_exists","repository":repo}),
                );
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
                let bundle = tokio::task::spawn_blocking(move || {
                    crate::transfer::bundle_with_auth(&u, cred.as_ref())
                })
                .await??;
                report(&format!("{name}：通过 SSH 传输并导入仓库"), index, total);
                let cmd =
                    crate::transfer::import_script(&self.environment.home, parent, &name, &url)?;
                let input = tokio::fs::File::open(&bundle.path).await?;
                let result = self
                    .execute_reader(cmd, input, Duration::from_secs(1800))
                    .await?;
                drop(bundle);
                result
            } else {
                report(&format!("{name}：使用服务器 Git 认证克隆"), index, total);
                let cred = if crate::git_auth::host(&url).is_ok() {
                    crate::git_auth::load("remote", &self.profile).await?
                } else {
                    None
                };
                let host = url::Url::parse(&url)?.host_str().unwrap_or("").to_string();
                let (mut cmd, input) = crate::git_auth::remote_prefix(&host, cred.as_ref());
                cmd += &crate::remote_fs::guard(&self.environment.home, parent)?;
                cmd += &format!(
                    r#"dest="$path"/{}
if [ -e "$dest" ] || [ -L "$dest" ]; then printf CCBRIDGE_ALREADY_EXISTS >&2; exit 43; fi
git_auth clone --quiet -- {} "$dest"
"#,
                    shell_quote(&name),
                    shell_quote(&url)
                );
                self.execute_for(cmd, &input, Duration::from_secs(600))
                    .await?
            };
            if status != Some(0) {
                return Ok(
                    json!({"complete":false,"root":parent,"repositories":prepared,"error":crate::provision::failure(&String::from_utf8_lossy(&err)),"failed_repository":name}),
                );
            }
            let repo = self
                .rpc(json!({"op":"repository_inspect","path":dest}))
                .await?;
            prepared.push(json!({"name":name,"path":dest,"state":"cloned","repository":repo}));
        }
        report("全部仓库处理完成", total, total);
        Ok(json!({"complete":true,"root":parent,"repositories":prepared}))
    }
    pub async fn git_access(
        &self,
        url: &str,
        cred: Option<crate::git_auth::Credential>,
    ) -> Result<Value> {
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
    pub async fn git_fetch(&self, path: &str) -> Result<Value> {
        self.git_fetch_remote(path, "origin").await
    }
    pub async fn git_fetch_remote(&self, path: &str, remote: &str) -> Result<Value> {
        if remote.is_empty() || remote.starts_with('-') || remote.contains(['\n', '\r', '\0']) {
            bail!("远端名称无效")
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
        let cred = if crate::git_auth::host(&url).is_ok() {
            crate::git_auth::load("remote", &self.profile).await?
        } else {
            None
        };
        let (mut cmd, input) = crate::git_auth::remote_prefix(&host, cred.as_ref());
        cmd += &crate::remote_fs::guard(&self.environment.home, path)?;
        cmd += &format!("git_auth -C \"$path\" fetch -- {}\n", shell_quote(remote));
        let (_, err, status) = self
            .execute_for(cmd, &input, Duration::from_secs(300))
            .await?;
        if status != Some(0) {
            bail!(
                "获取远端分支失败：{}",
                crate::transfer::local_failure(&String::from_utf8_lossy(&err))
                    .replace("本机", "远端")
                    + "；请在 Codeup 认证选择‘服务器当前 SSH 账号’，或同步认证到 VPS"
            )
        }
        self.rpc(json!({"op":"branches","path":path})).await
    }
    pub async fn prepare_repositories(&self, group: &str, urls: &str) -> Result<Value> {
        let plan = crate::provision::plan(group, urls)?;
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
        let (_, err, code) = self.execute(command, &bytes).await?;
        if code != Some(0) {
            bail!("Runner 安装失败: {}", String::from_utf8_lossy(&err))
        }
        self.refresh().await;
        Ok(json!({"installed":true,"arch":arch,"path":self.runner_path,"probe":self.probe}))
    }
    pub async fn rpc(&self, v: Value) -> Result<Value> {
        let bytes = serde_json::to_vec(&v)?;
        let (stdout, stderr, exit) = self
            .execute(
                format!(
                    "env CCBRIDGE_RUNNER_HOME={} {} rpc",
                    shell_quote(&format!("{}/.local/state/ccbridge", self.environment.home)),
                    shell_quote(&self.runner_path)
                ),
                &bytes,
            )
            .await?;
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
