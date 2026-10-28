import { useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { Button } from "./ui/button";
import {
  Dialog,
  DialogContent,
  DialogHeader,
  DialogTitle,
  DialogDescription,
  DialogFooter,
} from "./ui/dialog";
import { InlineAlert, OperationBar } from "./common/Feedback";
import { useOperation } from "@/lib/useOperation";
import { Confirm } from "./common/Confirm";
import { errorText } from "./CodeupAuth";
type D = Record<string, any>;
export function RepositoryFetch({
  request,
  connectionId,
  open,
  onClose,
  onChanged,
  onAuth,
  onRetry,
}: {
  request: {
    path: string;
    remote?: string;
    id: string;
    launch: boolean;
    only?: string[];
  };
  connectionId: string;
  open: boolean;
  onClose: () => void;
  onChanged: () => void;
  onAuth: (url: string) => void;
  onRetry: (paths: string[]) => void;
}) {
  const { snap, error: pollError } = useOperation(
    "git_fetch",
    request.path,
    connectionId,
    true,
    request.id,
  );
  const launched = useRef(false),
    changed = useRef(false),
    [error, setError] = useState(""),
    [checking, setChecking] = useState(false),
    [verified, setVerified] = useState(false),
    [releaseConfirm, setReleaseConfirm] = useState(false),
    [released, setReleased] = useState(false);
  const alive = useRef(true);
  useEffect(
    () => () => {
      alive.current = false;
    },
    [],
  );
  useEffect(() => {
    if (!request.launch || launched.current) return;
    launched.current = true;
    void invoke("git_fetch", {
      request: {
        path: request.path,
        remote: request.remote,
        only: request.only,
        operation_id: request.id,
        connection_id: connectionId,
      },
    }).catch((e) => {
      if (alive.current) setError(errorText(e));
    });
  }, []);
  useEffect(() => {
    if (snap && snap.state !== "running" && !changed.current) {
      changed.current = true;
      onChanged();
    }
  }, [snap?.state]);
  async function verify() {
    setChecking(true);
    setError("");
    try {
      await invoke("operation_verify", {
        request: { operation_id: request.id, connection_id: connectionId },
      });
      if (alive.current) {
        setVerified(true);
        onChanged();
      }
    } catch (e) {
      if (alive.current) setError(errorText(e));
    } finally {
      if (alive.current) setChecking(false);
    }
  }
  const running = (!snap && !error) || snap?.state === "running";
  const rows: D[] = snap?.result?.results || snap?.items || [];
  return (
    <Dialog
      open={open}
      onOpenChange={(v) => {
        if (!v) onClose();
      }}
    >
      <DialogContent className="git-update-dialog">
        <DialogHeader>
          <DialogTitle>获取最新状态</DialogTitle>
          <DialogDescription>
            {request.path.split("/").pop()}
            {request.remote ? ` · ${request.remote}` : ""}
          </DialogDescription>
        </DialogHeader>
        <div className="dialog-body">
          <OperationBar operation={snap} />
          {rows.map((r) => (
            <div className="repo-fetch-row" key={r.path}>
              <strong>{r.name || r.path.split("/").pop()}</strong>
              <span>{r.message || "等待获取"}</span>
              {r.code === "auth" && (
                <Button
                  size="sm"
                  variant="ghost"
                  onClick={() => onAuth(r.url || "")}
                >
                  管理认证
                </Button>
              )}
            </div>
          ))}
          {(error || pollError) && error !== snap?.error && (
            <InlineAlert>{error || pollError}</InlineAlert>
          )}
          {verified && <p role="status">已核实当前仓库状态，未重新获取。</p>}
          <p className="hint">仅获取远端分支信息，不更新工作文件。</p>
        </div>
        <Confirm
          open={releaseConfirm}
          error={error}
          title="释放获取占用？"
          action="确认已停止并释放"
          destructive
          onCancel={() => setReleaseConfirm(false)}
          onConfirm={() => {
            void invoke("operation_release", {
              request: {
                operation_id: request.id,
                connection_id: connectionId,
                confirmed: true,
              },
            })
              .then(() => {
                setReleased(true);
                setReleaseConfirm(false);
                setError("");
                onChanged();
              })
              .catch((e) => setError(errorText(e)));
          }}
        >
          确认服务器上的获取操作已停止后再释放。不会重新执行获取。
        </Confirm>
        <DialogFooter>
          {snap?.remote_lease && !running && !released && (
            <Button variant="outline" onClick={() => setReleaseConfirm(true)}>
              释放占用…
            </Button>
          )}
          {snap &&
            !["running", "unknown"].includes(snap.state) &&
            rows.some((r) => r.state === "failed") && (
              <Button
                onClick={() =>
                  onRetry(
                    rows.filter((r) => r.state === "failed").map((r) => r.path),
                  )
                }
              >
                仅重试失败项
              </Button>
            )}
          {snap?.state === "unknown" && (
            <Button disabled={checking} onClick={() => void verify()}>
              {checking ? "核实中…" : "核实原操作"}
            </Button>
          )}
          <Button variant="outline" onClick={onClose}>
            {running ? "在后台继续" : "完成"}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
