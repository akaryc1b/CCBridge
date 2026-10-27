use crate::common::*;
use anyhow::Result;
use rmcp::{
    ServerHandler, ServiceExt,
    handler::server::{router::tool::ToolRouter, wrapper::Parameters},
    model::*,
    tool, tool_handler, tool_router,
};
use schemars::JsonSchema;
use serde::Deserialize;
use serde_json::{Value, json};

#[derive(Clone)]
struct Bridge {
    tool_router: ToolRouter<Self>,
}
#[derive(Deserialize, JsonSchema)]
struct Start {
    /// analysis: parallel read/search only, existing Claude login. write: file changes, blocked by human policy. Omitted follows account policy.
    access: Option<String>,
    /// queue (new Runner default): persist until workspace is free; reject returns busy.
    on_conflict: Option<String>,
    /// Task instruction. Code must already exist in the configured remote workspace.
    prompt: String,
    /// approval (default) or automatic. Automatic requires the human to enable it in the App.
    mode: Option<String>,
    /// Optional Claude model alias or full model ID.
    model: Option<String>,
    /// inherit (start: server default; reply: previous turn), remote_default, explicit.
    model_policy: Option<String>,
    /// Optional UUID for idempotent creation after an uncertain connection response.
    task_id: Option<String>,
    /// Absolute LOCAL Git 仓库 path from the user task. Matched to an existing remote repository by Git remote identity. Omit to use the remote directory explicitly selected in the App; if none, retain legacy local selection behavior.
    local_repository: Option<String>,
    /// Registered remote project path or unique name from project_list. Pins this task independently of App selection. Do not combine with local_repository.
    remote_project: Option<String>,
}
#[derive(Deserialize, JsonSchema)]
struct MatchRepository {
    local_repository: String,
}
#[derive(Deserialize, JsonSchema)]
struct GitUpdate {
    /// Exact absolute REMOTE repository or registered multi-repository project directory. Never use the App default implicitly.
    path: String,
    /// true only inspects repositories and upstreams; false fetches and fast-forwards clean repositories. Never stash/reset/push.
    dry_run: Option<bool>,
    /// Optional repositories array returned by dry_run. Pins the reviewed branch/head/upstream set.
    expected: Option<Vec<Value>>,
}
#[derive(Deserialize, JsonSchema)]
struct Id {
    task_id: String,
}
#[derive(Deserialize, JsonSchema)]
struct ResultWait {
    task_id: String,
    /// Wait for completion up to 900 seconds; returns early for human approval. Set MCP client tool timeout above this value.
    wait_seconds: Option<u64>,
    /// false omits large raw event arrays. Defaults to true for compatibility.
    include_events: Option<bool>,
}
#[derive(Deserialize, JsonSchema)]
struct Status {
    task_id: String,
    /// Byte cursor from next_cursor. Omit for first page.
    cursor: Option<u64>,
    /// Wait up to 30 seconds for terminal state or human approval.
    wait_seconds: Option<u64>,
}
#[derive(Deserialize, JsonSchema)]
struct Reply {
    /// analysis or write; omitted preserves previous analysis scope and respects account read-only policy.
    access: Option<String>,
    /// queue or reject; same-session concurrent replies are always refused.
    on_conflict: Option<String>,
    /// Optional model override for this turn; omitted means previous turn selection.
    model: Option<String>,
    model_policy: Option<String>,
    task_id: String,
    prompt: String,
    /// Optional UUID for this new turn, for idempotent retries.
    new_task_id: Option<String>,
}
fn output(r: Result<Value>) -> std::result::Result<CallToolResult, rmcp::ErrorData> {
    Ok(match r {
        Ok(v) => CallToolResult::success(vec![Content::text(
            serde_json::to_string_pretty(&v).unwrap(),
        )]),
        Err(e) => CallToolResult::error(vec![Content::text(format!("{e:#}"))]),
    })
}
#[tool_router]
impl Bridge {
    fn new() -> Self {
        Self {
            tool_router: Self::tool_router(),
        }
    }
    #[tool(
        description = "Get SSH connection, configured remote workspace and human-enabled permissions. Login is performed in the CCBridge App, never send passwords through MCP."
    )]
    async fn connection_status(&self) -> std::result::Result<CallToolResult, rmcp::ErrorData> {
        output(local_call(json!({"op":"connection_status"})).await)
    }
    #[tool(
        description = "List registered remote projects and their named database mappings without credentials. Use the exact project path as cc_start.remote_project to avoid cross-project routing when multiple Codex sessions run."
    )]
    async fn project_list(&self) -> std::result::Result<CallToolResult, rmcp::ErrorData> {
        output(local_call(json!({"op":"project_list"})).await)
    }
    #[tool(
        description = "Read the local Git 仓库 identity and find matching existing repositories under the authenticated remote user's HOME. Strips URL credentials. Never matches by folder name or clones/uploads files. Multiple candidates, incomplete scans or no matches require user selection in the App."
    )]
    async fn repository_match(
        &self,
        Parameters(p): Parameters<MatchRepository>,
    ) -> std::result::Result<CallToolResult, rmcp::ErrorData> {
        output(
            local_call(json!({"op":"repository_match","local_repository":p.local_repository}))
                .await,
        )
    }
    #[tool(
        description = "Inspect or update an explicit REMOTE Git repository/project using App remote Codeup credentials or VPS Git credentials. Fetch the configured upstream then fast-forward only. Refuses dirty, divergent, detached, changed-branch and active-task repositories; never reset/stash/push. Multiple repos return individual results. Local repositories remain managed by Codex local Git tools. Never supply credentials in this tool."
    )]
    async fn git_update(
        &self,
        Parameters(p): Parameters<GitUpdate>,
    ) -> std::result::Result<CallToolResult, rmcp::ErrorData> {
        output(local_call(json!({"op":if p.dry_run.unwrap_or(false){"git_update_plan"}else{"git_update"},"path":p.path,"expected":p.expected})).await)
    }
    #[tool(
        description = "Start remote Claude Code task. Omit local_repository to use the remote workspace selected by the human in the App. Supply it only when matching a specific LOCAL Git repository to its remote clone. Conflicts queue on capable Runner; queued is not failure. Pass on_conflict=reject for immediate busy. Returns task_id immediately. Only remote files are visible; this does not upload local files. Never retry an uncertain start with a different ID: list tasks or reuse the ID."
    )]
    async fn cc_start(
        &self,
        Parameters(p): Parameters<Start>,
    ) -> std::result::Result<CallToolResult, rmcp::ErrorData> {
        let id = p
            .task_id
            .unwrap_or_else(|| uuid::Uuid::new_v4().to_string());
        output(local_call(json!({"op":"start","access":p.access,"on_conflict":p.on_conflict,"prompt":p.prompt,"mode":p.mode.unwrap_or("approval".into()),"task_id":id,"local_repository":p.local_repository,"remote_project":p.remote_project,"model":p.model,"model_policy":p.model_policy})).await)
    }
    #[tool(
        description = "Get task state, incremental events, final result, permission requests and next_cursor. Can wait up to 30s. Human approval is only available in the App; do not approve through shell or request CLI."
    )]
    async fn cc_status(
        &self,
        Parameters(p): Parameters<Status>,
    ) -> std::result::Result<CallToolResult, rmcp::ErrorData> {
        let end = tokio::time::Instant::now()
            + std::time::Duration::from_secs(p.wait_seconds.unwrap_or(0).min(30));
        loop {
            let r = local_call(
                json!({"op":"status","task_id":p.task_id,"cursor":p.cursor.unwrap_or(0)}),
            )
            .await;
            match &r {
                Ok(v)
                    if ["queued", "running"].contains(&v["state"].as_str().unwrap_or(""))
                        && v["events"].as_array().is_none_or(|a| a.is_empty())
                        && tokio::time::Instant::now() < end =>
                {
                    tokio::time::sleep(std::time::Duration::from_millis(600)).await;
                }
                _ => return output(r),
            }
        }
    }
    #[tool(
        description = "List recent tasks owned by the authenticated SSH account (plus its explicitly configured legacy workspace), including after reconnecting."
    )]
    async fn cc_list(&self) -> std::result::Result<CallToolResult, rmcp::ErrorData> {
        output(local_call(json!({"op":"list"})).await)
    }
    #[tool(
        description = "Continue a finished task's exact Claude session with a follow-up. Returns a NEW task_id for this turn, retaining session_id and permission mode. Wait for the previous task to finish first."
    )]
    async fn cc_reply(
        &self,
        Parameters(p): Parameters<Reply>,
    ) -> std::result::Result<CallToolResult, rmcp::ErrorData> {
        output(local_call(json!({"op":"reply","access":p.access,"on_conflict":p.on_conflict,"reply_to":p.task_id,"prompt":p.prompt,"model":p.model,"model_policy":p.model_policy,"task_id":p.new_task_id.unwrap_or_else(||uuid::Uuid::new_v4().to_string())})).await)
    }
    #[tool(
        description = "Request cancellation of a running remote task. Query status until cancelled; cancellation does not roll back file edits."
    )]
    async fn cc_cancel(
        &self,
        Parameters(p): Parameters<Id>,
    ) -> std::result::Result<CallToolResult, rmcp::ErrorData> {
        output(local_call(json!({"op":"cancel","task_id":p.task_id})).await)
    }
    #[tool(
        description = "Get final result and state of a task. For unattended automatic runs, set wait_seconds=900 to receive the finished result in this call (or running if deadline expires). A completed turn is not independent verification of Claude's reported changes."
    )]
    async fn cc_result(
        &self,
        Parameters(p): Parameters<ResultWait>,
    ) -> std::result::Result<CallToolResult, rmcp::ErrorData> {
        let lease = uuid::Uuid::new_v4().to_string();
        if let Err(e) = local_call(json!({"op":"client_activity_begin","lease_id":lease})).await {
            return output(Err(e));
        }
        let end = tokio::time::Instant::now()
            + std::time::Duration::from_secs(p.wait_seconds.unwrap_or(0).min(900));
        let mut renewed = tokio::time::Instant::now();
        let result = loop {
            if renewed.elapsed() > std::time::Duration::from_secs(30) {
                if let Err(e) =
                    local_call(json!({"op":"client_activity_renew","lease_id":lease})).await
                {
                    break Err(e);
                }
                renewed = tokio::time::Instant::now();
            }
            let r = local_call(json!({"op":"result","task_id":p.task_id})).await;
            match r {
                Ok(v)
                    if ["queued", "running"].contains(&v["state"].as_str().unwrap_or(""))
                        && tokio::time::Instant::now() < end =>
                {
                    tokio::time::sleep(std::time::Duration::from_secs(1)).await
                }
                other => break other,
            }
        };
        let _ = local_call(json!({"op":"client_activity_end","lease_id":lease})).await;
        output(result.map(|mut v| {
            if p.include_events == Some(false) {
                v.as_object_mut().unwrap().remove("events");
            }
            v
        }))
    }
}
#[tool_handler]
impl ServerHandler for Bridge {
    fn get_info(&self) -> ServerInfo {
        ServerInfo{instructions:Some("CCBridge routes tasks to a remote Claude Code. Use only when the user authorizes delegation. Check connection_status then project_list. For a named project, pass its exact registered path as cc_start.remote_project; never silently use another App-selected project. Multiple projects may each map multiple named databases. Omitting remote_project retains the App-selected workspace. If the user opened a remote workspace in the App, cc_start without local_repository uses that directory, including a multi-repository project group. If the user specifically asks to match a local repository, pass its actual local_repository path to repository_match and cc_start; an explicit local path takes precedence. Do not guess by folder name. Check local/remote branch and dirty flags; no local changes are synchronized. If ambiguous, ask the human to choose in the App. Existing tasks retain their original workspace across App selection changes. Login, automatic-mode enablement and tool approvals belong to the human App UI. Never obtain SSH credentials. Reuse sessions for follow-ups; do not concurrently edit the same remote workspace. cc_start/cc_reply return immediately; use cc_status until completed/failed/cancelled/waiting_approval. Present final result and genuine errors to caller. Remote workspace is NOT an OS sandbox.".into()),capabilities:ServerCapabilities::builder().enable_tools().build(),..Default::default()}
    }
}
pub async fn serve_bridge() -> Result<()> {
    Bridge::new()
        .serve(rmcp::transport::stdio())
        .await?
        .waiting()
        .await?;
    Ok(())
}

