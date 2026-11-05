import type { RecordValue as Data } from "./conversation";
export const sessionActive = (t: Data) =>
  ["running", "queued", "waiting_approval"].includes(t.state);
export function sessionReadTask(t: Data) {
  return {
    id: t.task_id || t.id,
    state: t.ownership_unknown ? "interrupted" : t.state,
    running: sessionActive(t),
    finished: sessionActive(t)
      ? undefined
      : t.updated_at
        ? t.updated_at * 1000
        : undefined,
  };
}
export function sessionStatus(t: Data, connected: boolean) {
  if (t.ownership_unknown || (!connected && sessionActive(t)))
    return "状态待确认";
  return (
    (
      {
        running: "运行中",
        queued: "排队中",
        waiting_approval: "待审批",
        completed: "已完成",
        failed: "失败",
        cancelled: "已取消",
        interrupted: "已中断",
      } as Record<string, string>
    )[t.state] || "状态待确认"
  );
}
