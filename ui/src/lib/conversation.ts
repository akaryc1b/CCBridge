export type RecordValue = Record<string, any>;
export function normalizeTask(task: RecordValue) {
  return {
    ...task,
    task_id: task.task_id || task.id,
    prompt: task.prompt || task.title || "",
  };
}
export function conversationTurns(
  selected: RecordValue | undefined,
  details: Record<string, RecordValue>,
) {
  const turns: RecordValue[] = [];
  const visited = new Set<string>();
  let current = selected;
  while (current && !visited.has(current.task_id)) {
    turns.unshift(current);
    visited.add(current.task_id);
    current = details[current.reply_to];
  }
  return turns;
}
/** Keep all turns in the transcript, show one latest item per session in the sidebar. */
export function latestSessions(tasks: RecordValue[]) {
  const sorted = [...tasks].sort(
    (a, b) => (b.created_at || 0) - (a.created_at || 0),
  );
  const sessions = new Set<string>();
  return sorted.filter((t) => {
    const key = t.session_id || t.task_id;
    if (sessions.has(key)) return false;
    sessions.add(key);
    return true;
  });
}
export function displayModel(task: RecordValue) {
  return {
    requested: task.model_requested || task.model || "远端默认",
    actual:
      task.model_resolved ||
      (["running", "queued"].includes(task.state) ? "等待上报" : "未上报"),
  };
}

export function eventText(event: RecordValue): string {
  if (event.type !== "assistant" || event.parent_tool_use_id) return "";
  return (event.message?.content || [])
    .filter((c: RecordValue) => c.type === "text")
    .map((c: RecordValue) => c.text || "")
    .join("\n\n");
}
export function eventLabel(event: RecordValue): string {
  const tools = (event.message?.content || []).filter(
    (c: RecordValue) => c.type === "tool_use",
  );
  const names: Record<string, string> = {
    Read: "读取文件",
    Bash: "运行命令",
    Edit: "编辑文件",
    Write: "写入文件",
    Grep: "搜索内容",
    Glob: "查找文件",
  };
  if (tools.length)
    return tools.map((t: RecordValue) => names[t.name] || t.name).join("、");
  if (event.type === "user") return "工具返回";
  if (event.type === "assistant") return "Claude 消息";
  if (event.type === "system")
    return event.subtype === "init" ? "会话初始化" : "运行状态";
  return event.type === "result" ? "本轮结果" : "运行事件";
}

/** Most-specific registered project owns its subdirectories; unrelated paths stay separate. */
export function groupSessions(tasks: RecordValue[], projects: RecordValue[]) {
  const groups = new Map<
    string,
    { path: string; name: string; registered: boolean; sessions: RecordValue[] }
  >();
  for (const t of latestSessions(tasks)) {
    const workspace = String(t.workspace || "").replace(/\/$/, "");
    const project = projects
      .filter(
        (p) =>
          workspace === p.path ||
          workspace.startsWith(p.path.replace(/\/$/, "") + "/"),
      )
      .sort((a, b) => b.path.length - a.path.length)[0];
    const path = project?.path || workspace;
    if (!groups.has(path))
      groups.set(path, {
        path,
        name:
          project?.name ||
          (workspace
            ? `其他目录 · ${workspace.split("/").pop()}`
            : "未记录项目"),
        registered: !!project,
        sessions: [],
      });
    groups.get(path)!.sessions.push(t);
  }
  return [...groups.values()]
    .map((g) => ({
      ...g,
      sessions: g.sessions.sort(
        (a, b) =>
          (b.updated_at || b.created_at || 0) -
          (a.updated_at || a.created_at || 0),
      ),
    }))
    .sort(
      (a, b) =>
        Math.max(...b.sessions.map((t) => t.updated_at || t.created_at || 0)) -
        Math.max(...a.sessions.map((t) => t.updated_at || t.created_at || 0)),
    );
}

/** A concrete configured name, never an invented resolution of a remote default. */
export function nextModelLabel(
  task: RecordValue | undefined,
  serverModel: string,
) {
  if (!task) return serverModel?.trim() || "远端默认";
  const requested = task.model_requested || task.model;
  if (requested) return requested;
  return task.model_resolved
    ? `${task.model_resolved}（远端默认）`
    : "远端默认";
}

/** Known service failures are actionable state, not assistant prose. */
export function taskFailure(task: RecordValue): string | null {
  const result = task.result;
  const events = task.events || [];
  const limited =
    result?.api_error === "usage_limit_reached" ||
    events.some((e: RecordValue) => e.api_error === "usage_limit_reached");
  if (!limited) return null;
  const reset =
    events.find((e: RecordValue) => e.rate_limit_info?.resetsAt)
      ?.rate_limit_info?.resetsAt ||
    events.find(
      (e: RecordValue) => e.api_error_params?.rate_limit_info?.resetsAt,
    )?.api_error_params?.rate_limit_info?.resetsAt;
  const time = reset
    ? new Date(reset * 1000).toLocaleString("zh-CN", {
        month: "2-digit",
        day: "2-digit",
        hour: "2-digit",
        minute: "2-digit",
        hour12: false,
      })
    : "";
  return `Claude 用量已达上限${time ? `，预计 ${time} 恢复` : ""}。恢复后可继续此会话。`;
}