#[derive(Clone)]
struct Permissions {
    task_id: String,
    tool_router: ToolRouter<Self>,
}
#[derive(Deserialize, JsonSchema)]
struct PermissionInput {
    tool_name: String,
    input: Value,
    tool_use_id: Option<String>,
}
#[tool_router]
impl Permissions {
    fn new(task_id: String) -> Self {
        Self {
            task_id,
            tool_router: Self::tool_router(),
        }
    }
    #[tool(
        description = "Ask the human in CCBridge to allow or deny a Claude Code tool invocation."
    )]
    async fn approve(
        &self,
        Parameters(p): Parameters<PermissionInput>,
    ) -> std::result::Result<CallToolResult, rmcp::ErrorData> {
        let r:Result<Value>=async {
            let dir=task_dir(&self.task_id)?;let rid=uuid::Uuid::new_v4().to_string();
            let path=dir.join(format!("approval-{rid}.json"));
            let mut request=json!({"request_id":rid,"tool_name":p.tool_name,"input":p.input,"tool_use_id":p.tool_use_id,"created_at":now(),"resolved":false});
            write_json(&path,&request)?;
            let deadline=tokio::time::Instant::now()+std::time::Duration::from_secs(3600);
            let allow=loop {
                if dir.join("cancel").exists() || tokio::time::Instant::now()>deadline {break false}
                if let Ok(v)=read_json::<Value>(&dir.join(format!("decision-{rid}.json"))) {break v["allow"]==true}
                tokio::time::sleep(std::time::Duration::from_millis(250)).await;
            };
            request["resolved"]=json!(true);request["allow"]=json!(allow);write_json(&path,&request)?;
            Ok(if allow {json!({"behavior":"allow","updatedInput":p.input})}else{json!({"behavior":"deny","message":"Human denied, cancelled, or approval timed out. Do not retry this action."})})
        }.await;
        output(r)
    }
}
#[tool_handler]
impl ServerHandler for Permissions {
    fn get_info(&self) -> ServerInfo {
        ServerInfo {
            capabilities: ServerCapabilities::builder().enable_tools().build(),
            ..Default::default()
        }
    }
}
pub async fn serve_permissions(id: &str) -> Result<()> {
    valid_id(id)?;
    Permissions::new(id.into())
        .serve(rmcp::transport::stdio())
        .await?
        .waiting()
        .await?;
    Ok(())
}
