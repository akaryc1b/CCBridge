import type { RecordValue } from "./conversation";
export function queueBlockers(t: RecordValue): RecordValue[] {
  return Array.isArray(t.queue?.blocked_by)
    ? t.queue.blocked_by.filter(
        (b: RecordValue) => b && typeof b.title === "string",
      )
    : [];
}
export function queueLabel(t: RecordValue): string {
  if (t.ownership_unknown) return "状态未知 · 目录仍被占用";
  if (t.state !== "queued") return "";
  if (t.blocked_reason === "policy") return "已暂停 · 禁止修改已开启";
  const first = queueBlockers(t)[0];
  return first
    ? `等待「${first.title}」完成`
    : t.phase === "executing"
      ? "正在启动"
      : "等待执行";
}
export function shouldRefreshQueued(
  task: RecordValue | undefined,
  active: RecordValue[],
): boolean {
  if (!task || !["queued", "running"].includes(task.state)) return false;
  const current = active.find((t) => (t.task_id || t.id) === task.task_id);
  return (
    !current ||
    current.state !== task.state ||
    queueLabel(current) !== queueLabel(task)
  );
}

/** List status is authoritative; transcript caches only supply missing detail. */
export function taskCenterRecord(
  summary: RecordValue,
  detail?: RecordValue,
): RecordValue {
  return { ...detail, ...summary };
}

export function shouldRefreshTaskList(
  cached: RecordValue[],
  observed: RecordValue[],
): boolean {
  return [...cached, ...observed].some(
    (t) =>
      !t.ownership_unknown &&
      ["running", "queued", "waiting_approval"].includes(t.state),
  );
}
