import { useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import {
  GitBranch,
  Plus,
  Check,
  MoreHorizontal,
  RefreshCw,
  Trash2,
} from "lucide-react";
import { DropdownMenu } from "radix-ui";
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
import { OperationBar } from "./common/Feedback";
import { useOperation } from "@/lib/useOperation";
import { errorText } from "./CodeupAuth";
type Data = Record<string, any>;
type Branch = {
  name: string;
  oid: string;
  upstream: string;
  merged: boolean;
  reason: string;
  deletable: boolean;
};
type View = "select" | "create" | "delete" | "clean";
export function BranchDialog({
  open,
  onClose,
  path,
  connectionId,
  onOperation,
  operationId,
  onChanged,
}: {
  open: boolean;
  onClose: () => void;
  path: string;
  connectionId: string;
  onOperation?: (v: Data) => void;
  operationId?: string;
  onChanged?: () => Promise<void>;
}) {
  const [repos, setRepos] = useState<Data[]>([]),
    [repo, setRepo] = useState(path),
    [info, setInfo] = useState<Data>({}),
    [view, setView] = useState<View>("select");
  const [query, setQuery] = useState(""),
    [name, setName] = useState(""),
    [start, setStart] = useState(""),
    [selected, setSelected] = useState<string[]>([]),
    [target, setTarget] = useState<Branch>();
  const [busy, setBusy] = useState(false),
    [error, setError] = useState(""),
    [message, setMessage] = useState(""),
    [remoteDelete, setRemoteDelete] = useState(false),
    [fetchId, setFetchId] = useState<string>();
  const generation = useRef(0),
    alive = useRef(true),
    [highlight, setHighlight] = useState(0);
  const operation = useOperation(
    "branch_fetch",
    repo,
    connectionId,
    open || !!fetchId,
    fetchId,
  );
  const fetching = operation.snap?.state === "running";
  const refreshedOperation = useRef("");
  useEffect(() => {
    const snap = operation.snap;
    if (
      !open ||
      busy ||
      !snap?.id ||
      snap.state !== "done" ||
      snap.path !== repo ||
      snap.connection_id !== connectionId ||
      refreshedOperation.current === snap.id
    )
      return;
    refreshedOperation.current = snap.id;
    void load(repo, info.base);
  }, [
    open,
    busy,
    operation.snap?.id,
    operation.snap?.state,
    repo,
    connectionId,
  ]);
  useEffect(() => {
    if (operation.snap) onOperation?.(operation.snap);
  }, [operation.snap]);
  useEffect(
    () => () => {
      alive.current = false;
      generation.current++;
    },
    [],
  );
  async function load(p = repo, base?: string, fetch = false) {
    const id = ++generation.current;
    setBusy(true);
    setError("");
    setMessage("");
    try {
      const operation_id = fetch ? crypto.randomUUID() : undefined;
      setFetchId(operation_id);
      const data = await invoke<Data>(fetch ? "branch_fetch" : "branches", {
        request: { path: p, connection_id: connectionId, base, operation_id },
      });
      if (alive.current && id === generation.current) {
        setInfo(data);
        setStart(data.current);
        setSelected([]);
        setHighlight(0);
      }
    } catch (e) {
      if (alive.current && id === generation.current) setError(errorText(e));
    } finally {
      if (alive.current && id === generation.current) setBusy(false);
    }
  }
  useEffect(() => {
    if (!open) return;
    alive.current = true;
    setView("select");
    setQuery("");
    setInfo({});
    setFetchId(operationId);
    const id = ++generation.current;
    setBusy(true);
    setError("");
    void invoke<Data>("git_update_plan", {
      request: { path, connection_id: connectionId },
    })
      .then(async (p) => {
        if (!alive.current || id !== generation.current) return;
        const list = p.repositories || [];
        setRepos(list);
        const chosen = list.some((r: Data) => r.path === path)
          ? path
          : list[0]?.path;
        if (!chosen) throw Error("此目录没有 Git 仓库，请先打开或克隆仓库");
        setRepo(chosen);
        await load(chosen);
      })
      .catch((e) => {
        if (alive.current && id === generation.current) {
          setError(errorText(e));
          setBusy(false);
        }
      });
    return () => {
      generation.current++;
    };
  }, [open, path, connectionId]);
  async function change(action: string, extra: Data = {}) {
    if (busy || fetching) return;
    const id = ++generation.current;
    setBusy(true);
    setFetchId(undefined);
    setError("");
    setMessage("");
    try {
      let result = await invoke<Data>("branch_change", {
        request: {
          path: repo,
          connection_id: connectionId,
          expected_current: info.current,
          action,
          ...extra,
        },
      });
      if (!alive.current || id !== generation.current) return;
      let remoteRemoved = false;
      if (action === "delete_remote") {
        remoteRemoved = true;
        setMessage("远端分支已删除，正在删除本地分支…");
        try {
          result = await invoke<Data>("branch_change", {
            request: {
              path: repo,
              connection_id: connectionId,
              action: "delete",
              ...extra,
            },
          });
        } catch (e) {
          setError(`远端已删除，本地未删除：${errorText(e)}`);
          setRemoteDelete(false);
          return;
        }
      }
      if (!alive.current || id !== generation.current) return;
      setInfo(result);
      setView("select");
      setQuery("");
      setSelected([]);
      setTarget(undefined);
      const failures = (result.results || []).filter((r: Data) => !r.ok);
      if (failures.length)
        setError(
          failures.map((r: Data) => `${r.name}：${r.message}`).join("；"),
        );
      else
        setMessage(
          action === "clean"
            ? `已清理 ${extra.selections.length} 个本地分支`
            : action === "delete" || remoteRemoved
              ? "已删除分支"
              : `已切换到 ${result.current}`,
        );
      try {
        await onChanged?.();
      } catch {
        setError("分支操作已完成，文件列表刷新失败，请刷新目录");
      }
    } catch (e) {
      if (alive.current && id === generation.current) setError(errorText(e));
    } finally {
      if (alive.current && id === generation.current) setBusy(false);
    }
  }
  const details: Branch[] = info.details || [];
  const local = details
    .filter((b) => b.name.toLowerCase().includes(query.toLowerCase()))
    .sort(
      (a, b) =>
        Number(b.name === info.current) - Number(a.name === info.current),
    );
  const remote: string[] = (info.remote_branches || []).filter((b: string) =>
    b.toLowerCase().includes(query.toLowerCase()),
  );
  const options = [
    ...local
      .filter((b) => b.name !== info.current)
      .map((b) => ({ name: b.name, remote: false })),
    ...remote.map((b) => ({ name: b, remote: true })),
  ];
  function switchTo(n: string, isRemote = false) {
    if (info.dirty) {
      setError("仓库有未提交改动，暂不能切换");
      return;
    }
    if (isRemote) {
      const slash = n.indexOf("/"),
        remoteName = n.slice(0, slash),
        branch = n.slice(slash + 1);
      const existing = details.find((b) => b.upstream === n);
      if (existing) return void change("switch", { branch: existing.name });
      if (details.some((b) => b.name === branch)) {
        setError(
          `本地 ${branch} 跟踪其他分支，请选择本地分支或使用不同名称创建`,
        );
        return;
      }
      void change("track", { branch, remote: `${remoteName}/${branch}` });
    } else void change("switch", { branch: n });
  }
  function to(v: View) {
    setView(v);
    setError("");
    setMessage("");
    setRemoteDelete(false);
    setSelected([]);
  }
  const menu = (b?: Branch) => (
    <DropdownMenu.Root>
      <DropdownMenu.Trigger asChild>
        <Button
          variant="ghost"
          size="icon"
          aria-label={b ? `${b.name} 的操作` : "分支操作"}
          disabled={busy || fetching}
        >
          <MoreHorizontal size={16} />
        </Button>
      </DropdownMenu.Trigger>
      <DropdownMenu.Portal>
        <DropdownMenu.Content
          className="z-50 min-w-40 rounded-md border bg-card p-1 shadow-md"
          sideOffset={4}
        >
          {b ? (
            <DropdownMenu.Item
              disabled={!b.deletable}
              className="branch-menu-item"
              onSelect={() => {
                setTarget(b);
                to("delete");
              }}
            >
              <Trash2 size={14} />
              删除分支{b.reason ? ` · ${b.reason}` : ""}
            </DropdownMenu.Item>
          ) : (
            <DropdownMenu.Item
              className="branch-menu-item"
              onSelect={() => to("clean")}
            >
              清理已合并分支…
            </DropdownMenu.Item>
          )}
        </DropdownMenu.Content>
      </DropdownMenu.Portal>
    </DropdownMenu.Root>
  );
  return (
    <Dialog
      open={open}
      onOpenChange={(v) => {
        if (!v && (!busy || !!fetchId || fetching)) onClose();
      }}
    >
      <DialogContent
        className="branch-dialog"
        onEscapeKeyDown={(e) => {
          if (busy && !fetchId && !fetching) e.preventDefault();
        }}
      >
        <DialogHeader>
          <DialogTitle>
            {view === "select"
              ? "切换分支"
              : view === "create"
                ? "新建分支"
                : view === "delete"
                  ? "删除分支"
                  : "清理已合并分支"}
          </DialogTitle>
          <DialogDescription>
            {repos.length > 1 ? "选择要操作的仓库" : repo.split("/").pop()}
          </DialogDescription>
        </DialogHeader>
        {repos.length > 1 && (
          <Select
            value={repo}
            disabled={busy || fetching}
            onValueChange={(p) => {
              setRepo(p);
              to("select");
              void load(p);
            }}
          >
            <SelectTrigger aria-label="选择仓库">
              <SelectValue />
            </SelectTrigger>
            <SelectContent>
              {repos.map((r) => (
                <SelectItem key={r.path} value={r.path}>
                  {r.name || r.path.split("/").pop()}
                </SelectItem>
              ))}
            </SelectContent>
          </Select>
        )}
        {view === "select" && (
          <>
            <div className="branch-search">
              <Input
                autoFocus
                aria-label="搜索分支"
                placeholder="搜索分支…"
                value={query}
                disabled={busy || fetching}
                onChange={(e) => {
                  setQuery(e.target.value);
                  setHighlight(0);
                }}
                onKeyDown={(e) => {
                  if (e.key === "ArrowDown") {
                    e.preventDefault();
                    setHighlight((i) => Math.min(i + 1, options.length - 1));
                  }
                  if (e.key === "ArrowUp") {
                    e.preventDefault();
                    setHighlight((i) => Math.max(i - 1, 0));
                  }
                  if (e.key === "Enter" && options[highlight]) {
                    e.preventDefault();
                    const b = options[highlight];
                    switchTo(b.name, b.remote);
                  }
                }}
              />
              <Button
                variant="ghost"
                size="icon"
                aria-label="刷新分支列表"
                disabled={busy || fetching}
                onClick={() => void load(repo, undefined, true)}
              >
                <RefreshCw size={16} />
              </Button>
              {menu()}
            </div>
            <Button
              className="branch-create"
              variant="ghost"
              disabled={busy || !info.current}
              onClick={() => {
                setName("");
                setStart(info.current);
                to("create");
              }}
            >
              <Plus size={15} />
              创建分支…
            </Button>
            <div className="branch-picker">
              <div className="branch-group">本地分支</div>
              {local.map((b) => (
                <div
                  className={`branch-row ${b.name === info.current ? "current" : ""} ${options[highlight]?.name === b.name ? "focused" : ""}`}
                  key={b.name}
                >
                  <Button
                    variant="ghost"
                    disabled={busy || b.name === info.current}
                    onClick={() => switchTo(b.name)}
                  >
                    {b.name === info.current ? (
                      <Check size={15} />
                    ) : (
                      <GitBranch size={15} />
                    )}
                    <span className="branch-name">{b.name}</span>
                    <small>
                      {b.name === info.current ? "当前" : b.upstream}
                    </small>
                  </Button>
                  {menu(b)}
                </div>
              ))}
              <div className="branch-group">远端分支</div>
              {remote.map((b) => (
                <div
                  key={b}
                  className={`branch-row ${options[highlight]?.name === b ? "focused" : ""}`}
                >
                  <Button
                    variant="ghost"
                    disabled={busy || fetching}
                    onClick={() => switchTo(b, true)}
                  >
                    <GitBranch size={15} />
                    <span className="branch-name">{b}</span>
                  </Button>
                </div>
              ))}
              {!local.length && !remote.length && !busy && (
                <p className="hint">没有匹配的分支</p>
              )}
            </div>
          </>
        )}
        {view === "create" && (
          <>
            <Label htmlFor="new-branch">分支名称</Label>
            <Input
              id="new-branch"
              autoFocus
              value={name}
              disabled={busy || fetching}
              onChange={(e) => setName(e.target.value)}
              placeholder="feature/login-dialog"
            />
            <Label>创建起点</Label>
            <Select
              value={start}
              disabled={busy || fetching}
              onValueChange={setStart}
            >
              <SelectTrigger aria-label="创建起点">
                <SelectValue />
              </SelectTrigger>
              <SelectContent>
                {details.map((b) => (
                  <SelectItem key={b.name} value={b.name}>
                    {b.name}
                    {b.name === info.current ? " · 当前分支" : ""}
                  </SelectItem>
                ))}
              </SelectContent>
            </Select>
            <p className="hint">创建后切换到新分支。</p>
          </>
        )}
        {view === "delete" && target && (
          <>
            <div className="branch-meta">
              <span>分支</span>
              <strong>{target.name}</strong>
              <span>位置</span>
              <span>本地</span>
              <span>合并状态</span>
              <span>已合并到 {info.base}</span>
            </div>
            <p>删除本地分支，保留工作文件和历史提交。</p>
            {(info.remote_branches || []).includes(`origin/${target.name}`) && (
              <>
                <label className="branch-check">
                  <input
                    type="checkbox"
                    checked={remoteDelete}
                    disabled={busy || fetching}
                    onChange={(e) => setRemoteDelete(e.target.checked)}
                  />
                  同时删除 origin 上的同名分支
                </label>
                {remoteDelete && (
                  <p className="branch-warning">
                    将删除 origin/{target.name}。其他远端不受影响。
                  </p>
                )}
              </>
            )}
          </>
        )}
        {view === "clean" && (
          <>
            <div className="branch-base">
              <Label>合并目标</Label>
              <Select
                value={info.base}
                disabled={busy || fetching}
                onValueChange={(base) => void load(repo, base)}
              >
                <SelectTrigger aria-label="合并目标">
                  <SelectValue />
                </SelectTrigger>
                <SelectContent>
                  {details.map((b) => (
                    <SelectItem key={b.name} value={b.name}>
                      {b.name}
                      {b.name === info.default_branch ? " · 默认分支" : ""}
                    </SelectItem>
                  ))}
                </SelectContent>
              </Select>
            </div>
            <div className="branch-picker">
              {details.map((b) => (
                <label
                  key={b.name}
                  className={`branch-check ${!b.deletable ? "muted" : ""}`}
                >
                  <input
                    type="checkbox"
                    checked={selected.includes(b.name)}
                    disabled={busy || !b.deletable}
                    onChange={(e) =>
                      setSelected((s) =>
                        e.target.checked
                          ? [...s, b.name]
                          : s.filter((n) => n !== b.name),
                      )
                    }
                  />
                  <span className="branch-name">{b.name}</span>
                  <small>{b.reason || "已合并"}</small>
                </label>
              ))}
            </div>
            <p className="hint">仅清理本地分支。</p>
          </>
        )}
        {operation.snap &&
          !error &&
          (operation.snap.state === "running" ||
            (fetchId &&
              operation.snap.id === fetchId &&
              ["failed", "unknown"].includes(operation.snap.state))) && (
            <OperationBar operation={operation.snap} />
          )}
        {busy && operation.snap?.state !== "running" && (
          <p role="status" className="hint">
            正在处理…
          </p>
        )}
        {message && (
          <p role="status" className="branch-success">
            {message}
          </p>
        )}
        {error && (
          <div role="alert" className="branch-warning">
            {error}
            {info.dirty && (
              <ul className="branch-dirty-files">
                {(info.changed_files || []).map((f: string) => (
                  <li key={f}>{f}</li>
                ))}
              </ul>
            )}
            <Button
              variant="ghost"
              size="sm"
              disabled={busy || fetching}
              onClick={() => {
                to("select");
                void load(repo, undefined, error.includes("远端"));
              }}
            >
              重新检查
            </Button>
          </div>
        )}
        <DialogFooter>
          {view === "select" ? (
            <>
              <span className="branch-keyboard">↑↓ 选择 · Enter 切换</span>
              <Button
                variant="outline"
                disabled={busy && !fetchId && !fetching}
                onClick={onClose}
              >
                {(busy && fetchId) || fetching ? "在后台继续" : "关闭"}
              </Button>
            </>
          ) : (
            <>
              <Button
                variant="outline"
                disabled={busy || fetching}
                onClick={() => to("select")}
              >
                取消
              </Button>
              {view === "create" ? (
                <Button
                  disabled={busy || !name.trim() || !start}
                  onClick={() =>
                    void change("create", {
                      branch: name.trim(),
                      start_point: start,
                    })
                  }
                >
                  创建并切换
                </Button>
              ) : (
                <Button
                  variant="destructive"
                  disabled={busy || (view === "clean" && !selected.length)}
                  onClick={() =>
                    void change(
                      view === "clean"
                        ? "clean"
                        : remoteDelete
                          ? "delete_remote"
                          : "delete",
                      {
                        base: info.base,
                        base_oid: info.base_oid,
                        remote: "origin",
                        selections: details
                          .filter((b) =>
                            view === "clean"
                              ? selected.includes(b.name)
                              : b.name === target?.name,
                          )
                          .map(({ name, oid }) => ({ name, oid })),
                      },
                    )
                  }
                >
                  {view === "clean"
                    ? `清理 ${selected.length} 个分支`
                    : "删除分支"}
                </Button>
              )}
            </>
          )}
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
