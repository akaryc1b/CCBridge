import { useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import {
  RefreshCw,
  CheckCircle2,
  Clock,
  Loader2,
  AlertCircle,
  Copy,
} from "lucide-react";
import { Button } from "./ui/button";
import {
  Dialog,
  DialogContent,
  DialogHeader,
  DialogTitle,
  DialogDescription,
  DialogFooter,
} from "./ui/dialog";
import { errorText } from "./CodeupAuth";
import { useOperation } from "@/lib/useOperation";
import { OperationBar, InlineAlert } from "./common/Feedback";
type D = Record<string, any>;
export function GitUpdate({
  open,
  path,
  connectionId,
  onClose,
  onUpdated,
  onAuth,
  onOperation,
  operationId,
}: {
  open: boolean;
  path: string;
  connectionId: string;
  onClose: () => void;
  onUpdated: () => void;
  onAuth: (url: string) => void;
  onOperation?: (op: D) => void;
  operationId?: string;
}) {
  const [plan, setPlan] = useState<D[]>([]),
    [result, setResult] = useState<D[]>([]),
    [checking, setChecking] = useState(false),
    [starting, setStarting] = useState(false),
    [id, setId] = useState<string>(),
    [error, setError] = useState("");
  const [verifiedUnknown, setVerifiedUnknown] = useState(false);
  const [reviewing, setReviewing] = useState(false);
  const scope = useRef(0);
  const { snap, error: pollError } = useOperation(
    "git_update",
    path,
    connectionId,
    open || !!id,
    id,
  );
  const running = starting || snap?.state === "running";
  useEffect(() => {
    scope.current++;
    setId(undefined);
    setReviewing(false);
    setVerifiedUnknown(false);
    setChecking(false);
    setStarting(false);
    setPlan([]);
    setResult([]);
    setError("");
  }, [path, connectionId, operationId]);
  useEffect(() => {
    if (open) setId(operationId);
  }, [open, operationId, path, connectionId]);
  useEffect(() => {
    if (snap) {
      onOperation?.(snap);
      if (!reviewing && snap.result?.results) setResult(snap.result.results);
    }
  }, [snap]);
  async function inspect(manual = false) {
    if (checking) return null;
    const seq = scope.current;
    setChecking(true);
    setError("");
    try {
      const r = await invoke<D>("git_update_plan", { request: { path } });
      if (seq === scope.current) {
        setPlan(r.repositories || []);
        if (manual) {
          setVerifiedUnknown(true);
          setReviewing(true);
          setResult([]);
        }
      }
      return r.repositories || [];
    } catch (e) {
      if (seq === scope.current) setError(errorText(e));
      return null;
    } finally {
      if (seq === scope.current) setChecking(false);
    }
  }
  useEffect(() => {
    if (open && !running) void inspect();
  }, [open, path, connectionId]);
  async function update(only?: string[]) {
    if (running) return;
    setVerifiedUnknown(false);
    setReviewing(false);
    setStarting(true);
    setError("");
    const seq = scope.current;
    const target = path;
    let expected = plan;
    if (only) {
      expected = await inspect();
      if (!expected) {
        setStarting(false);
        return;
      }
    }
    const opid = crypto.randomUUID();
    setId(opid);
    setResult([]);
    try {
      const r = await invoke<D>("git_update", {
        request: {
          path: target,
          expected,
          operation_id: opid,
          connection_id: connectionId,
          ...(only ? { only } : {}),
        },
      });
      if (seq !== scope.current) return;
      setResult(r.results || []);
      onUpdated();
    } catch (e) {
      if (seq === scope.current) setError(errorText(e));
    } finally {
      if (seq === scope.current) setStarting(false);
    }
  }
  const rows = plan.map((r) => ({
    ...r,
    ...(!reviewing ? snap?.items || [] : []).find((x: D) => x.path === r.path),
    ...result.find((x) => x.path === r.path),
  }));
  const failed = rows.filter(
    (r) => r.state === "failed" && ["auth", "network"].includes(r.code),
  );
  const unknown = snap?.state === "unknown";
  const finished = !!snap && snap.state !== "running" && !reviewing;
  const completed = finished && snap.state === "done";
  return (
    <Dialog
      open={open}
      onOpenChange={(v) => {
        if (!v) onClose();
      }}
    >
      <DialogContent className="git-update-dialog">
        <DialogHeader>
          <DialogTitle>
            <RefreshCw size={18} />
            Git 更新
          </DialogTitle>
          <DialogDescription title={path}>
            {path.split("/").pop()} ·{" "}
            {snap && !reviewing
              ? snap.total || plan.length || "…"
              : checking && !plan.length
                ? "…"
                : plan.length}{" "}
            个仓库{" "}
            <Button
              variant="ghost"
              size="icon"
              aria-label="复制目录"
              onClick={() => void navigator.clipboard.writeText(path)}
            >
              <Copy size={13} />
            </Button>
          </DialogDescription>
        </DialogHeader>
        <div className="dialog-body">
          <OperationBar operation={reviewing ? null : snap} />
          {checking && !snap && (
            <p role="status">
              <Loader2 size={14} className="spin" />
              检查仓库…
            </p>
          )}
          <div className="repo-list">
            {rows.map((r) => {
              const current = running && snap?.current_path === r.path;
              const blocked =
                (r.blocked && !r.state) ||
                ["dirty", "no_upstream"].includes(r.code);
              const message = blocked
                ? `跳过 · ${r.blocked || r.message}`
                : r.state === "updated"
                  ? "已快进"
                  : r.state === "unchanged"
                    ? "已是最新"
                    : r.state === "ahead"
                      ? "本地领先"
                      : r.state === "failed"
                        ? r.message
                        : current
                          ? "处理中"
                          : running
                            ? "等待"
                            : "可检查更新";
              const Icon =
                r.state === "failed"
                  ? AlertCircle
                  : r.state
                    ? CheckCircle2
                    : current
                      ? Loader2
                      : Clock;
              return (
                <div className="repo-item" key={r.path}>
                  <Icon
                    size={17}
                    className={
                      current
                        ? "spin"
                        : r.state === "failed"
                          ? "text-destructive"
                          : "muted"
                    }
                  />
                  <div className="grow">
                    <strong>{r.name || r.path.split("/").pop()}</strong>
                    <small>
                      {r.branch || "分离 HEAD"} ← {r.upstream || "无上游"}
                    </small>
                    {r.before && r.after && r.before !== r.after && (
                      <code>
                        {r.before.slice(0, 7)} → {r.after.slice(0, 7)}
                      </code>
                    )}
                    <span
                      className={
                        r.state === "failed" ? "text-destructive" : "muted"
                      }
                    >
                      {message}
                    </span>
                  </div>
                  {r.state === "failed" &&
                    r.code === "auth" &&
                    r.url &&
                    !blocked && (
                      <Button
                        variant="ghost"
                        size="sm"
                        onClick={() => onAuth(r.url)}
                      >
                        修复认证
                      </Button>
                    )}
                </div>
              );
            })}
          </div>
          {(error || pollError) && (
            <InlineAlert>{error || pollError}</InlineAlert>
          )}
        </div>
        <DialogFooter>
          <Button variant="outline" onClick={onClose}>
            {running ? "后台运行" : completed ? "完成" : "关闭"}
          </Button>
          {!running && (
            <>
              {(finished || reviewing) && (
                <Button
                  variant={finished && !failed.length ? "default" : "outline"}
                  disabled={checking}
                  onClick={() => void inspect(true)}
                >
                  {checking ? "检查中…" : finished ? "检查新更新" : "重新检查"}
                </Button>
              )}
              {!completed && (!finished || failed.length > 0) && (
                <Button
                  disabled={
                    checking ||
                    !plan.some((r) => !r.blocked) ||
                    (unknown && !verifiedUnknown) ||
                    (finished && !failed.length)
                  }
                  onClick={() =>
                    void update(
                      failed.length ? failed.map((r) => r.path) : undefined,
                    )
                  }
                >
                  {failed.length
                    ? "重试失败项"
                    : `快进更新 ${plan.filter((r) => !r.blocked).length} 个仓库`}
                </Button>
              )}
            </>
          )}
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
