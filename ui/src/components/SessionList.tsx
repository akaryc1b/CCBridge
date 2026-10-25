import { searchHighlight } from "../lib/searchHighlight";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "./ui/select";
import { useState } from "react";
import { DropdownMenu as Menu } from "radix-ui";
import {
  Archive,
  ArchiveRestore,
  MoreHorizontal,
  Pencil,
  RefreshCw,
  Search,
  FolderGit2,
  FolderPlus,
  Plus,
  MessagesSquare,
} from "lucide-react";
import { toast } from "sonner";
import { Button } from "./ui/button";
import { Input } from "./ui/input";
import {
  Dialog,
  DialogContent,
  DialogTitle,
  DialogDescription,
} from "./ui/dialog";
import { projectSessionGroups } from "../lib/projectEntries";
import { type RecordValue as Data } from "../lib/conversation";
import { time } from "../lib/format";
import type { useSessions } from "../lib/useSessions";
export function SessionList({
  sessions,
  projects,
  selected,
  selectedProject,
  onSelect,
  onUpdateRunner,
  onAddProject,
  onNewSession,
}: {
  sessions: ReturnType<typeof useSessions>;
  projects: Data[];
  selected: string;
  selectedProject: string;
  onSelect: (id: string) => void;
  onUpdateRunner: () => void;
  onAddProject: () => void;
  onNewSession: (path: string) => void;
}) {
  const [diagnosticsOpen, setDiagnosticsOpen] = useState(false);
  const [rename, setRename] = useState<Data | null>(null),
    [name, setName] = useState(""),
    [saving, setSaving] = useState(false),
    [error, setError] = useState("");
  async function archive(s: Data) {
    try {
      const r = await sessions.edit(s, { archived: !s.archived });
      toast.success(s.archived ? "已恢复" : "已归档", {
        action: {
          label: "撤销",
          onClick: () => {
            sessions
              .edit({ ...s, revision: r.revision }, { archived: s.archived })
              .catch((e) => toast.error(e.message));
          },
        },
      });
    } catch (e: any) {
      toast.error(e.message);
    }
  }
  return (
    <>
      <div className="session-project-heading">
        <strong>项目</strong>
        <Menu.Root>
          <Menu.Trigger asChild>
            <Button variant="ghost" size="icon" aria-label="添加项目或会话">
              <Plus size={16} />
            </Button>
          </Menu.Trigger>
          <Menu.Portal>
            <Menu.Content className="context-menu" align="end">
              <Menu.Item disabled={sessions.offline} onSelect={onAddProject}>
                <FolderPlus size={15} />
                添加项目
              </Menu.Item>
              <Menu.Item onSelect={() => onNewSession("")}>
                <MessagesSquare size={15} />
                新建普通会话
              </Menu.Item>
            </Menu.Content>
          </Menu.Portal>
        </Menu.Root>
      </div>
      <div className="sessions-heading">
        <div className="session-search">
          <Search size={15} />
          <Input
            disabled={sessions.offline}
            aria-label="搜索会话"
            placeholder="搜索标题、输入或最终结果"
            value={sessions.query}
            onChange={(e) => sessions.setQuery(e.target.value)}
            maxLength={200}
          />
        </div>
        <Button
          size="icon"
          variant="ghost"
          aria-label="刷新会话"
          disabled={sessions.loading || sessions.offline}
          onClick={() => sessions.load()}
        >
          <RefreshCw size={15} />
        </Button>
      </div>
      <div className="session-tabs">
        {[false, true].map((a) => (
          <Button
            key={String(a)}
            variant={sessions.archive === a ? "secondary" : "ghost"}
            disabled={sessions.offline}
            onClick={() => sessions.setArchive(a)}
          >
            {a ? "已归档" : "最近"}
          </Button>
        ))}
      </div>
      <div className="px-3 pb-3">
        <Select
          disabled={sessions.offline}
          value={sessions.project || "__all__"}
          onValueChange={(v) => sessions.setProject(v === "__all__" ? "" : v)}
        >
          <SelectTrigger aria-label="筛选会话项目">
            <SelectValue placeholder="全部项目" />
          </SelectTrigger>
          <SelectContent>
            <SelectItem value="__all__">全部项目</SelectItem>
            {projects.map((p) => (
              <SelectItem key={p.path} value={p.path}>
                {p.name}
              </SelectItem>
            ))}
          </SelectContent>
        </Select>
      </div>
      {sessions.offline && <div className="notice">离线 · 上次加载的会话</div>}
      {!sessions.supported ? (
        <div className="empty">
          更新 Runner 后可搜索、命名和归档
          <Button variant="outline" onClick={onUpdateRunner}>
            更新 Runner
          </Button>
        </div>
      ) : (
        <>
          {sessions.error && (
            <div className="notice">
              {sessions.error.replace(/^SEARCH_EXPIRED:\s*/, "")}
              <Button variant="outline" onClick={() => sessions.load()}>
                刷新
              </Button>
            </div>
          )}
          {!!sessions.incomplete && (
            <div className="notice">
              结果不完整 · {sessions.incomplete} 条记录无法读取
              <Button variant="ghost" onClick={() => setDiagnosticsOpen(true)}>
                查看诊断
              </Button>
            </div>
          )}
          {projectSessionGroups(
            sessions.items,
            projects,
            sessions.query,
            sessions.archive,
            sessions.project,
          ).map((g) => (
            <details className="session-project" key={g.path} open>
              <summary
                className={`session-group-label ${selectedProject === g.path ? "active-project" : ""}`}
              >
                <FolderGit2 size={14} />
                <span className="session-group-name">{g.name}</span>
                {g.registered && (
                  <Button
                    variant="ghost"
                    size="icon"
                    aria-label={`在${g.name}新建会话`}
                    disabled={sessions.offline}
                    onClick={(e) => {
                      e.preventDefault();
                      e.stopPropagation();
                      onNewSession(g.path);
                    }}
                  >
                    <Plus size={14} />
                  </Button>
                )}
              </summary>
              {!g.sessions.length && (
                <Button
                  className="session-empty-add"
                  variant="ghost"
                  disabled={sessions.offline}
                  onClick={() => onNewSession(g.path)}
                >
                  <Plus size={14} />
                  新建会话
                </Button>
              )}
              {g.sessions.map((s) => (
                <div
                  className={`session-row ${selected === s.task_id ? "active" : ""}`}
                  key={s.session_id}
                >
                  <div className="session-select">
                    <button onClick={() => onSelect(s.task_id)}>
                      <strong>{s.title}</strong>
                      <span>
                        {time(s.updated_at || s.created_at)}
                        {s.legacy ? " · 旧记录" : ""}
                      </span>
                    </button>
                    {s.hits?.slice(0, 2).map((hit: Data, i: number) => (
                      <button
                        aria-label={`打开命中轮次 ${hit.field === "result" ? "最终结果" : hit.field === "prompt" ? "用户输入" : "标题"}`}
                        className="session-hit"
                        key={i}
                        onClick={(e) => {
                          e.stopPropagation();
                          onSelect(hit.task_id);
                        }}
                      >
                        {hit.field === "result"
                          ? "最终结果"
                          : hit.field === "prompt"
                            ? "用户输入"
                            : "标题"}{" "}
                        · {searchHighlight(hit.snippet, sessions.query)}
                      </button>
                    ))}
                  </div>
                  {!s.legacy && !sessions.offline && (
                    <Menu.Root>
                      <Menu.Trigger asChild>
                        <Button
                          variant="ghost"
                          size="icon"
                          aria-label={`管理会话 ${s.title}`}
                        >
                          <MoreHorizontal size={15} />
                        </Button>
                      </Menu.Trigger>
                      <Menu.Portal>
                        <Menu.Content className="context-menu" align="end">
                          <Menu.Item
                            onSelect={() => {
                              setRename(s);
                              setName(s.title);
                              setError("");
                            }}
                          >
                            <Pencil size={14} />
                            重命名
                          </Menu.Item>
                          <Menu.Item onSelect={() => archive(s)}>
                            {s.archived ? (
                              <ArchiveRestore size={14} />
                            ) : (
                              <Archive size={14} />
                            )}
                            {s.archived ? "恢复" : "归档"}
                          </Menu.Item>
                        </Menu.Content>
                      </Menu.Portal>
                    </Menu.Root>
                  )}
                </div>
              ))}
            </details>
          ))}
          {!sessions.items.length &&
            !sessions.loading &&
            !sessions.error &&
            (!!sessions.query || sessions.archive || !projects.length) && (
              <p className="empty">
                {sessions.query
                  ? "没有匹配的会话"
                  : sessions.archive
                    ? "没有已归档会话"
                    : "还没有会话"}
              </p>
            )}
          {sessions.loading && <p className="empty">加载会话…</p>}
          {sessions.cursor && (
            <Button
              variant="ghost"
              disabled={sessions.loading || sessions.offline}
              onClick={() => sessions.load(sessions.cursor!)}
            >
              加载更多
            </Button>
          )}
        </>
      )}
      <Dialog open={diagnosticsOpen} onOpenChange={setDiagnosticsOpen}>
        <DialogContent>
          <DialogTitle>会话读取诊断</DialogTitle>
          <DialogDescription>
            {sessions.incomplete} 条记录未包含在结果中
            {sessions.incomplete > sessions.diagnostics.length
              ? `，显示前 ${sessions.diagnostics.length} 条诊断`
              : ""}
          </DialogDescription>
          <pre className="max-h-80 overflow-auto whitespace-pre-wrap text-xs">
            {JSON.stringify(sessions.diagnostics, null, 2)}
          </pre>
        </DialogContent>
      </Dialog>
      <Dialog
        open={!!rename}
        onOpenChange={(v) => {
          if (!saving && !v) setRename(null);
        }}
      >
        <DialogContent>
          <DialogTitle>重命名会话</DialogTitle>
          <DialogDescription className="sr-only">
            修改当前会话的显示名称
          </DialogDescription>
          <label>
            会话名称
            <Input
              autoFocus
              value={name}
              onChange={(e) =>
                setName([...e.target.value].slice(0, 80).join(""))
              }
            />
          </label>
          {error && (
            <div role="alert" className="notice">
              {error.replace(/^SESSION_CONFLICT:\s*/, "")}
              {error.includes("CONFLICT") && (
                <Button
                  variant="outline"
                  onClick={async () => {
                    try {
                      const latest = await sessions.read(rename!);
                      setRename((current) => ({ ...current, ...latest }));
                      setError("已读取最新状态；你的输入已保留，确认后可保存");
                    } catch (e: any) {
                      setError(
                        (e.detail?.code === "session_conflict"
                          ? "SESSION_CONFLICT: "
                          : "") + e.message,
                      );
                    }
                  }}
                >
                  读取最新
                </Button>
              )}
            </div>
          )}
          <div className="dialog-actions">
            <Button
              variant="outline"
              disabled={saving}
              onClick={() => setRename(null)}
            >
              取消
            </Button>
            <Button
              disabled={saving || !name.trim() || /[\x00-\x1f\x7f]/.test(name)}
              onClick={async () => {
                setSaving(true);
                setError("");
                try {
                  await sessions.edit(rename!, { title: name.trim() });
                  setRename(null);
                } catch (e: any) {
                  setError(
                    (e.detail?.code === "session_conflict"
                      ? "SESSION_CONFLICT: "
                      : "") + e.message,
                  );
                } finally {
                  setSaving(false);
                }
              }}
            >
              {saving ? "保存中…" : "保存"}
            </Button>
          </div>
        </DialogContent>
      </Dialog>
    </>
  );
}
