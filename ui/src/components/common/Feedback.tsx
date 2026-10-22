import { useEffect, useState, type ReactNode } from "react";
import { Loader2 } from "lucide-react";
import { Progress } from "../ui/progress";
import { elapsed, bytes, phaseText } from "@/lib/format";
export function InlineAlert({
  children,
  kind = "error",
}: {
  children: ReactNode;
  kind?: string;
}) {
  return (
    <div
      role={kind === "error" ? "alert" : "status"}
      className={`inline-alert ${kind}`}
    >
      {children}
    </div>
  );
}
export function StatusRow({
  label,
  children,
  action,
}: {
  label: string;
  children: ReactNode;
  action?: ReactNode;
}) {
  return (
    <div className="status-row">
      <span>{label}</span>
      <div className="status-value">
        {children}
        {action}
      </div>
    </div>
  );
}
export function EmptyState({
  title,
  action,
}: {
  title: string;
  action?: ReactNode;
}) {
  return (
    <div className="empty-state">
      <p>{title}</p>
      {action}
    </div>
  );
}
export function OperationBar({
  operation,
}: {
  operation: Record<string, any> | null;
}) {
  const [now, setNow] = useState(Date.now());
  useEffect(() => {
    if (operation?.state !== "running") return;
    const t = setInterval(() => setNow(Date.now()), 1000);
    return () => clearInterval(t);
  }, [operation?.state]);
  if (!operation) return null;
  const o = operation;
  const running = o.state === "running";
  const summary =
    o.state === "partial"
      ? "部分完成"
      : o.state === "done"
        ? "已完成"
        : o.state === "unknown"
          ? "结果未确认"
          : o.state === "failed"
            ? "失败"
            : "";
  return (
    <div className="operation-bar" role="status" aria-live="polite">
      <div className="operation-label">
        {running && <Loader2 size={14} className="spin" />}
        <strong>
          {running
            ? `${phaseText[o.phase] || o.phase || "处理中"} ${o.name || ""}`
            : summary}
        </strong>
        {o.total > 0 && (
          <span>
            已处理 {o.processed}/{o.total}
          </span>
        )}
        <time>{elapsed((o.finished_at_ms || now) - o.started_at_ms)}</time>
      </div>
      {o.total > 0 && (
        <Progress
          value={(o.processed / o.total) * 100}
          aria-label="已处理仓库数"
        />
      )}
      {o.objects && (
        <small>
          对象 {o.objects.received} / {o.objects.total}
        </small>
      )}
      {o.bytes && (
        <div className="transfer-progress">
          <span>
            已发送 {bytes(o.bytes.received)}
            {o.bytes.total ? ` / ${bytes(o.bytes.total)}` : ""}
          </span>
          {o.bytes.total > 0 && (
            <>
              <span>
                {Math.floor((o.bytes.received / o.bytes.total) * 100)}%
              </span>
              <Progress
                value={(o.bytes.received / o.bytes.total) * 100}
                aria-label="上传字节"
              />
            </>
          )}
        </div>
      )}
      {o.error && <InlineAlert>{o.error}</InlineAlert>}
    </div>
  );
}
