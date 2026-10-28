type Task = {
  id: string;
  state?: string;
  running: boolean;
  finished?: number;
  error?: string;
  verification?: string;
  verifiedAt?: number;
};
export type ReadTasks = Record<string, string>;
export const taskState = (t: Task) =>
  t.state || (t.running ? "running" : "done");
export const needsAction = (t: Task) =>
  ["queued", "running", "waiting_approval"].includes(taskState(t));
export const readVersion = (t: Task) =>
  JSON.stringify([taskState(t), t.finished || 0, t.verifiedAt || 0]);
export const isTaskRead = (t: Task, read: ReadTasks) =>
  read[t.id] === readVersion(t);
export function markTasksRead(tasks: Task[], previous: ReadTasks): ReadTasks {
  const next = { ...previous };
  for (const task of tasks)
    if (!needsAction(task)) next[task.id] = readVersion(task);
  return Object.fromEntries(Object.entries(next).slice(-1000));
}
export function taskAction(kind?: string) {
  return (
    (
      {
        git_update: "查看更新结果",
        branch_fetch: "查看分支",
        branch_change: "查看分支",
        clone_into_directory: "查看克隆进度",
        install_runner: "查看服务端程序",
      } as Record<string, string>
    )[kind || ""] || "查看详情"
  );
}
