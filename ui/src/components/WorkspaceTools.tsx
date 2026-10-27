import { time } from "@/lib/format";
import { latestNavigation } from "@/lib/navigation";
import { OperationBar } from "./common/Feedback";
import { useOperation } from "@/lib/useOperation";
import { useEffect, useRef, useState, type ReactNode } from "react";
import { invoke } from "@tauri-apps/api/core";
import { DropdownMenu, ContextMenu } from "radix-ui";
import {
  MoreHorizontal,
  GitBranch,
  Trash2,
  Pencil,
  FolderInput,
  RefreshCw,
  Folder,
} from "lucide-react";
import { Button } from "./ui/button";
import { Input } from "./ui/input";
import { Label } from "./ui/label";
import { Badge } from "./ui/badge";
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
import { toast } from "sonner";
import { errorText } from "./CodeupAuth";
type Data = Record<string, any>;
export function FileActions({
  entry,
  onAction,
  disabled,
}: {
  entry: Data;
  onAction: (action: string, entry: Data) => void;
  disabled: boolean;
}) {
  return (
    <DropdownMenu.Root>
      <DropdownMenu.Trigger asChild>
        <Button
          variant="ghost"
          size="icon"
          aria-label={`${entry.name} 的操作`}
          disabled={disabled}
        >
          <MoreHorizontal size={16} />
        </Button>
      </DropdownMenu.Trigger>
      <DropdownMenu.Portal>
        <DropdownMenu.Content
          className="z-50 min-w-36 rounded-md border bg-card p-1 shadow-md"
          sideOffset={4}
        >
          {(
            [
              ["rename", "重命名", Pencil],
              ["move", "移动到…", FolderInput],
              ["trash", "移入回收区", Trash2],
            ] as const
          ).map(([id, label, Icon]) => (
            <DropdownMenu.Item
              key={String(id)}
              className="flex cursor-pointer items-center gap-2 rounded px-3 py-2 text-sm outline-none focus:bg-accent"
              onSelect={() => onAction(String(id), entry)}
            >
              <Icon size={14} />
              {String(label)}
            </DropdownMenu.Item>
          ))}
        </DropdownMenu.Content>
      </DropdownMenu.Portal>
    </DropdownMenu.Root>
  );
}
export function FileContextMenu({
  entry,
  disabled,
  onAction,
  children,
}: {
  entry: Data;
  disabled: boolean;
  onAction: (action: string, entry: Data) => void;
  children: ReactNode;
}) {
  return (
    <ContextMenu.Root>
      <ContextMenu.Trigger asChild disabled={disabled}>
        {children}
      </ContextMenu.Trigger>
      <ContextMenu.Portal>
        <ContextMenu.Content className="context-menu">
          {[
            ["rename", "重命名"],
            ["move", "移动到…"],
            ["trash", "移入回收区"],
          ].map(([id, label]) => (
            <ContextMenu.Item key={id} onSelect={() => onAction(id, entry)}>
              {label}
            </ContextMenu.Item>
          ))}
        </ContextMenu.Content>
      </ContextMenu.Portal>
    </ContextMenu.Root>
  );
}
export function FileOperation({
  operation,
  onClose,
  onDone,
  home,
  connectionId,
}: {
  connectionId: string;
  operation: Data | null;
  onClose: () => void;
  onDone: () => Promise<void>;
  home: string;
}) {
  const [name, setName] = useState(operation?.entry?.name || ""),
    [destination, setDestination] = useState(""),
    [folders, setFolders] = useState<Data | null>(null),
    [busy, setBusy] = useState(false),
    [error, setError] = useState("");
  const mounted = useRef(true);
  const navigation = useRef<ReturnType<typeof latestNavigation<Data>> | null>(
    null,
  );
  if (!navigation.current)
    navigation.current = latestNavigation<Data>(
      (path) =>
        invoke<Data>("directory_list", {
          request: { path, hidden: false, connection_id: connectionId },
        }),
      (r) => {
        setFolders(r);
        setDestination(r.path);
      },
      () => {},
    );
  useEffect(() => {
    mounted.current = true;
    return () => {
      mounted.current = false;
      navigation.current?.invalidate();
    };
  }, []);
  if (!operation) return null;
  const action = operation.action,
    entry = operation.entry;
  async function browse(path: string) {
    try {
      setError("");
      await navigation.current!(path);
    } catch (e) {
      if (mounted.current) setError(errorText(e));
    }
  }
  async function submit() {
    setBusy(true);
    setError("");
    try {
      const result = await invoke<Data>(
        action === "create" ? "file_create" : "file_mutate",
        {
          request: {
            path: entry.path,
            action,
            name,
            destination,
            connection_id: connectionId,
          },
        },
      );
      if (!mounted.current) return;
      toast.success(
        result.message || "操作已完成",
        result.id
          ? {
              action: {
                label: "撤销",
                onClick: () => {
                  void invoke("trash_restore", {
                    request: { id: result.id, connection_id: connectionId },
                  })
                    .then(async () => {
                      try {
                        await onDone();
                        toast.success("已恢复到原路径");
                      } catch {
                        toast.error("已恢复，但目录列表未刷新");
                      }
                    })
                    .catch((e) => toast.error(errorText(e)));
                },
              },
            }
          : undefined,
      );
      await onDone().catch(() => toast.error("已完成，列表未刷新"));
      if (mounted.current) onClose();
    } catch (e) {
      if (mounted.current) setError(errorText(e));
    } finally {
      if (mounted.current) setBusy(false);
    }
  }
  return (
    <Dialog
      open
      onOpenChange={(v) => {
        if (!v && !busy) onClose();
      }}
    >
      <DialogContent>
        <DialogHeader>
          <DialogTitle>
            {
              (
                {
                  create: "新建文件",
                  rename: "重命名",
                  move: "移动",
                  trash: "移入回收区",
                } as Record<string, string>
              )[action]
            }
          </DialogTitle>
          <DialogDescription className="break-all">
            {entry.path}
          </DialogDescription>
        </DialogHeader>
        {["rename", "create"].includes(action) && (
          <>
            <Label htmlFor="file-new-name">名称</Label>
            <Input
              id="file-new-name"
              value={name}
              onChange={(e) => setName(e.target.value)}
              placeholder={entry.name}
              disabled={busy}
            />
          </>
        )}
        {action === "move" && (
          <>
            <Label htmlFor="move-destination">移动到</Label>
            <div className="flex gap-2">
              <Input
                id="move-destination"
                value={destination}
                onChange={(e) => setDestination(e.target.value)}
                placeholder={home}
                disabled={busy}
              />
              <Button
                variant="outline"
                onClick={() => void browse(destination || home)}
                disabled={busy}
              >
                浏览
              </Button>
            </div>
            {folders && (
              <div className="max-h-52 overflow-auto rounded border p-2">
                <p className="hint break-all">{folders.path}</p>
                <Button
                  variant="ghost"
                  size="sm"
                  onClick={() =>
                    void browse(
                      folders.path.slice(0, folders.path.lastIndexOf("/")) ||
                        home,
                    )
                  }
                >
                  上一级
                </Button>
                {folders.entries
                  .filter((f: Data) =>
                    ["directory", "repository"].includes(f.kind),
                  )
                  .map((f: Data) => (
                    <Button
                      className="flex w-full justify-start"
                      variant="ghost"
                      key={f.path}
                      disabled={
                        f.path === entry.path ||
                        f.path.startsWith(entry.path + "/")
                      }
                      onClick={() => void browse(f.path)}
                    >
                      <Folder size={15} /> {f.name}
                    </Button>
                  ))}
              </div>
            )}
          </>
        )}
        {action === "trash" && (
          <p>
            将「{entry.name}」移入回收区
            {entry.kind !== "file" ? "（含全部内容）" : ""}，可恢复。
          </p>
        )}

        {error && (
          <p role="alert" className="text-destructive">
            {error}
          </p>
        )}
        <DialogFooter>
          <Button variant="outline" disabled={busy} onClick={onClose}>
            取消
          </Button>
          <Button
            variant={action === "trash" ? "destructive" : "default"}
            disabled={
              busy ||
              (["rename", "create"].includes(action) && !name.trim()) ||
              (action === "move" && !destination)
            }
            onClick={() => void submit()}
          >
            {busy
              ? "正在处理…"
              : (
                  {
                    trash: "移入回收区",
                    rename: "重命名",
                    create: "新建",
                    move: "移动",
                  } as Record<string, string>
                )[action] || "保存"}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
export function TrashDialog({
  open,
  onClose,
  onDone,
  connectionId,
}: {
  connectionId: string;
  open: boolean;
  onClose: () => void;
  onDone: () => Promise<void>;
}) {
  const [items, setItems] = useState<Data[]>([]),
    [busy, setBusy] = useState(false),
    [error, setError] = useState(""),
    [loaded, setLoaded] = useState(false);
  const mounted = useRef(true);
  useEffect(() => {
    mounted.current = true;
    return () => {
      mounted.current = false;
    };
  }, []);
  async function refresh() {
    setBusy(true);
    setError("");
    try {
      const r = await invoke<Data>("trash_list", {
        request: { connection_id: connectionId },
      });
      if (!mounted.current) return;
      setItems(r.items || []);
      setLoaded(true);
    } catch (e) {
      setError(errorText(e));
    } finally {
      setBusy(false);
    }
  }
  useEffect(() => {
    if (open) void refresh();
  }, [open]);
  async function restore(id: string) {
    setBusy(true);
    setError("");
    let restored = false;
    try {
      await invoke("trash_restore", {
        request: { id, connection_id: connectionId },
      });
      restored = true;
      if (!mounted.current) return;
      setItems((previous) => previous.filter((item) => item.id !== id));
      let refreshFailed = false;
      try {
        await onDone();
      } catch {
        refreshFailed = true;
      }
      if (!mounted.current) return;
      const r = await invoke<Data>("trash_list", {
        request: { connection_id: connectionId },
      });
      if (!mounted.current) return;
      setItems(r.items || []);
      if (refreshFailed) setError("文件已恢复，但目录列表未刷新");
    } catch (e) {
      if (mounted.current)
        setError(
          restored
            ? `文件已恢复，但回收区列表未刷新：${errorText(e)}`
            : errorText(e),
        );
    } finally {
      if (mounted.current) setBusy(false);
    }
  }
  return (
    <Dialog
      open={open}
      onOpenChange={(v) => {
        if (!v && !busy) onClose();
      }}
    >
      <DialogContent className="max-w-2xl">
        <DialogHeader>
          <DialogTitle>回收区</DialogTitle>
          <DialogDescription className="sr-only">
            恢复已移入回收区的文件
          </DialogDescription>
        </DialogHeader>
        <Button
          variant="outline"
          disabled={busy}
          onClick={() => void refresh()}
        >
          <RefreshCw size={15} />
          刷新
        </Button>
        {error && (
          <p role="alert" className="text-destructive">
            {error}
          </p>
        )}
        <div className="max-h-96 space-y-3 overflow-auto">
          {items.map((i) => (
            <div key={i.id} className="flex items-center gap-3 border-b py-3">
              <div className="min-w-0 flex-1">
                <strong>{i.name}</strong>
                <p className="hint break-all">{i.original}</p>
                <small>{i.deleted_at ? time(i.deleted_at) : ""}</small>
              </div>
              <Button
                variant="outline"
                disabled={busy}
                onClick={() => void restore(i.id)}
              >
                恢复
              </Button>
            </div>
          ))}
          {loaded && !items.length && <p className="hint">回收区为空。</p>}
        </div>
      </DialogContent>
    </Dialog>
  );
}
export function BranchDialog({
  open,
  onClose,
  path,
  connectionId,
  onOperation,
  onChanged,
  operationId,
}: {
  open: boolean;
  onClose: () => void;
  path: string;
  connectionId: string;
  onOperation?: (operation: Data) => void;
  operationId?: string;
  onChanged?: () => Promise<void>;
}) {
  const [fetchId, setFetchId] = useState<string>();
  useEffect(() => {
    if (open) setFetchId(operationId);
  }, [open, operationId, path, connectionId]);
  const operation = useOperation(
    "branch_fetch",
    path,
    connectionId,
    open || !!fetchId,
    fetchId,
  );
  useEffect(() => {
    if (operation.snap) onOperation?.(operation.snap);
  }, [operation.snap]);
  const [info, setInfo] = useState<Data>({}),
    [branch, setBranch] = useState(""),
    [remote, setRemote] = useState(""),
    [action, setAction] = useState("switch"),
    [busy, setBusy] = useState(false),
    [error, setError] = useState(""),
    [confirm, setConfirm] = useState(false),
    [message, setMessage] = useState("");
  async function load(op = "branches") {
    if (busy) return;
    setBusy(true);
    setError("");
    setMessage(op === "branch_fetch" ? "正在获取最新分支…" : "正在读取分支…");
    try {
      const operation_id =
        op === "branch_fetch" ? crypto.randomUUID() : undefined;
      if (operation_id) setFetchId(operation_id);
      const r = await invoke<Data>(op, {
        request: {
          path,
          connection_id: connectionId,
          ...(operation_id ? { operation_id } : {}),
        },
      });
      setInfo(r);
      setMessage(op === "branch_fetch" ? "已获取最新分支" : "");
    } catch (e) {
      setError(errorText(e));
      setMessage("");
    } finally {
      setBusy(false);
    }
  }
  useEffect(() => {
    if (open && !info.current && !error) void load();
  }, [open, path, connectionId]);
  async function change() {
    setBusy(true);
    setError("");
    try {
      const r = await invoke<Data>("branch_change", {
        request: { path, action, branch, remote },
      });
      setInfo(r);
      setConfirm(false);
      setMessage(
        action === "fast_forward"
          ? `已更新 ${r.current} 到 ${branch}`
          : `已切换到 ${r.current}`,
      );
      setBranch("");
      setRemote("");
      await onChanged?.();
    } catch (e) {
      setError(errorText(e));
    } finally {
      setBusy(false);
    }
  }
  return (
    <Dialog
      open={open}
      onOpenChange={(v) => {
        if (!v) onClose();
      }}
    >
      <DialogContent>
        <DialogHeader>
          <DialogTitle>
            <GitBranch className="inline" size={18} /> Git 分支
          </DialogTitle>
          <DialogDescription className="break-all">
            <span title={path}>{path.split("/").pop()}</span>
          </DialogDescription>
        </DialogHeader>
        <div className="flex flex-wrap gap-2">
          <Button variant="outline" disabled={busy} onClick={() => void load()}>
            刷新
          </Button>
          <Button
            variant="outline"
            disabled={busy}
            onClick={() => void load("branch_fetch")}
          >
            获取最新分支
          </Button>
        </div>
        {info.current && (
          <p>
            当前分支：<Badge>{info.current}</Badge>{" "}
            {info.dirty && (
              <Badge variant="destructive">有未提交改动，无法切换</Badge>
            )}
          </p>
        )}
        <Label htmlFor="branch-search">查找分支或输入新分支名</Label>
        <Input
          id="branch-search"
          value={branch}
          disabled={busy}
          onChange={(e) => {
            setBranch(e.target.value);
            setAction(
              (info.branches || []).includes(e.target.value)
                ? "switch"
                : "create",
            );
            setRemote("");
          }}
          placeholder="搜索分支…"
        />
        <div className="branch-list">
          {(info.branches || [])
            .filter((b: string) => b.includes(branch))
            .map((b: string) => (
              <Button
                key={b}
                variant={branch === b ? "secondary" : "ghost"}
                disabled={busy || b === info.current}
                onClick={() => {
                  setBranch(b);
                  setAction("switch");
                  setRemote("");
                }}
              >
                {b}
                {b === info.current ? " · 当前" : ""}
              </Button>
            ))}
          {(info.remote_branches || [])
            .filter((b: string) => b.includes(branch))
            .map((b: string) => (
              <Button
                key={b}
                variant="ghost"
                disabled={busy}
                onClick={() => {
                  setRemote(b);
                  setBranch(b.replace(/^origin\//, ""));
                  setAction("track");
                }}
              >
                {b}
              </Button>
            ))}
        </div>
        {branch && !(info.branches || []).includes(branch) && (
          <p className="hint">
            {action === "fast_forward"
              ? `将当前 ${info.current || "HEAD"}更新为 ${branch} 的代码，保留当前分支名`
              : action === "track"
                ? `从 ${remote} 新建本地分支`
                : `从当前 ${info.current || "HEAD"} 新建 ${branch}`}
          </p>
        )}

        {operation.snap &&
        ["running", "failed", "unknown"].includes(operation.snap.state) ? (
          <OperationBar operation={operation.snap} />
        ) : (
          message && <p role="status">{message}</p>
        )}
        {error && error !== operation.snap?.error && (
          <p role="alert" className="text-destructive">
            {error}
          </p>
        )}
        {confirm && (
          <p>
            确认在此仓库
            {action === "fast_forward"
              ? "将当前分支更新"
              : action === "switch"
                ? "切换"
                : "创建并切换"}
            到 <strong>{branch}</strong>？
          </p>
        )}
        <DialogFooter>
          {remote && action !== "fast_forward" && (
            <Button
              variant="outline"
              disabled={busy || info.dirty}
              onClick={() => {
                setAction("fast_forward");
                setBranch(remote);
                setConfirm(true);
              }}
            >
              使用 {remote} 更新当前分支
            </Button>
          )}
          <Button variant="outline" onClick={onClose}>
            {busy ? "在后台继续" : "关闭"}
          </Button>
          <Button
            disabled={
              busy ||
              !info.current ||
              info.dirty ||
              !branch ||
              (action === "track" && !remote)
            }
            onClick={() => void change()}
          >
            {busy
              ? "正在执行…"
              : `${action === "fast_forward" ? "确认更新到" : action === "switch" ? "切换到" : "新建并切换到"} ${branch || "分支"}`}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
