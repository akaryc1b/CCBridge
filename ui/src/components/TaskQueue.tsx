import { useState } from "react";
import { Clock, ArrowRight, RefreshCw, TriangleAlert } from "lucide-react";
import { Button } from "./ui/button";
import { Confirm } from "./common/Confirm";
import { queueLabel, queueBlockers } from "../lib/taskQueue";
import type { RecordValue as Data } from "../lib/conversation";
export function TaskQueue({
  task,
  onOpen,
  onCancel,
  onCheck,
  onRelease,
}: {
  task: Data;
  onOpen?: (id: string) => void;
  onCancel?: () => void;
  onCheck?: () => void;
  onRelease?: () => Promise<void>;
}) {
  const [confirm, C] = useState(false),
    [busy, B] = useState(false),
    [error, E] = useState("");
  const title = queueLabel(task);
  if (!title) return null;
  const unknown = task.ownership_unknown,
    blocker = queueBlockers(task).find((b) => b.kind === "task" && b.id);
  return (
    <div className="task-queue-status" role="status">
      <div className="task-queue-title">
        {unknown ? <TriangleAlert size={18} /> : <Clock size={18} />}
        <strong>
          {unknown
            ? "状态未知 · 目录仍被占用"
            : task.state === "queued"
              ? task.blocked_reason === "policy"
                ? "已暂停"
                : "排队中"
              : title}
        </strong>
      </div>
      <div className="task-queue-description">
        <span>
          {unknown
            ? "未能确认 Claude 已结束。为避免同时修改，后续任务继续等待。"
            : title}
        </span>
        {blocker && onOpen && (
          <Button variant="ghost" size="sm" onClick={() => onOpen(blocker.id)}>
            查看任务
            <ArrowRight size={14} />
          </Button>
        )}
      </div>
      <div className="task-queue-actions">
        {!unknown && onCancel && (
          <Button variant="outline" size="sm" onClick={onCancel}>
            取消排队
          </Button>
        )}
        {unknown && onRelease && (
          <Button variant="outline" size="sm" onClick={() => C(true)}>
            释放占用…
          </Button>
        )}
        {unknown && onCheck && (
          <Button size="sm" onClick={onCheck}>
            <RefreshCw size={14} />
            重新检查
          </Button>
        )}
      </div>
      {error && !confirm && <p className="inline-error">{error}</p>}
      <Confirm
        open={confirm}
        title="释放此目录的占用？"
        action="确认已结束并释放"
        destructive
        busy={busy}
        error={error}
        onCancel={() => C(false)}
        onConfirm={() => {
          B(true);
          E("");
          void onRelease?.()
            .then(() => C(false))
            .catch((e) => E(e.message || "释放失败，请重新检查"))
            .finally(() => B(false));
        }}
      >
        仅在你已确认原任务及其子进程停止后继续。释放后，排队任务可能立即开始；如果原任务仍在运行，可能同时修改文件。
      </Confirm>
    </div>
  );
}
