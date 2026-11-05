type D = Record<string, any>;
/** Build one text block per main message; completed frames replace streamed prefixes. */
export function streamingText(events: D[]): string[] {
  const messages = new Map<
    string,
    { partial: Map<number, string>; complete?: string }
  >();
  let current = "";
  for (let i = 0; i < events.length; i++) {
    const frame = events[i];
    if (frame.parent_tool_use_id) continue;
    const e = frame.type === "stream_event" ? frame.event : frame;
    if (!e) continue;
    if (e.type === "message_start") {
      current = e.message?.id || `partial-${i}`;
      if (!messages.has(current)) messages.set(current, { partial: new Map() });
    } else if (
      e.type === "content_block_delta" &&
      e.delta?.type === "text_delta" &&
      current
    ) {
      const m = messages.get(current)!;
      const index = e.index || 0;
      m.partial.set(index, (m.partial.get(index) || "") + e.delta.text);
    } else if (e.type === "assistant") {
      const id = e.message?.id || `legacy-${i}`;
      const m = messages.get(id) || { partial: new Map<number, string>() };
      m.complete = (e.message?.content || [])
        .filter((c: D) => c.type === "text")
        .map((c: D) => c.text || "")
        .join("\n\n");
      messages.set(id, m);
    }
  }
  return [...messages.values()]
    .map(
      (m) =>
        m.complete ??
        [...m.partial.entries()]
          .sort((a, b) => a[0] - b[0])
          .map(([, s]) => s)
          .join("\n\n"),
    )
    .filter(Boolean);
}
export function speedLabel(task: D, connected: boolean, now: number): string {
  if (task.ownership_unknown) return "结果待核实";
  const t = task.telemetry;
  if (task.state !== "running")
    return Number.isFinite(t?.average_tokens_per_second)
      ? `本轮平均 ${t.average_tokens_per_second.toFixed(1)} tokens/s`
      : "";
  if (!connected) return "连接中断 · 速度暂停";
  if (!t) return ""; // Old Runner, no fabricated value.
  const age =
    Number.isFinite(t.age_ms) && Number.isFinite(task.telemetry_received_at)
      ? t.age_ms + Math.max(0, now - task.telemetry_received_at)
      : now - t.observed_at_ms;
  if (age > 5000) return "等待输出…";
  if (t.phase === "tool") return "执行工具中";
  if (t.phase === "thinking") return "思考中…";
  if (t.phase !== "generating" || !Number.isFinite(t.tokens_per_second))
    return "等待输出…";
  return `${t.speed_source === "text_heuristic" ? "≈ " : ""}${t.tokens_per_second.toFixed(1)} tokens/s${t.speed_source === "text_heuristic" ? " · 实时估算" : ""}`;
}

export function referenceDraft(draft: string, path: string): string {
  const ref = `@${/\s/.test(path) ? JSON.stringify(path) : path} `;
  return draft.endsWith("@")
    ? draft.slice(0, -1) + ref
    : draft + (draft ? "\n" : "") + ref;
}

export function toolSummary(events: D[]): string {
  const tools = new Map<string, string>();
  for (const e of events)
    for (const t of e.message?.content || [])
      if (t.type === "tool_use")
        tools.set(t.id || `${e.message?.id}:${t.name}`, t.name);
  const names = [...tools.values()];
  const read = names.filter((n) => n === "Read").length,
    search = names.filter((n) => ["Glob", "Grep"].includes(n)).length,
    shell = names.filter((n) => n === "Bash").length,
    other = names.length - read - search - shell;
  return (
    [
      read ? `读取 ${read} 个文件` : "",
      search ? `搜索 ${search} 次` : "",
      shell ? `执行 ${shell} 条命令` : "",
      other ? `${other} 次其他工具调用` : "",
    ]
      .filter(Boolean)
      .join(" · ") || "运行记录"
  );
}
