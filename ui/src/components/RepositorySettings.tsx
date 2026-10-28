import { useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { Button } from "./ui/button";
import { Input } from "./ui/input";
import { Label } from "./ui/label";
import {
  Dialog,
  DialogContent,
  DialogHeader,
  DialogTitle,
  DialogDescription,
  DialogFooter,
} from "./ui/dialog";
import {
  Select,
  SelectTrigger,
  SelectValue,
  SelectContent,
  SelectItem,
} from "./ui/select";
import { InlineAlert } from "./common/Feedback";
import { errorText } from "./CodeupAuth";
import { authLabel } from "@/lib/codeupAuth";
type D = Record<string, any>;
export function RepositorySettings({
  repo,
  connectionId,
  kind,
  onClose,
  onChanged,
  onAuth,
  onFetch,
}: {
  repo: D;
  connectionId: string;
  kind: "remote" | "upstream";
  onClose: () => void;
  onChanged: () => void;
  onAuth: (url: string) => void;
  onFetch: (path: string, remote?: string) => void;
}) {
  const [remote, setRemote] = useState(
      repo.remote || repo.remotes?.[0]?.name || "",
    ),
    [refs, setRefs] = useState<D>({}),
    [target, setTarget] = useState(repo.upstream || ""),
    [confirm, setConfirm] = useState(false),
    [editing, setEditing] = useState(false),
    [url, setUrl] = useState(""),
    [busy, setBusy] = useState(false),
    [error, setError] = useState(""),
    [message, setMessage] = useState(""),
    [access, setAccess] = useState("");
  const [authConfig, setAuthConfig] = useState({
    loading: true,
    label: "读取中…",
  });
  const mounted = useRef(true);
  useEffect(
    () => () => {
      mounted.current = false;
    },
    [],
  );
  const selected = (repo.remotes || []).find((r: D) => r.name === remote);
  useEffect(() => {
    if (
      kind !== "remote" ||
      !selected?.url?.startsWith("https://codeup.aliyun.com/")
    )
      return;
    let alive = true;
    setAuthConfig({ loading: true, label: "读取中…" });
    void Promise.all([
      invoke<D>("git_auth_status", { request: { scope: "remote" } }),
      invoke<D>("git_vps_auth_status", {
        request: { connection_id: connectionId },
      }),
    ])
      .then(([app, vps]) => {
        if (alive)
          setAuthConfig({
            loading: false,
            label: authLabel("remote", app, vps),
          });
      })
      .catch(() => {
        if (alive) setAuthConfig({ loading: false, label: "读取失败" });
      });
    return () => {
      alive = false;
    };
  }, [kind, remote, repo.path, connectionId]);
  useEffect(() => {
    setEditing(false);
    setError("");
    setMessage("");
    setAccess("");
  }, [remote]);
  useEffect(() => {
    if (kind !== "upstream") return;
    let alive = true;
    setBusy(true);
    void invoke<D>("branches", {
      request: { path: repo.path, connection_id: connectionId },
    })
      .then((r) => {
        if (alive) setRefs(r);
      })
      .catch((e) => {
        if (alive) setError(errorText(e));
      })
      .finally(() => {
        if (alive) setBusy(false);
      });
    return () => {
      alive = false;
    };
  }, [repo.path, connectionId, kind]);
  async function apply() {
    if (busy) return;
    setBusy(true);
    setError("");
    try {
      await invoke("branch_change", {
        request: {
          path: repo.path,
          connection_id: connectionId,
          expected_current: repo.branch,
          expected_head: repo.head,
          action: kind === "upstream" ? "upstream" : "remote_url",
          ...(kind === "upstream"
            ? {
                target,
                target_head: refs.remote_details?.find(
                  (r: D) => r.name === target,
                )?.head,
              }
            : { remote, expected_url: selected?.url, url }),
        },
      });
      if (!mounted.current) return;
      onChanged();
      onClose();
    } catch (e) {
      if (mounted.current) setError(errorText(e));
    } finally {
      if (mounted.current) setBusy(false);
    }
  }
  async function check() {
    if (!selected) return;
    setBusy(true);
    setError("");
    setAccess("");
    try {
      await invoke("git_remote_access", {
        request: {
          path: repo.path,
          remote,
          expected_url: selected.url,
          connection_id: connectionId,
        },
      });
      if (mounted.current) setAccess("仓库读取已验证");
    } catch (e) {
      if (mounted.current) setError(errorText(e));
    } finally {
      if (mounted.current) setBusy(false);
    }
  }
  const codeup = selected?.url?.startsWith("https://codeup.aliyun.com/");
  return (
    <Dialog
      open
      onOpenChange={(v) => {
        if (!v && !busy) onClose();
      }}
    >
      <DialogContent className="repo-settings-dialog">
        <DialogHeader>
          <DialogTitle>
            {kind === "upstream" ? "设置上游" : "远端与认证"}
          </DialogTitle>
          <DialogDescription>
            {repo.name} · {repo.branch}
          </DialogDescription>
        </DialogHeader>
        {kind === "upstream" ? (
          <>
            {!confirm ? (
              <>
                <Label>当前分支</Label>
                <p className="repo-code">{repo.branch}</p>
                <Label>跟踪的远端分支</Label>
                <Select
                  value={target}
                  onValueChange={setTarget}
                  disabled={busy}
                >
                  <SelectTrigger aria-label="上游分支">
                    <SelectValue placeholder="选择已获取的远端分支" />
                  </SelectTrigger>
                  <SelectContent>
                    {(refs.remote_details || []).map((r: D) => (
                      <SelectItem key={r.name} value={r.name}>
                        {r.name}
                      </SelectItem>
                    ))}
                  </SelectContent>
                </Select>
                {!refs.remote_details?.length && !busy && (
                  <p className="hint">
                    先获取远端信息，再选择上游。
                    <Button variant="ghost" onClick={() => onFetch(repo.path)}>
                      获取最新状态
                    </Button>
                  </p>
                )}
              </>
            ) : (
              <div className="repo-fields">
                <span>当前分支</span>
                <strong>{repo.branch}</strong>
                <span>原上游</span>
                <span>{repo.upstream || "未设置"}</span>
                <span>新上游</span>
                <strong>{target}</strong>
              </div>
            )}
            <p className="hint">只修改跟踪关系，不切换分支、不更新文件。</p>
          </>
        ) : (
          <>
            <div className="repo-fields">
              <span>远端</span>
              <Select value={remote} onValueChange={setRemote} disabled={busy}>
                <SelectTrigger aria-label="远端">
                  <SelectValue placeholder="没有远端" />
                </SelectTrigger>
                <SelectContent>
                  {(repo.remotes || []).map((r: D) => (
                    <SelectItem key={r.name} value={r.name}>
                      {r.name}
                    </SelectItem>
                  ))}
                </SelectContent>
              </Select>
              <span>仓库地址</span>
              <span className="repo-url">{selected?.url || "未配置"}</span>
              <span>执行位置</span>
              <strong>当前 SSH 账号</strong>
            </div>
            {editing ? (
              <>
                <Label htmlFor="remote-url">新远端地址</Label>
                <Input
                  id="remote-url"
                  value={url}
                  disabled={busy}
                  onChange={(e) => setUrl(e.target.value)}
                  placeholder="https://…/repo.git"
                />
                <p className="hint">
                  后续获取和更新将使用此地址，保存后需重新验证访问权限。
                </p>
              </>
            ) : (
              <div className="repo-inline-actions">
                <Button
                  variant="outline"
                  disabled={busy || !selected}
                  onClick={() => void check()}
                >
                  检测仓库访问
                </Button>
                <Button
                  variant="ghost"
                  disabled={busy || !selected}
                  onClick={() => {
                    setEditing(true);
                    setUrl(selected.url);
                  }}
                >
                  修改地址…
                </Button>
              </div>
            )}
            {codeup ? (
              <div className="repo-auth">
                <span>Codeup 认证 · {authConfig.label}</span>
                <Button
                  variant="outline"
                  disabled={busy}
                  onClick={() => onAuth(selected.url)}
                >
                  管理 / 同步认证…
                </Button>
              </div>
            ) : (
              <p className="hint">
                此远端使用服务器已有 Git 认证；App 的认证管理目前仅支持 Codeup。
              </p>
            )}
            <p className="hint">读取通过不代表有推送权限。</p>
            {access && (
              <p className="repo-status-success" role="status">
                {access}
              </p>
            )}
            {message && <p role="status">{message}</p>}
          </>
        )}
        {error && <InlineAlert>{error}</InlineAlert>}
        <DialogFooter>
          <Button
            variant="outline"
            disabled={busy}
            onClick={() =>
              confirm
                ? setConfirm(false)
                : editing
                  ? setEditing(false)
                  : onClose()
            }
          >
            {confirm ? "返回" : editing ? "取消" : "关闭"}
          </Button>
          {kind === "upstream" ? (
            <Button
              disabled={
                busy ||
                !target ||
                !refs.remote_details?.some((r: D) => r.name === target)
              }
              onClick={() => (confirm ? void apply() : setConfirm(true))}
            >
              {busy ? "处理中…" : confirm ? "确定设置" : "下一步"}
            </Button>
          ) : !editing ? (
            <Button
              disabled={busy || !selected}
              onClick={() => onFetch(repo.path, remote)}
            >
              获取此远端
            </Button>
          ) : (
            editing && (
              <Button
                disabled={busy || !url.trim()}
                onClick={() => void apply()}
              >
                {busy ? "保存中…" : "确定修改地址"}
              </Button>
            )
          )}
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
