import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { DropdownMenu } from "radix-ui";
import {
  MoreHorizontal,
  GitBranch,
  Trash2,
  Pencil,
  FolderInput,
  RefreshCw,
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
          className="z-50 min-w-36 rounded-md border bg-white p-1 shadow-md"
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
              className="flex cursor-pointer items-center gap-2 rounded px-3 py-2 text-sm outline-none focus:bg-blue-50"
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
export function FileOperation({
  operation,
  onClose,
  onDone,
  home,
}: {
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
  if (!operation) return null;
  const action = operation.action,
    entry = operation.entry;
  async function browse(path: string) {
    try {
      setError("");
      const r = await invoke<Data>("directory_list", {
        request: { path, hidden: false },
      });
      setFolders(r);
      setDestination(r.path);
    } catch (e) {
      setError(errorText(e));
    }
  }
  async function submit() {
    setBusy(true);
    setError("");
    try {
      const result = await invoke<Data>(
        action === "create" ? "file_create" : "file_mutate",
        {
          request: { path: entry.path, action, name, destination },
        },
      );
      toast.success(result.message || "操作已完成");
      await onDone().catch(() =>
        toast.error(
          "操作已完成，但目录刷新失败；请重新连接后刷新，不要重复执行",
        ),
      );
      onClose();
    } catch (e) {
      setError(errorText(e));
    } finally {
      setBusy(false);
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
                  move: "移动文件或文件夹",
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
            <Label htmlFor="move-destination">目标文件夹（保留原名称）</Label>
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
                      onClick={() => void browse(f.path)}
                    >
                      📁 {f.name}
                    </Button>
                  ))}
              </div>
            )}
            <p className="hint">不会覆盖同名目标；不能移动到自身或子目录。</p>
          </>
        )}
        {action === "trash" && (
          <p>
            将此项移入当前账号的 CCBridge
            回收区，文件夹包含其全部内容。可在“回收区”恢复；不永久删除。
          </p>
        )}
        <p className="hint">
          进行中的任务、已注册项目根目录和当前工作区受到保护。
        </p>
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
              : action === "trash"
                ? "确认移入回收区"
                : "确认"}
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
}: {
  open: boolean;
  onClose: () => void;
  onDone: () => Promise<void>;
}) {
  const [items, setItems] = useState<Data[]>([]),
    [busy, setBusy] = useState(false),
    [error, setError] = useState(""),
    [loaded, setLoaded] = useState(false);
  async function refresh() {
    setBusy(true);
    setError("");
    try {
      const r = await invoke<Data>("trash_list", { request: {} });
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
    try {
      await invoke("trash_restore", { request: { id } });
      await onDone();
      const r = await invoke<Data>("trash_list", { request: {} });
      setItems(r.items || []);
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
        if (!v && !busy) onClose();
      }}
    >
      <DialogContent className="max-w-2xl">
        <DialogHeader>
          <DialogTitle>当前账号回收区</DialogTitle>
          <DialogDescription>
            恢复到原路径，不覆盖已有同名文件。这里不提供永久删除。
          </DialogDescription>
        </DialogHeader>
        <Button
          variant="outline"
          disabled={busy}
          onClick={() => void refresh()}
        >
          <RefreshCw size={15} />
          加载 / 刷新
        </Button>
        {error && (
          <p role="alert" className="text-destructive">
            {error}
          </p>
        )}
        <div className="max-h-96 space-y-3 overflow-auto">
          {items.map((i) => (
            <div
              key={i.id}
              className="flex items-center gap-3 rounded border p-3"
            >
              <div className="min-w-0 flex-1">
                <strong>{i.name}</strong>
                <p className="hint break-all">{i.original}</p>
              </div>
              <Button disabled={busy} onClick={() => void restore(i.id)}>
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
}: {
  open: boolean;
  onClose: () => void;
  path: string;
}) {
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
    setMessage(
      op === "branch_fetch"
        ? "正在获取 origin 引用，不合并代码…"
        : "正在读取分支…",
    );
    try {
      const r = await invoke<Data>(op, { request: { path } });
      setInfo(r);
      setMessage(op === "branch_fetch" ? "远端分支引用已更新" : "");
    } catch (e) {
      setError(errorText(e));
      setMessage("");
    } finally {
      setBusy(false);
    }
  }
  useEffect(() => {
    if (open) void load();
  }, [open, path]);
  async function change() {
    setBusy(true);
    setError("");
    try {
      const r = await invoke<Data>("branch_change", {
        request: { path, action, branch, remote },
      });
      setInfo(r);
      setConfirm(false);
      setMessage(`已切换到 ${r.current}`);
      setBranch("");
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
        if (!v && !busy) onClose();
      }}
    >
      <DialogContent>
        <DialogHeader>
          <DialogTitle>
            <GitBranch className="inline" size={18} /> Git 分支
          </DialogTitle>
          <DialogDescription className="break-all">
            {path} · 请先打开一个 Git 仓库根目录
          </DialogDescription>
        </DialogHeader>
        <div className="flex flex-wrap gap-2">
          <Button variant="outline" disabled={busy} onClick={() => void load()}>
            读取 / 刷新分支
          </Button>
          <Button
            variant="outline"
            disabled={busy}
            onClick={() => void load("branch_fetch")}
          >
            获取远端分支（fetch）
          </Button>
        </div>
        {info.current && (
          <p>
            当前分支：<Badge>{info.current}</Badge>{" "}
            {info.dirty && (
              <Badge variant="destructive">有未提交改动，禁止切换</Badge>
            )}
          </p>
        )}
        <Select
          value={action}
          onValueChange={(v) => {
            setAction(v);
            setConfirm(false);
            setBranch("");
          }}
          disabled={busy}
        >
          <SelectTrigger className="w-full">
            <SelectValue />
          </SelectTrigger>
          <SelectContent>
            <SelectItem value="switch">切换已有本地分支</SelectItem>
            <SelectItem value="create">从当前 HEAD 新建并切换</SelectItem>
            <SelectItem value="track">从远端分支建立跟踪分支</SelectItem>
          </SelectContent>
        </Select>
        {action === "switch" ? (
          <Select value={branch} onValueChange={setBranch} disabled={busy}>
            <SelectTrigger className="w-full">
              <SelectValue placeholder="选择本地分支" />
            </SelectTrigger>
            <SelectContent>
              {(info.branches || []).map((b: string) => (
                <SelectItem key={b} value={b}>
                  {b}
                </SelectItem>
              ))}
            </SelectContent>
          </Select>
        ) : (
          <>
            <Label htmlFor="branch-name">新本地分支名称</Label>
            <Input
              id="branch-name"
              value={branch}
              disabled={busy}
              onChange={(e) => setBranch(e.target.value)}
              placeholder="feature/example"
            />
            {action === "track" && (
              <Select value={remote} onValueChange={setRemote} disabled={busy}>
                <SelectTrigger className="w-full">
                  <SelectValue placeholder="选择远端分支" />
                </SelectTrigger>
                <SelectContent>
                  {(info.remote_branches || []).map((b: string) => (
                    <SelectItem key={b} value={b}>
                      {b}
                    </SelectItem>
                  ))}
                </SelectContent>
              </Select>
            )}
          </>
        )}
        <p className="hint">
          不自动
          stash、reset、合并或推送。切换会改变该仓库之后任务读取的代码；已有进行中任务会阻止操作。
        </p>
        {message && <p role="status">{message}</p>}
        {error && (
          <p role="alert" className="text-destructive">
            {error}
          </p>
        )}
        {confirm && (
          <p>
            确认在此仓库{action === "switch" ? "切换" : "创建并切换"}到{" "}
            <strong>{branch}</strong>？
          </p>
        )}
        <DialogFooter>
          <Button variant="outline" onClick={onClose} disabled={busy}>
            关闭
          </Button>
          <Button
            disabled={
              busy ||
              !info.current ||
              info.dirty ||
              !branch ||
              (action === "track" && !remote)
            }
            onClick={() => (confirm ? void change() : setConfirm(true))}
          >
            {busy ? "正在执行…" : confirm ? "确认执行" : "应用分支操作"}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
