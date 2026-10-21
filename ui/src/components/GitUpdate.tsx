import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { RefreshCw, Loader2 } from "lucide-react";
import { Button } from "./ui/button";
import { Badge } from "./ui/badge";
import {
  Dialog,
  DialogContent,
  DialogHeader,
  DialogTitle,
  DialogDescription,
  DialogFooter,
} from "./ui/dialog";
import { errorText } from "./CodeupAuth";
type D = Record<string, any>;
export function GitUpdate({
  open,
  path,
  onClose,
  onUpdated,
  onAuth,
}: {
  open: boolean;
  path: string;
  onClose: () => void;
  onUpdated: () => void;
  onAuth: (url: string) => void;
}) {
  const [plan, setPlan] = useState<D[]>([]),
    [results, setResults] = useState<D[]>([]),
    [busy, setBusy] = useState(false),
    [error, setError] = useState("");
  async function inspect() {
    setBusy(true);
    setError("");
    setResults([]);
    try {
      const r = await invoke<D>("git_update_plan", { request: { path } });
      setPlan(r.repositories || []);
    } catch (e) {
      setError(errorText(e));
      setPlan([]);
    } finally {
      setBusy(false);
    }
  }
  useEffect(() => {
    if (open) void inspect();
  }, [open, path]);
  async function update() {
    setBusy(true);
    setError("");
    setResults([]);
    try {
      const r = await invoke<D>("git_update", {
        request: { path, expected: plan },
      });
      setResults(r.results || []);
      onUpdated();
      const current = await invoke<D>("git_update_plan", { request: { path } });
      setPlan(current.repositories || []);
    } catch (e) {
      setError(errorText(e));
    } finally {
      setBusy(false);
    }
  }
  const authNeeded = results.some((r) =>
    /认证|Codeup|令牌|凭据/.test(r.message || ""),
  );
  return (
    <Dialog
      open={open}
      onOpenChange={(v) => {
        if (!v && !busy) onClose();
      }}
    >
      <DialogContent className="max-h-[85vh] max-w-2xl overflow-y-auto">
        <DialogHeader>
          <DialogTitle>
            <RefreshCw className="inline" size={18} /> Git 更新
          </DialogTitle>
          <DialogDescription className="break-all">{path}</DialogDescription>
        </DialogHeader>
        <p className="hint">
          获取各仓库当前跟踪分支并快进更新。不切分支、不推送；未提交改动、分叉或运行中的任务会阻止对应仓库。
        </p>
        {plan.map((r) => (
          <div key={r.path} className="rounded border p-3">
            <div className="flex items-center justify-between">
              <strong>{r.name}</strong>
              <Badge variant="outline">{r.branch || "未检出分支"}</Badge>
            </div>
            <p className="hint break-all">
              {r.upstream ? `跟踪 ${r.upstream}` : "未设置跟踪分支"} ·{" "}
              {r.head?.slice(0, 7)}
            </p>
            {r.blocked && <p className="text-destructive">{r.blocked}</p>}
          </div>
        ))}
        {busy && (
          <p role="status" className="flex items-center gap-2">
            <Loader2 size={16} className="animate-spin" />
            正在检查 / 更新，请勿重复提交…
          </p>
        )}
        {results.map((r) => (
          <div key={r.path} className="rounded border p-3">
            <strong>{r.path.split("/").pop()}</strong>
            <p
              role="status"
              className={r.state === "failed" ? "text-destructive" : ""}
            >
              {r.message}
            </p>
            {r.before && (
              <p className="hint">
                {r.before.slice(0, 7)} → {r.after?.slice(0, 7)}
              </p>
            )}
          </div>
        ))}
        {error && (
          <p role="alert" className="text-destructive">
            {error}
          </p>
        )}
        {authNeeded && (
          <Button
            variant="outline"
            onClick={() =>
              onAuth(
                plan.find((r) => r.url?.includes("codeup.aliyun.com"))?.url ||
                  "",
              )
            }
          >
            配置 Codeup 认证 / 同步到 VPS
          </Button>
        )}
        <DialogFooter>
          <Button variant="outline" disabled={busy} onClick={onClose}>
            关闭
          </Button>
          <Button
            variant="outline"
            disabled={busy}
            onClick={() => void inspect()}
          >
            刷新状态
          </Button>
          <Button
            disabled={busy || !plan.some((r) => !r.blocked)}
            onClick={() => void update()}
          >
            {busy ? "更新中…" : "获取并快进更新"}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
