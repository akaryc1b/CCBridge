use crate::{backend, common::*, ssh};
use anyhow::{Context, Result};
use russh::{
    Channel, ChannelId,
    keys::{Algorithm, HashAlg, PrivateKey, ssh_key::rand_core::UnwrapErr},
    server::{self, Msg, Server as _, Session},
};
use serde_json::{Value, json};
use std::{collections::HashMap, sync::Arc, time::Duration};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};

#[derive(Clone)]
struct TestServer {
    bin: String,
    data: HashMap<ChannelId, Vec<u8>>,
    commands: HashMap<ChannelId, String>,
    home: String,
    claude: String,
}
impl server::Server for TestServer {
    type Handler = Self;
    fn new_client(&mut self, _: Option<std::net::SocketAddr>) -> Self {
        self.clone()
    }
}
impl server::Handler for TestServer {
    type Error = russh::Error;
    async fn auth_password(&mut self, u: &str, p: &str) -> Result<server::Auth, Self::Error> {
        Ok(if u == "fixture" && p == "test-only-password" {
            server::Auth::Accept
        } else {
            server::Auth::Reject {
                proceed_with_methods: None,
                partial_success: false,
            }
        })
    }
    async fn channel_open_session(
        &mut self,
        ch: Channel<Msg>,
        reply: server::ChannelOpenHandle,
        _: &mut Session,
    ) -> Result<(), Self::Error> {
        self.data.insert(ch.id(), vec![]);
        reply.accept().await;
        Ok(())
    }
    async fn exec_request(
        &mut self,
        id: ChannelId,
        data: &[u8],
        s: &mut Session,
    ) -> Result<(), Self::Error> {
        let command = String::from_utf8_lossy(data).to_string();
        let rpc = format!(
            "env CCBRIDGE_RUNNER_HOME={} {} rpc",
            shell_quote(&format!("{}/.local/state/ccbridge", self.home)),
            shell_quote(&self.bin)
        );
        if command.starts_with("# ccbridge:discover\n")
            || command.starts_with("# ccbridge:filesystem\n")
            || command == rpc
        {
            self.commands.insert(id, command);
            s.channel_success(id)?;
        } else {
            s.channel_failure(id)?;
        }
        Ok(())
    }
    async fn data(
        &mut self,
        id: ChannelId,
        data: &[u8],
        _: &mut Session,
    ) -> Result<(), Self::Error> {
        self.data.get_mut(&id).unwrap().extend_from_slice(data);
        Ok(())
    }
    async fn channel_eof(&mut self, id: ChannelId, s: &mut Session) -> Result<(), Self::Error> {
        let data = self.data.remove(&id).unwrap_or_default();
        let bin = self.bin.clone();
        let command = self.commands.remove(&id).unwrap_or_default();
        let home = self.home.clone();
        let claude = self.claude.clone();
        let h = s.handle();
        tokio::spawn(async move {
            use std::process::Stdio;
            use tokio::process::Command;
            if command.starts_with("# ccbridge:discover\n") {
                h.data(
                    id,
                    format!("{home}\0fixture\0{claude}\0Darwin arm64\0").into_bytes(),
                )
                .await
                .unwrap();
                h.exit_status_request(id, 0).await.unwrap();
                h.eof(id).await.unwrap();
                h.close(id).await.unwrap();
                return;
            }
            if command.contains("slow-directory") {
                tokio::time::sleep(Duration::from_millis(800)).await;
            }
            let mut cmd = if command.starts_with("# ccbridge:filesystem\n") {
                let mut c = Command::new("sh");
                c.arg("-c").arg(&command);
                c
            } else {
                let mut c = Command::new(bin);
                c.arg("rpc");
                c
            };
            let mut c = cmd
                .env("HOME", &home)
                .env(
                    "CCBRIDGE_RUNNER_HOME",
                    format!("{home}/.local/state/ccbridge"),
                )
                .stdin(Stdio::piped())
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .spawn()
                .unwrap();
            c.stdin.take().unwrap().write_all(&data).await.unwrap();
            let out = c.wait_with_output().await.unwrap();
            h.data(id, out.stdout).await.unwrap();
            if !out.stderr.is_empty() {
                h.extended_data(id, 1, out.stderr).await.unwrap();
            }
            h.exit_status_request(id, out.status.code().unwrap_or(1) as u32)
                .await
                .unwrap();
            h.eof(id).await.unwrap();
            h.close(id).await.unwrap();
        });
        Ok(())
    }
}
async fn poll(c: &ssh::Connection, id: &str, state: &str) -> Result<Value> {
    for _ in 0..150 {
        let v = c.rpc(json!({"op":"status","task_id":id})).await?;
        if v["state"] == state {
            return Ok(v);
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    anyhow::bail!("Timed out waiting for {state}")
}
fn create_git(path: &std::path::Path, url: &str) -> Result<()> {
    private_dir(path)?;
    for args in [
        vec!["init", "-b", "main"],
        vec!["remote", "add", "origin", url],
    ] {
        assert!(
            std::process::Command::new("git")
                .arg("-C")
                .arg(path)
                .args(args)
                .output()?
                .status
                .success()
        );
    }
    std::fs::write(path.join("README.md"), "remote-fixture-code")?;
    assert!(
        std::process::Command::new("git")
            .arg("-C")
            .arg(path)
            .args(["add", "README.md"])
            .output()?
            .status
            .success()
    );
    assert!(
        std::process::Command::new("git")
            .arg("-C")
            .arg(path)
            .args([
                "-c",
                "user.name=Fixture",
                "-c",
                "user.email=fixture@example.invalid",
                "commit",
                "-m",
                "fixture"
            ])
            .output()?
            .status
            .success()
    );
    Ok(())
}
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn encrypted_ssh_runner_and_mcp_acceptance() -> Result<()> {
    let bin = std::fs::canonicalize(
        std::env::var("CCBRIDGE_TEST_BIN").unwrap_or_else(|_| "target/release/ccbridge".into()),
    )
    .context("Run cargo build --release before cargo test")?
    .to_string_lossy()
    .to_string();
    let root = std::path::PathBuf::from("/tmp").join(format!("cb-test-{}", uuid::Uuid::new_v4()));
    private_dir(&root)?;
    let root = root.canonicalize()?;
    let remote_home = root.join("account-special-home");
    private_dir(&remote_home)?;
    unsafe {
        std::env::set_var("CCBRIDGE_HOME", root.join("app"));
        std::env::set_var("CCBRIDGE_RUNNER_HOME", root.join("runner"));
    }
    let workspace = root.join("code");
    private_dir(&workspace)?;
    std::fs::write(workspace.join("README.md"), "remote-fixture-code")?;
    let fake = std::fs::canonicalize("tests/fake_claude.py")?;
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(&fake, std::fs::Permissions::from_mode(0o700))?;
    let key = PrivateKey::random(&mut UnwrapErr(getrandom::SysRng), Algorithm::Ed25519)?;
    let fp = key.public_key().fingerprint(HashAlg::Sha256).to_string();
    let config = Arc::new(server::Config {
        keys: vec![key],
        auth_rejection_time: Duration::from_millis(10),
        auth_rejection_time_initial: Some(Duration::from_millis(0)),
        ..Default::default()
    });
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await?;
    let port = listener.local_addr()?.port();
    let fixture_home = remote_home.to_string_lossy().to_string();
    let fixture_claude = fake.to_string_lossy().to_string();
    tokio::spawn(async move {
        TestServer {
            bin: bin.clone(),
            data: HashMap::new(),
            commands: HashMap::new(),
            home: fixture_home,
            claude: fixture_claude,
        }
        .run_on_socket(config, &listener)
        .await
        .unwrap();
    });
    let bin = std::fs::canonicalize(
        std::env::var("CCBRIDGE_TEST_BIN").unwrap_or_else(|_| "target/release/ccbridge".into()),
    )?
    .to_string_lossy()
    .to_string();
    let mut p = Profile {
        host: "127.0.0.1".into(),
        port,
        user: "fixture".into(),
        runner: bin.clone(),
        workspace: workspace.to_string_lossy().into(),
        claude: fake.to_string_lossy().into(),
        ..Default::default()
    };
    assert!(
        ssh::connect(p.clone(), "test-only-password".into())
            .await
            .err()
            .unwrap()
            .to_string()
            .starts_with("HOST_KEY:")
    );
    p.fingerprint = "SHA256:previous-server-key".into();
    assert!(
        ssh::connect(p.clone(), "test-only-password".into())
            .await
            .err()
            .unwrap()
            .to_string()
            .starts_with("HOST_KEY:")
    );
    p.fingerprint = fp;
    assert!(ssh::connect(p.clone(), "wrong".into()).await.is_err());
    let c = ssh::connect(p.clone(), "test-only-password".into()).await?;
    assert_eq!(c.probe["code_access"], "remote_directory");
    assert_eq!(c.environment.home, remote_home.to_string_lossy());
    assert_eq!(
        c.probe["state_directory"],
        format!("{}/.local/state/ccbridge", remote_home.display())
    );
    let mut automatic_paths = p.clone();
    automatic_paths.runner.clear();
    automatic_paths.claude.clear();
    automatic_paths.workspace.clear();
    let auto_conn = ssh::connect(automatic_paths, "test-only-password".into()).await?;
    assert_eq!(
        auto_conn.runner_path,
        format!("{}/.local/bin/ccbridge", remote_home.display())
    );
    assert_eq!(auto_conn.claude_path, fake.to_string_lossy());
    drop(auto_conn);

    let start = |prompt: &str, mode: &str| json!({"op":"start","task_id":uuid::Uuid::new_v4().to_string(),"prompt":prompt,"mode":mode,"workspace":p.workspace,"claude":p.claude});
    // SSH encrypted transport + actual Runner + actual MCP approval server; model is a fixture.
    let req = start("APPROVAL", "approval");
    let t = c.rpc(req.clone()).await?;
    let id = field(&t, "task_id")?;
    let waiting = poll(&c, id, "waiting_approval").await?;
    assert!(!workspace.join("result.txt").exists());
    let rid = waiting["approvals"][0]["request_id"].clone();
    c.rpc(json!({"op":"approve","task_id":id,"request_id":rid,"allow":true}))
        .await?;
    let done = poll(&c, id, "completed").await?;
    assert!(
        done["result"]["result"]
            .as_str()
            .unwrap()
            .contains("allowed=True")
    );
    assert_eq!(
        std::fs::read_to_string(workspace.join("result.txt"))?,
        "approved"
    );
    assert_eq!(c.rpc(req).await?["task_id"], id); // idempotent replay
    let t = c.rpc(start("APPROVAL", "approval")).await?;
    let id = field(&t, "task_id")?;
    let waiting = poll(&c, id, "waiting_approval").await?;
    c.rpc(json!({"op":"approve","task_id":id,"request_id":waiting["approvals"][0]["request_id"],"allow":false})).await?;
    assert!(
        poll(&c, id, "completed").await?["result"]["result"]
            .as_str()
            .unwrap()
            .contains("allowed=False")
    );
    let t = c.rpc(start("WRITE", "automatic")).await?;
    poll(&c, field(&t, "task_id")?, "completed").await?;
    assert_eq!(
        std::fs::read_to_string(workspace.join("result.txt"))?,
        "automatic"
    );
    let t = c.rpc(start("FAIL", "approval")).await?;
    poll(&c, field(&t, "task_id")?, "failed").await?;
    let t = c.rpc(start("SLOW", "approval")).await?;
    let id = field(&t, "task_id")?;
    poll(&c, id, "running").await?;
    assert!(c.rpc(start("collision", "approval")).await.is_err());
    drop(c);
    let c = ssh::connect(p.clone(), "test-only-password".into()).await?;
    poll(&c, id, "running").await?;
    c.rpc(json!({"op":"cancel","task_id":id})).await?;
    poll(&c, id, "cancelled").await?;
    // Main model comes from init, not requested alias or aggregated child usage.
    let mut model_request = start("MODEL", "automatic");
    model_request["model"] = json!("fixture-model-one");
    let model_turn = c.rpc(model_request.clone()).await?;
    let model_done = poll(&c, field(&model_turn, "task_id")?, "completed").await?;
    assert_eq!(model_done["model_requested"], "fixture-model-one");
    assert_eq!(model_done["model_resolved"], "fixture-model-one");
    model_request["model"] = json!("fixture-model-two");
    assert!(c.rpc(model_request).await.is_err());
    let mismatch = c.rpc(start("SESSION_MISMATCH", "automatic")).await?;
    let mismatch = poll(&c, field(&mismatch, "task_id")?, "failed").await?;
    assert!(mismatch["observed_session_id"].is_string());
    assert_ne!(mismatch["session_id"], mismatch["observed_session_id"]);
    // Broker enforces human-only approvals and automatic-mode grant.

    // Find repositories by Git remote identity, not the folder name; exclude other HOME/symlinks.
    let local_repo = root.join("mac-local/same-name");
    let remote_repo = remote_home.join("projects/completely-different-name");
    let wrong_repo = remote_home.join("projects/same-name");
    create_git(
        &local_repo,
        "https://fake-user:fake-token@fixture.invalid/team/service.git?token=do-not-expose",
    )?;
    create_git(&remote_repo, "git@fixture.invalid:team/service.git")?;
    create_git(&wrong_repo, "https://fixture.invalid/other/repo.git")?;
    assert!(
        std::process::Command::new("git")
            .arg("-C")
            .arg(&wrong_repo)
            .args([
                "remote",
                "add",
                "upstream",
                "https://fixture.invalid/team/service.git"
            ])
            .output()?
            .status
            .success()
    );
    let outside = root.join("other-account/same-name");
    create_git(&outside, "https://fixture.invalid/team/service.git")?;
    std::os::unix::fs::symlink(&outside, remote_home.join("symlink-outside"))?;
    let matched = c.match_repository(local_repo.to_str().unwrap()).await?;
    assert_eq!(matched["state"], "matched");
    assert_eq!(
        matched["selected_path"],
        remote_repo.to_string_lossy().as_ref()
    );
    assert_eq!(matched["candidates"].as_array().unwrap().len(), 1);
    assert!(!matched.to_string().contains("fake-token"));
    let mut bounded = p.clone();
    bounded.search_root = outside.to_string_lossy().into();
    let bounded = ssh::connect(bounded, "test-only-password".into()).await?;
    assert!(
        bounded
            .match_repository(local_repo.to_str().unwrap())
            .await
            .is_err()
    );
    drop(bounded);
    let broken = remote_home.join("broken-repo");
    private_dir(&broken.join(".git"))?;
    let partial = c.match_repository(local_repo.to_str().unwrap()).await?;
    assert_eq!(partial["state"], "selection_required");
    assert_eq!(partial["scan"]["failed_repositories"], 1);
    std::fs::remove_dir_all(broken)?;
    let duplicate = remote_home.join("projects/second-worktree");
    assert!(
        std::process::Command::new("git")
            .arg("-C")
            .arg(&remote_repo)
            .args(["worktree", "add", "-b", "sibling"])
            .arg(&duplicate)
            .output()?
            .status
            .success()
    );
    assert_eq!(
        c.match_repository(local_repo.to_str().unwrap()).await?["state"],
        "selection_required"
    );
    let mut c = c;
    let unchanged = c.install().await?;
    assert_eq!(unchanged["already_current"], true);
    assert_eq!(unchanged["installed"], false);
    let repeated = c.install().await?;
    assert_eq!(repeated["already_current"], true);
    let shared = Arc::new(backend::BackendState::new(Some(c)));
    let slow_path = remote_home.join("slow-directory");
    private_dir(&slow_path)?;
    let slow_state = shared.clone();
    let slow = tokio::spawn(async move {
        backend::dispatch(
            slow_state,
            json!({"op":"directory_list","path":slow_path}),
            true,
        )
        .await
    });
    tokio::time::sleep(Duration::from_millis(80)).await;
    let status = tokio::time::timeout(
        Duration::from_millis(250),
        backend::dispatch(shared.clone(), json!({"op":"connection_status"}), false),
    )
    .await??;
    assert_eq!(status["connected"], true);
    slow.await??;

    let match_request = json!({"op":"repository_match","local_repository":local_repo});
    assert_eq!(
        backend::dispatch(shared.clone(), match_request, false).await?["state"],
        "selection_required"
    );
    assert!(
        backend::dispatch(
            shared.clone(),
            json!({"op":"bind_repository","local_repository":local_repo,"remote_path":remote_repo}),
            false
        )
        .await
        .is_err()
    );
    backend::dispatch(
        shared.clone(),
        json!({"op":"bind_repository","local_repository":local_repo,"remote_path":remote_repo}),
        true,
    )
    .await?;
    assert_eq!(
        backend::dispatch(
            shared.clone(),
            json!({"op":"repository_match","local_repository":local_repo}),
            false
        )
        .await?["selected_path"],
        remote_repo.to_string_lossy().as_ref()
    );
    let absent = root.join("mac-local/absent");
    create_git(&absent, "https://fixture.invalid/no/matching.git")?;
    assert_eq!(
        backend::dispatch(
            shared.clone(),
            json!({"op":"repository_match","local_repository":absent}),
            false
        )
        .await?["state"],
        "not_found"
    );
    println!(
        "PASS account HOME auto-detection / per-account state / remote Git identity / credential redaction / no folder-name matching / no other-account or symlink search / ambiguity / human binding / missing repository"
    );

    let listing = backend::dispatch(
        shared.clone(),
        json!({"op":"directory_list","path":remote_home}),
        false,
    )
    .await?;
    assert!(
        listing["entries"]
            .as_array()
            .unwrap()
            .iter()
            .any(|e| e["name"] == "projects")
    );
    assert!(
        backend::dispatch(
            shared.clone(),
            json!({"op":"select_remote_workspace","path":remote_repo}),
            false
        )
        .await
        .is_err()
    );
    backend::dispatch(
        shared.clone(),
        json!({"op":"select_remote_workspace","path":remote_repo}),
        true,
    )
    .await?;
    let opened = backend::dispatch(
        shared.clone(),
        json!({"op":"start","task_id":uuid::Uuid::new_v4().to_string(),"prompt":"Read README.md","mode":"approval"}),
        false,
    )
    .await?;
    assert_eq!(
        opened["workspace"],
        remote_repo.to_string_lossy().to_string()
    );
    {
        let mut guard = shared.connection.lock().await;
        let conn = guard.as_mut().unwrap();
        poll(conn, field(&opened, "task_id")?, "completed").await?;
        conn.profile.active_remote_workspace = String::new();
    }
    let project_b = remote_home.join("project-b-routing");
    std::fs::create_dir_all(&project_b)?;
    std::fs::write(project_b.join("README.md"), "remote-fixture-code")?;
    assert!(
        backend::dispatch(
            shared.clone(),
            json!({"op":"db_project_save","workspace":project_b,"name":"项目B"}),
            false
        )
        .await
        .is_err()
    );
    for (name, path) in [
        ("项目A", remote_repo.as_path()),
        ("项目B", project_b.as_path()),
    ] {
        backend::dispatch(
            shared.clone(),
            json!({"op":"db_project_save","workspace":path,"name":name}),
            true,
        )
        .await?;
    }
    let projects = backend::dispatch(shared.clone(), json!({"op":"db_projects"}), false).await?;
    assert_eq!(projects["projects"].as_array().unwrap().len(), 2);
    let db_path = remote_home.join(".local/state/ccbridge/databases.json");
    let before = std::fs::read(&db_path)?;
    backend::dispatch(
        shared.clone(),
        json!({"op":"project_save","workspace":project_b,"name":"项目B"}),
        true,
    )
    .await?;
    let independent =
        backend::dispatch(shared.clone(), json!({"op":"project_list"}), false).await?;
    assert_eq!(independent["projects"].as_array().unwrap().len(), 2);
    assert!(
        independent["projects"]
            .as_array()
            .unwrap()
            .iter()
            .all(|p| p["databases"].is_array())
    );
    assert_eq!(std::fs::read(&db_path)?, before);

    {
        shared
            .connection
            .lock()
            .await
            .as_mut()
            .unwrap()
            .profile
            .active_remote_workspace = remote_repo.to_string_lossy().into();
    }
    let routed=backend::dispatch(shared.clone(),json!({"op":"start","task_id":uuid::Uuid::new_v4().to_string(),"remote_project":"项目B","prompt":"Read README.md","mode":"approval"}),false).await?;
    assert_eq!(routed["workspace"], project_b.to_string_lossy().to_string());
    {
        let mut guard = shared.connection.lock().await;
        let c = guard.as_mut().unwrap();
        poll(c, field(&routed, "task_id")?, "completed").await?;
        assert_eq!(
            c.profile.active_remote_workspace,
            remote_repo.to_string_lossy()
        );
        c.profile.active_remote_workspace = String::new();
    }
    assert!(backend::dispatch(shared.clone(),json!({"op":"start","task_id":uuid::Uuid::new_v4().to_string(),"remote_project":"不存在的项目","prompt":"x","mode":"approval"}),false).await.is_err());
    println!(
        "PASS multiple registered projects / human-only registration / readonly project list / explicit project routing independent of App selection / unknown project rejected"
    );
    println!(
        "PASS SSH filesystem list / human-only remote selection / start without local repository / original local matching preserved"
    );

    #[cfg(target_os = "macos")]
    {
        let mut saved = p.clone();
        saved.remember_password = true;
        saved.credential_store = "file".into();
        assert!(
            backend::dispatch(
                shared.clone(),
                json!({"op":"save_profile","profile":saved,"password":"test-only-password"}),
                true
            )
            .await
            .is_err()
        );
        backend::dispatch(
            shared.clone(),
            json!({"op":"save_profile","profile":saved,"password":"test-only-password","storage_ack":true}),
            true,
        )
        .await?;
        assert_eq!(
            backend::dispatch(
                shared.clone(),
                json!({"op":"load_password","profile":saved}),
                true
            )
            .await?["password"],
            "test-only-password"
        );
        assert!(
            !std::fs::read_to_string(app_dir()?.join("profile.json"))?
                .contains("test-only-password")
        );
        backend::dispatch(
            shared.clone(),
            json!({"op":"forget_password","profile":saved}),
            true,
        )
        .await?;
        assert!(
            backend::dispatch(
                shared.clone(),
                json!({"op":"load_password","profile":saved}),
                true
            )
            .await
            .is_err()
        );
        println!(
            "PASS explicit profile save / one-time local-storage consent / private file store-read-delete / no password in profile"
        );
    }
    assert!(
        backend::dispatch(shared.clone(), start("no", "automatic"), false)
            .await
            .is_err()
    );
    assert!(
        backend::dispatch(shared.clone(), json!({"op":"set_auto","allow":true}), false)
            .await
            .is_err()
    );
    assert!(backend::dispatch(shared.clone(),json!({"op":"approve","task_id":id,"request_id":uuid::Uuid::new_v4().to_string(),"allow":true}),false).await.is_err());
    assert!(
        backend::dispatch(
            shared.clone(),
            json!({"op":"prepare_repositories","group":"test","urls":"https://example.com/a.git"}),
            false
        )
        .await
        .unwrap_err()
        .to_string()
        .contains("只能由人在 App")
    );
    backend::dispatch(shared.clone(), json!({"op":"set_auto","allow":true}), true).await?;
    assert!(
        backend::dispatch(
            shared.clone(),
            json!({"op":"project_remove","workspace":"/not-a-project"}),
            false
        )
        .await
        .unwrap_err()
        .to_string()
        .contains("只能在 App")
    );
    // Session metadata is human-owned; archiving never changes the MCP execution contract.
    let analysis=backend::dispatch(shared.clone(),json!({"op":"start","task_id":uuid::Uuid::new_v4().to_string(),"remote_project":"项目B","prompt":"Session acceptance Read README.md","mode":"automatic"}),false).await?;
    {
        let g = shared.connection.lock().await;
        poll(
            g.as_ref().unwrap(),
            field(&analysis, "task_id")?,
            "completed",
        )
        .await?;
    }
    let session = field(&analysis, "session_id")?;
    let named = json!({"op":"session_meta_set","session_id":session,"expected_revision":0,"title":"稳定分析名称","archived":true});
    assert!(
        backend::dispatch(shared.clone(), named.clone(), false)
            .await
            .is_err()
    );
    backend::dispatch(shared.clone(), named, true).await?;
    let archived = backend::dispatch(
        shared.clone(),
        json!({"op":"session_search","q":"稳定分析","archive_filter":"archived"}),
        true,
    )
    .await?;
    assert_eq!(archived["sessions"][0]["title"], "稳定分析名称");
    let next=backend::dispatch(shared.clone(),json!({"op":"reply","reply_to":analysis["task_id"],"task_id":uuid::Uuid::new_v4().to_string(),"prompt":"continued Read README.md"}),false).await?;
    assert_eq!(next["session_id"], analysis["session_id"]);
    assert_eq!(next["workspace"], analysis["workspace"]);
    assert_eq!(next["mode"], "automatic");
    {
        let g = shared.connection.lock().await;
        poll(g.as_ref().unwrap(), field(&next, "task_id")?, "completed").await?;
    }
    assert_eq!(
        backend::dispatch(
            shared.clone(),
            json!({"op":"session_meta_get","session_id":session}),
            true
        )
        .await?["archived"],
        true
    );
    println!(
        "PASS session search / human metadata / archived MCP automatic continuation / unchanged session and directory"
    );
    let socket = tokio::net::UnixListener::bind(app_dir()?.join("bridge.sock"))?;
    let cloned = shared.clone();
    let broker = tokio::spawn(async move {
        loop {
            let (stream, _) = socket.accept().await.unwrap();
            let state = cloned.clone();
            tokio::spawn(async move {
                let (r, mut w) = stream.into_split();
                let mut line = String::new();
                BufReader::new(r).read_line(&mut line).await.unwrap();
                let v: Value = serde_json::from_str(&line).unwrap();
                let mut bytes =
                    serde_json::to_vec(&envelope(backend::dispatch(state, v, false).await))
                        .unwrap();
                bytes.push(b'\n');
                w.write_all(&bytes).await.unwrap();
            });
        }
    });
    let out = tokio::process::Command::new("python3")
        .args(["tests/mcp_probe.py", &bin, local_repo.to_str().unwrap()])
        .output()
        .await?;
    assert!(
        out.status.success(),
        "{}\n{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    println!("{}", String::from_utf8_lossy(&out.stdout));
    let lease = uuid::Uuid::new_v4().to_string();
    backend::dispatch(
        shared.clone(),
        json!({"op":"client_activity_begin","lease_id":lease,"client_protocol":1}),
        false,
    )
    .await?;
    assert!(
        backend::dispatch(shared.clone(), json!({"op":"prepare_update"}), true)
            .await
            .is_err()
    );
    backend::dispatch(
        shared.clone(),
        json!({"op":"client_activity_end","lease_id":lease,"client_protocol":1}),
        false,
    )
    .await?;
    assert!(
        backend::dispatch(shared.clone(), json!({"op":"prepare_update"}), false)
            .await
            .is_err()
    );
    backend::dispatch(shared.clone(), json!({"op":"prepare_update"}), true).await?;
    assert!(
        backend::dispatch(shared.clone(), start("blocked by drain", "automatic"), true)
            .await
            .is_err()
    );
    assert_eq!(
        backend::dispatch(shared.clone(), json!({"op":"connection_status"}), false).await?["connected"],
        true
    );
    backend::dispatch(shared.clone(), json!({"op":"cancel_update"}), true).await?;
    println!("PASS update lease / drain / no new writes / status remains available");
    broker.abort();
    // Disconnect/reconnect and recover persisted task list without replay.
    shared.connection.lock().await.take();
    let c = ssh::connect(p, "test-only-password".into()).await?;
    assert!(
        c.rpc(json!({"op":"list"})).await?["tasks"]
            .as_array()
            .unwrap()
            .len()
            == 14 // Original twelve plus the new archived-session start/reply pair.
    );
    println!(
        "PASS host pin / wrong password / encrypted SSH / remote code / approve / deny / automatic / cancellation / failure / workspace lock / reconnect / human-only policy"
    );
    std::fs::remove_dir_all(root)?;
    Ok(())
}
