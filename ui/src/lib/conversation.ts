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
    requested: task.model_requested || task.model || "远端默认模型",
    actual: task.model_resolved || "实际模型尚未上报",
  };
}
