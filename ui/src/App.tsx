import {
  normalizeTask,
  conversationTurns,
  latestSessions,
  displayModel,
} from "@/lib/conversation";
import { useEffect, useRef, useState, type ReactNode } from "react";
import { invoke, isTauri } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import {
  Activity,
  Server,
  FolderGit2,
  MessagesSquare,
  Cable,
  Download,
  Settings,
  ArrowUp,
  RefreshCw,
  Plus,
  ChevronRight,
  Folder,
  FileText,
  ShieldCheck,
  Terminal,
  Check,
  Loader2,
  Send,
  Square,
  Sparkles,
} from "lucide-react";
import ReactMarkdown from "react-markdown";
import remarkGfm from "remark-gfm";
import { Toaster, toast } from "sonner";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Badge } from "@/components/ui/badge";
import {
  Card,
  CardHeader,
  CardTitle,
  CardDescription,
  CardContent,
} from "@/components/ui/card";
import { Textarea } from "@/components/ui/textarea";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import { Switch } from "@/components/ui/switch";
import {
  Dialog,
  DialogContent,
  DialogHeader,
  DialogTitle,
  DialogDescription,
  DialogFooter,
} from "@/components/ui/dialog";
import { Progress } from "@/components/ui/progress";
type Data = Record<string, any>;
const pages = [
  ["overview", "概览", Activity],
  ["servers", "服务器", Server],
  ["projects", "项目与文件", FolderGit2],
  ["chat", "任务与对话", MessagesSquare],
  ["codex", "Codex 接入", Cable],
  ["updates", "应用更新", Download],
  ["settings", "设置", Settings],
] as const;
const statusLabels: Data = {
  queued: "排队中",
  running: "执行中",
  waiting_approval: "等待审批",
  completed: "已完成",
  failed: "失败",
  cancelled: "已取消",
  interrupted: "已中断",
};
const active = (t: Data) =>
  ["running", "queued", "waiting_approval"].includes(t.state);
function Field({
  label,
  children,
  hint,
}: {
  label: string;
  children: ReactNode;
  hint?: string;
}) {
  return (
    <div className="field">
      <Label>{label}</Label>
      {children}
      {hint && <p className="hint">{hint}</p>}
    </div>
  );
}
function Panel({
  title,
  description,
  children,
}: {
  title: string;
  description?: string;
  children: ReactNode;
}) {
  return (
    <Card>
      <CardHeader>
        <CardTitle>{title}</CardTitle>
        {description && <CardDescription>{description}</CardDescription>}
      </CardHeader>
      <CardContent>{children}</CardContent>
    </Card>
  );
}
function Markdown({ text }: { text: string }) {
  return (
    <div className="markdown">
      <ReactMarkdown
        remarkPlugins={[remarkGfm]}
        skipHtml
        urlTransform={(u) => (/^https?:\/\//i.test(u) ? u : "")}
      >
        {text}
      </ReactMarkdown>
    </div>
  );
}
export default function App() {
  const [page, setPage] = useState("overview"),
    [version, setVersion] = useState("0.5.0");
  const [profile, setProfile] = useState<Data>({
      host: "",
      port: 22,
      user: "",
      remember_password: false,
      model: "",
      allow_auto: false,
    }),
    [saved, setSaved] = useState<Data>({}),
    [connection, setConnection] = useState<Data>({ connected: false }),
    [password, setPassword] = useState("");
  const [busy, setBusy] = useState<Set<string>>(new Set()),
    [errors, setErrors] = useState<Data>({}),
    [serverEdit, setServerEdit] = useState(false),
    [autoConfirm, setAutoConfirm] = useState(false),
    [hostKey, setHostKey] = useState("");
  const [projects, setProjects] = useState<Data[]>([]),
    [listing, setListing] = useState<Data>({ entries: [] }),
    [path, setPath] = useState(""),
    [preview, setPreview] = useState(""),
    [cloneOpen, setCloneOpen] = useState(false),
    [cloneUrls, setCloneUrls] = useState(""),
    [localAuth, setLocalAuth] = useState(false),
    [folderName, setFolderName] = useState(""),
    [projectName, setProjectName] = useState("");
  const [tasks, setTasks] = useState<Data[]>([]),
    [selected, setSelected] = useState(""),
    [details, setDetails] = useState<Data>({}),
    [draft, setDraft] = useState(""),
    [mode, setMode] = useState("automatic"),
    [modelPolicy, setModelPolicy] = useState("inherit"),
    [model, setModel] = useState(""),
    [newConversation, setNewConversation] = useState(false);
  const [update, setUpdate] = useState<Data>({}),
    [updatePhase, setUpdatePhase] = useState("idle"),
    [progress, setProgress] = useState(0),
    [integration, setIntegration] = useState("");
  const drafts = useRef<Record<string, string>>({});
  const submission = useRef<{ signature: string; id: string } | null>(null);
  const detailsRef = useRef(details);
  detailsRef.current = details;

  function switchConversation(id: string) {
    drafts.current[selected || "__new__"] = draft;
    setDraft(drafts.current[id || "__new__"] || "");
    setSelected(id);
    setNewConversation(!id);
    setModelPolicy("inherit");
    const previous = details[id] || tasks.find((t) => t.task_id === id);
    if (previous) setMode(previous.mode);
  }
  const inflight = useRef(new Set<string>()),
    selectedRef = useRef(selected);
  selectedRef.current = selected;
  const connected = connection.connected === true,
    task = details[selected] || tasks.find((t) => t.task_id === selected),
    runner = connection.probe || {};
  const timeline = conversationTurns(task, details);
  const currentProject =
    connection.profile?.active_remote_workspace ||
    profile.active_remote_workspace ||
    "";
  const request = async (
    command: string,
    args: Data = {},
    quiet = false,
  ): Promise<any> => {
    if (inflight.current.has(command)) return undefined;
    inflight.current.add(command);
    setBusy(new Set(inflight.current));
    try {
      const data = await invoke(command, { request: args });
      setErrors((e) => {
        const next = { ...e };
        delete next[command];
        return next;
      });
      return data;
    } catch (e) {
      const message =
        typeof e === "object" && e !== null && "message" in e
          ? String((e as Data).message)
          : String(e);
      setErrors((old) => ({ ...old, [command]: message }));
      if (!quiet) toast.error(message);
      throw e;
    } finally {
      inflight.current.delete(command);
      setBusy(new Set(inflight.current));
    }
  };
  const safe = (p: Promise<any>) => {
    void p.catch(() => {});
  };
  async function refreshConnection() {
    const c = await request("connection_status", {}, true);
    if (c) {
      setConnection(c);
      if (c.profile)
        setProfile((p) => ({
          ...p,
          allow_auto: c.profile.allow_auto,
          active_remote_workspace: c.profile.active_remote_workspace,
        }));
    }
    return c;
  }
  async function refreshTasks() {
    const r = await request("task_list", {}, true);
    if (r) setTasks((r.tasks || []).map(normalizeTask));
  }
  async function refreshProjects() {
    const r = await request("project_list", {}, true);
    if (r) setProjects(r.projects || []);
  }
  async function browse(p: string) {
    const r = await request("directory_list", { path: p, hidden: false });
    if (r) {
      setListing(r);
      setPath(r.path);
    }
  }
  async function fetchTask(id: string) {
    let currentId: string | undefined = id;
    const seen = new Set<string>();
    while (currentId && !seen.has(currentId)) {
      seen.add(currentId);
      const r = await request(
        "task_status",
        { task_id: currentId, cursor: 0 },
        true,
      );
      if (!r) break;
      const key = currentId;
      setDetails((d) => ({ ...d, [key]: r }));
      currentId =
        r.reply_to && !detailsRef.current[r.reply_to] ? r.reply_to : undefined;
    }
  }

  useEffect(() => {
    if (!isTauri()) {
      setErrors({
        runtime:
          "这是界面预览；请在 CCBridge App 内连接和执行，浏览器不会连接 SSH。",
      });
      return;
    }
    safe(
      (async () => {
        const s = await invoke<Data>("initial_state");
        setProfile({ ...profile, ...s.profile });
        setSaved(s.profiles || {});
        setVersion(s.version);
        await refreshConnection();
      })(),
    );
  }, []);
  useEffect(() => {
    if (!connected) return;
    safe(refreshTasks());
    safe(refreshProjects());
    const timer = setInterval(() => {
      if (document.hidden) return;
      safe(refreshConnection());
      safe(refreshTasks());
      if (selectedRef.current) safe(fetchTask(selectedRef.current));
    }, 5000);
    return () => clearInterval(timer);
  }, [connected]);
  useEffect(() => {
    if (selected && connected) safe(fetchTask(selected));
  }, [selected, connected]);
  useEffect(() => {
    if (!isTauri()) return;
    let cleanup: (() => void) | undefined;
    listen<Data>("update-progress", (e) =>
      setProgress(
        e.payload.total
          ? Math.round((e.payload.downloaded / e.payload.total) * 100)
          : 0,
      ),
    )
      .then((f) => (cleanup = f))
      .catch((e) => toast.error(String(e)));
    return () => cleanup?.();
  }, []);
  useEffect(() => {
    if (!isTauri()) return;
    let cleanup: (() => void) | undefined;
    let disposed = false;
    listen<Data>("backend-event", (e) => {
      if (e.payload.response?.ok === false)
        setErrors((x) => ({ ...x, backend: e.payload.response.error }));
    }).then((f) => {
      if (disposed) f();
      else cleanup = f;
    });
    return () => {
      disposed = true;
      cleanup?.();
    };
  }, []);
  const patch = (key: string, value: any) =>
    setProfile((p) =>
      ["host", "port", "user"].includes(key) && p[key] !== value
        ? {
            ...p,
            [key]: value,
            fingerprint: "",
            bindings: [],
            allow_auto: false,
            runner: "",
            claude: "",
            workspace: "",
            active_remote_workspace: "",
            local_repository: "",
            search_root: "",
            model: "",
            remember_password: false,
          }
        : { ...p, [key]: value },
    );
  async function connect() {
    try {
      const c = await request("connect_ssh", { profile, password });
      if (c) {
        setPassword("");
        setConnection(c);
        setProfile(c.profile || profile);
        setServerEdit(false);
        toast.success("SSH 已连接");
      }
    } catch (e) {
      const message =
        typeof e === "object" && e !== null
          ? String((e as Data).message)
          : String(e);
      if (message.includes("HOST_KEY:")) setHostKey(message);
    }
  }
  async function send() {
    const prompt = draft.trim();
    if (!prompt) return;
    const continuing = task && !newConversation;
    const args: Data = {
      prompt,
      task_id: "",
      mode,
      model_policy: modelPolicy,
    };
    if (modelPolicy === "explicit") args.model = model.trim();
    if (continuing) args.reply_to = task.task_id;
    else if (
      currentProject &&
      projects.some((p) => (p.path || p.workspace) === currentProject)
    )
      args.remote_project = currentProject;
    const signature = JSON.stringify({
      ...args,
      reply_to: continuing ? task.task_id : null,
    });
    if (submission.current?.signature !== signature)
      submission.current = { signature, id: crypto.randomUUID() };
    args.task_id = submission.current.id;
    const r = await request(continuing ? "task_reply" : "task_start", args);
    if (r) {
      submission.current = null;
      setDraft("");
      setNewConversation(false);
      setSelected(r.task_id);
      setDetails((d) => ({ ...d, [r.task_id]: r }));
      await refreshTasks();
    }
  }
  async function checkUpdate() {
    setUpdatePhase("checking");
    try {
      const r = await invoke<Data>("check_update");
      setUpdate(r);
      setUpdatePhase(r.available ? "available" : "current");
    } catch (e) {
      setUpdatePhase("failed");
      setErrors((x) => ({ ...x, update: String(e) }));
    }
  }
  async function downloadUpdate() {
    setUpdatePhase("downloading");
    try {
      await invoke("download_update");
      setUpdatePhase("verified");
    } catch (e) {
      setUpdatePhase("failed");
      setErrors((x) => ({ ...x, update: String(e) }));
    }
  }
  const errorEntries = Object.entries(errors).filter(
    ([key]) => !["task_list", "task_status", "project_list"].includes(key),
  );
  const input = (key: string, type = "text", placeholder = "") => (
    <Input
      type={type}
      value={profile[key] ?? ""}
      placeholder={placeholder}
      onChange={(e) =>
        patch(key, type === "number" ? Number(e.target.value) : e.target.value)
      }
    />
  );
  return (
    <div className="app">
      <aside className="sidebar">
        <div className="brand">
          <span className="brand-icon">
            <Terminal size={19} />
          </span>
          <div>
            CCBridge<small>远端开发，近在眼前</small>
          </div>
        </div>
        <nav>
          {pages.map(([id, label, Icon]) => (
            <button
              key={id}
              className={page === id ? "selected" : ""}
              onClick={() => setPage(id)}
            >
              <Icon size={18} />
              {label}
              {id === "chat" && tasks.some(active) && (
                <span className="nav-dot" />
              )}
            </button>
          ))}
        </nav>
        <div className="sidebar-bottom">
          <div className="connection-mini">
            <span className={connected ? "dot live" : "dot"} />
            <span>
              {connected ? `${profile.user}@${profile.host}` : "尚未连接服务器"}
            </span>
          </div>
          <small>CCBridge {version} · 开源版</small>
        </div>
      </aside>
      <main>
        <header>
          <div>
            <span className="breadcrumb">工作空间</span>
            <ChevronRight size={14} />
            <strong>{pages.find((p) => p[0] === page)?.[1]}</strong>
          </div>
          <Badge variant={connected ? "secondary" : "outline"}>
            <span className={connected ? "dot live" : "dot"} />
            {connected ? "SSH 已连接" : "未连接"}
          </Badge>
        </header>
        <div className={`page ${page === "chat" ? "chat-page" : ""}`}>
          <div className="page-heading">
            <div>
              <h1>{pages.find((p) => p[0] === page)?.[1]}</h1>
              <p>
                {
                  {
                    overview: "连接、项目与执行状态，一目了然。",
                    servers: "保存你的 SSH 账号，在当前用户目录内独立运行。",
                    projects: "选择远端项目，浏览文件，或克隆已有仓库。",
                    chat: "以会话阅读结果，按轮次控制模型与执行权限。",
                    codex: "一次配置，在其他 Codex 会话中直接派发和续聊。",
                    updates: "从 GitHub 获取签名更新，无需重新下载安装包。",
                    settings: "本地偏好与兼容信息。",
                  }[page]
                }
              </p>
            </div>
            {page === "chat" && (
              <Button
                onClick={() => {
                  switchConversation("");
                  setModelPolicy("inherit");
                }}
              >
                <Plus size={16} />
                新对话
              </Button>
            )}
          </div>
          {!connected &&
            page !== "servers" &&
            page !== "updates" &&
            page !== "settings" && (
              <div className="notice">
                <Server size={18} />
                <span>先连接服务器，才能读取远端项目与任务。</span>
                <Button
                  variant="outline"
                  size="sm"
                  onClick={() => setPage("servers")}
                >
                  前往连接
                </Button>
              </div>
            )}
          {errorEntries.length > 0 && (
            <div className="error-box" role="alert">
              {errorEntries.map(([k, v]) => (
                <div key={k}>
                  <span>{String(v)}</span>
                  <Button
                    variant="ghost"
                    size="sm"
                    onClick={() =>
                      setErrors((x) => {
                        const n = { ...x };
                        delete n[k];
                        return n;
                      })
                    }
                  >
                    关闭
                  </Button>
                </div>
              ))}
            </div>
          )}
          {page === "overview" && (
            <>
              <div className="stats">
                <Panel title="SSH 连接">
                  <div className="metric">
                    {connected ? "已连接" : "未连接"}
                  </div>
                  <p className="muted">
                    {connected
                      ? `${profile.user} · ${profile.host}:${profile.port}`
                      : "连接你自己的服务器账号"}
                  </p>
                </Panel>
                <Panel title="远端项目">
                  <div className="metric">
                    {projects.length}
                    <small>个项目</small>
                  </div>
                  <p className="muted truncate">
                    {currentProject || "尚未选择工作区"}
                  </p>
                </Panel>
                <Panel title="进行中的任务">
                  <div className="metric">
                    {tasks.filter(active).length}
                    <small>个任务</small>
                  </div>
                  <p className="muted">
                    {tasks.filter((t) => t.state === "waiting_approval").length}{" "}
                    个等待审批
                  </p>
                </Panel>
              </div>
              <Panel
                title="开始远端协作"
                description="项目目录和已有会话相互独立，切换工作区不会改变旧任务。"
              >
                <div className="quick-actions">
                  <Button variant="outline" onClick={() => setPage("projects")}>
                    <FolderGit2 />
                    选择项目
                  </Button>
                  <Button onClick={() => setPage("chat")}>
                    <MessagesSquare />
                    打开对话
                  </Button>
                  <Button variant="outline" onClick={() => setPage("codex")}>
                    <Cable />
                    配置 Codex
                  </Button>
                </div>
              </Panel>
            </>
          )}
          {page === "servers" && (
            <>
              <Panel
                title="当前服务器"
                description="SSH 端口可修改；22 只是 SSH 的标准端口，不是固定要求。"
              >
                <div className="server-row">
                  <div className="server-symbol">
                    <Server />
                  </div>
                  <div className="grow">
                    <h3>
                      {profile.host
                        ? `${profile.user}@${profile.host}`
                        : "添加你的服务器"}
                    </h3>
                    <p className="muted">
                      {profile.host
                        ? `端口 ${profile.port} · 当前账号隔离`
                        : "使用账号密码连接，不需要共享同事的凭据"}
                    </p>
                  </div>
                  <Button
                    variant="outline"
                    disabled={busy.has("connect_ssh")}
                    onClick={() => setServerEdit(true)}
                  >
                    编辑连接
                  </Button>
                  {connected ? (
                    <Button
                      variant="outline"
                      onClick={() =>
                        safe(
                          request("disconnect_ssh").then(() =>
                            setConnection({ connected: false }),
                          ),
                        )
                      }
                    >
                      断开连接
                    </Button>
                  ) : (
                    <Button
                      disabled={busy.has("connect_ssh") || !profile.host}
                      onClick={() => safe(connect())}
                    >
                      {busy.has("connect_ssh") ? (
                        <Loader2 className="spin" />
                      ) : (
                        <Cable />
                      )}
                      连接 SSH
                    </Button>
                  )}
                </div>
                {Object.keys(saved).length > 1 && (
                  <Field label="已保存账号">
                    <Select
                      onValueChange={(key) =>
                        safe(
                          request("load_profile", { account_key: key }).then(
                            (r) => r && setProfile(r.profile),
                          ),
                        )
                      }
                    >
                      <SelectTrigger>
                        <SelectValue placeholder="切换已保存账号" />
                      </SelectTrigger>
                      <SelectContent>
                        {Object.entries(saved).map(([key, p]) => (
                          <SelectItem key={key} value={key}>
                            {p.user}@{p.host}:{p.port}
                          </SelectItem>
                        ))}
                      </SelectContent>
                    </Select>
                  </Field>
                )}
              </Panel>
              <Panel
                title="远端运行环境"
                description="连接 SSH 不等于 Claude Code 和 Runner 已就绪。"
              >
                <div className="environment-row">
                  <div>
                    <Label>当前账号 Runner</Label>
                    <p className="muted">
                      {runner.runner_version
                        ? `已安装 ${runner.runner_version}`
                        : connected
                          ? "尚未检测到"
                          : "连接后检测"}{" "}
                      · App {version}
                    </p>
                    <code>
                      {connection.resolved_paths?.runner ||
                        "自动使用远端 HOME/.local/bin/ccbridge"}
                    </code>
                  </div>
                  <Badge variant="outline">
                    {runner.runner_version === version
                      ? "已是最新版本"
                      : runner.runner_version
                        ? "可检查更新"
                        : "待检测"}
                  </Badge>
                  <Button
                    variant="outline"
                    disabled={!connected || busy.has("refresh_runner")}
                    onClick={() =>
                      safe(request("refresh_runner").then(refreshConnection))
                    }
                  >
                    <RefreshCw size={15} />
                    重新检测
                  </Button>
                  <Button
                    disabled={
                      !connected ||
                      runner.runner_version === version ||
                      busy.has("install_runner")
                    }
                    onClick={() =>
                      safe(
                        request("install_runner").then(() => {
                          toast.success("Runner 已就绪");
                          return refreshConnection();
                        }),
                      )
                    }
                  >
                    {busy.has("install_runner") ? (
                      "安装中…"
                    ) : runner.runner_version === version ? (
                      <>
                        <Check size={15} />
                        已安装
                      </>
                    ) : (
                      "安装 / 更新 Runner"
                    )}
                  </Button>
                </div>
                <div className="environment-row">
                  <div>
                    <Label>Claude Code</Label>
                    <p className="muted">
                      {runner.claude_version ||
                        runner.claude_error ||
                        "使用远端已安装并登录的 Claude Code"}
                    </p>
                    <code>
                      {connection.resolved_paths?.claude ||
                        "自动探测可执行文件"}
                    </code>
                  </div>
                </div>
                <Field
                  label="服务器默认模型"
                  hint="留空沿用远端 Claude 设置。修改后保存；已存在的对话保留各轮模型。"
                >
                  {input("model", "text", "例如 sonnet、opus 或完整模型 ID")}
                </Field>
                <Button
                  variant="outline"
                  disabled={busy.has("save_profile")}
                  onClick={() =>
                    safe(
                      request("save_profile", { profile, password }).then(() =>
                        toast.success("默认模型已保存"),
                      ),
                    )
                  }
                >
                  保存默认模型
                </Button>
              </Panel>
              <Panel
                title="执行权限"
                description="允许全自动与每轮实际模式分别设置。全自动不扩大你派发任务的范围。"
              >
                <div className="switch-row">
                  <Switch
                    checked={profile.allow_auto === true}
                    onCheckedChange={(v) =>
                      v
                        ? setAutoConfirm(true)
                        : safe(
                            request("set_auto", { allow: false }).then(
                              refreshConnection,
                            ),
                          )
                    }
                  />
                  <div>
                    <Label>允许选择全自动模式</Label>
                    <p className="muted">
                      跳过 Claude 工具审批，可使用该 SSH
                      账号的文件、命令和网络权限。
                    </p>
                  </div>
                </div>
              </Panel>
            </>
          )}
          {page === "projects" && (
            <div className="project-grid">
              <Panel
                title="已注册项目"
                description="每个项目有独立目录，可包含多个 Git 仓库。"
              >
                <div className="stack">
                  {projects.map((p) => (
                    <button
                      className={`project-item ${(p.path || p.workspace) === currentProject ? "active" : ""}`}
                      key={p.path || p.workspace}
                      onClick={() => safe(browse(p.path || p.workspace))}
                    >
                      <FolderGit2 size={20} />
                      <div>
                        <strong>{p.name}</strong>
                        <small>{p.path || p.workspace}</small>
                      </div>
                      <ChevronRight size={15} />
                    </button>
                  ))}
                  {!projects.length && (
                    <p className="empty">连接后选择文件夹并注册为项目。</p>
                  )}
                </div>
                <Button
                  variant="outline"
                  disabled={!connected}
                  onClick={() =>
                    safe(browse(connection.environment?.home || ""))
                  }
                >
                  <Folder size={16} />
                  浏览账号 HOME
                </Button>
              </Panel>
              <Panel
                title="远端文件"
                description="仅浏览当前 SSH 账号 HOME 下的目录。"
              >
                <div className="toolbar">
                  <Input
                    aria-label="远端目录"
                    value={path}
                    onChange={(e) => setPath(e.target.value)}
                    onKeyDown={(e) => e.key === "Enter" && safe(browse(path))}
                  />
                  <Button
                    variant="outline"
                    disabled={!connected}
                    onClick={() => safe(browse(path))}
                  >
                    <RefreshCw size={16} />
                  </Button>
                  <Button
                    variant="outline"
                    disabled={!path}
                    onClick={() =>
                      safe(browse(path.slice(0, path.lastIndexOf("/")) || "/"))
                    }
                  >
                    <ArrowUp size={16} />
                  </Button>
                </div>
                <div className="file-list">
                  {(listing.entries || []).map((e: Data) => (
                    <button
                      key={e.path}
                      onClick={() =>
                        ["directory", "repository"].includes(e.kind)
                          ? safe(browse(e.path))
                          : safe(
                              request("file_preview", { path: e.path }).then(
                                (r) =>
                                  setPreview(
                                    typeof r.text === "string"
                                      ? r.text
                                      : JSON.stringify(r, null, 2),
                                  ),
                              ),
                            )
                      }
                    >
                      {e.kind === "file" ? (
                        <FileText size={17} />
                      ) : (
                        <Folder size={17} />
                      )}
                      <span>{e.name}</span>
                      {e.kind === "repository" && (
                        <Badge variant="secondary">Git</Badge>
                      )}
                      <ChevronRight size={14} />
                    </button>
                  ))}
                  {!listing.entries?.length && (
                    <p className="empty">选择项目或打开 HOME 浏览目录。</p>
                  )}
                </div>
                {listing.truncated && (
                  <p className="hint">目录超过 1000 项，当前显示已截断。</p>
                )}
                <div className="toolbar">
                  <Button
                    variant="outline"
                    disabled={!connected || !path}
                    onClick={() => setCloneOpen(true)}
                  >
                    <Plus size={16} />
                    克隆仓库
                  </Button>
                  <Button
                    disabled={!connected || !path}
                    onClick={() =>
                      safe(
                        request("select_workspace", { path }).then(
                          refreshConnection,
                        ),
                      )
                    }
                  >
                    使用此目录
                  </Button>
                </div>
                <div className="register">
                  <Input
                    aria-label="新文件夹名称"
                    value={folderName}
                    onChange={(e) => setFolderName(e.target.value)}
                    placeholder="新文件夹名称"
                  />
                  <Button
                    variant="outline"
                    disabled={
                      !path ||
                      !folderName.trim() ||
                      busy.has("create_directory")
                    }
                    onClick={() =>
                      safe(
                        request("create_directory", {
                          path,
                          name: folderName,
                        }).then(() => {
                          setFolderName("");
                          return browse(path);
                        }),
                      )
                    }
                  >
                    新建文件夹
                  </Button>
                </div>
                <div className="register">
                  <Input
                    value={projectName}
                    onChange={(e) => setProjectName(e.target.value)}
                    placeholder="项目名称，例如 EHS"
                  />
                  <Button
                    variant="outline"
                    disabled={!path || !projectName.trim()}
                    onClick={() =>
                      safe(
                        request("project_save", {
                          name: projectName,
                          path,
                          workspace: path,
                        }).then(() => {
                          toast.success("项目已注册");
                          return refreshProjects();
                        }),
                      )
                    }
                  >
                    注册为项目
                  </Button>
                </div>
              </Panel>
            </div>
          )}
          {page === "chat" && (
            <div className="chat-layout">
              <aside className="sessions">
                <div className="sessions-heading">
                  <Label>会话</Label>
                  <Button
                    variant="ghost"
                    size="icon"
                    disabled={!connected || busy.has("task_list")}
                    onClick={() => safe(refreshTasks())}
                  >
                    <RefreshCw size={15} />
                  </Button>
                </div>
                {latestSessions(tasks).map((t) => (
                  <button
                    className={selected === t.task_id ? "active" : ""}
                    key={t.task_id}
                    onClick={() => {
                      switchConversation(t.task_id);
                    }}
                  >
                    <strong>
                      {t.prompt?.slice(0, 40) || t.task_id.slice(0, 8)}
                    </strong>
                    <span>
                      <i className={`dot ${active(t) ? "live" : ""}`} />
                      {statusLabels[t.state] || t.state}
                    </span>
                    <small>{t.workspace}</small>
                  </button>
                ))}
                {!tasks.length && <p className="empty">还没有远端任务</p>}
              </aside>
              <section className="conversation">
                <div className="conversation-heading">
                  <div>
                    <strong>
                      {task && !newConversation ? "当前对话" : "新对话"}
                    </strong>
                    <small>
                      {task && !newConversation
                        ? task.workspace
                        : currentProject || "在项目页选择工作区"}
                    </small>
                  </div>
                  {task && !newConversation && (
                    <Badge variant="outline">
                      {task.mode === "automatic" ? "全自动" : "审批模式"}
                    </Badge>
                  )}
                </div>
                <div className="messages">
                  {task && !newConversation ? (
                    <>
                      {timeline.map((turn) => (
                        <div key={turn.task_id}>
                          <article className="message user">
                            <div className="message-label">
                              你{" "}
                              <Badge variant="outline">
                                {displayModel(turn).requested}
                              </Badge>
                            </div>
                            <p>{turn.prompt}</p>
                          </article>
                          <article className="message assistant">
                            <div className="message-label">
                              <Sparkles size={16} />
                              Claude Code{" "}
                              <Badge variant="secondary">
                                {displayModel(turn).actual}
                              </Badge>
                              <span>
                                {statusLabels[turn.state] || turn.state}
                              </span>
                            </div>
                            {turn.result ? (
                              <Markdown
                                text={
                                  typeof turn.result === "string"
                                    ? turn.result
                                    : turn.result?.result || "本轮没有文本结果"
                                }
                              />
                            ) : (
                              <p className="muted">
                                {active(task)
                                  ? "正在执行；状态每 5 秒更新，不会重复派发任务。"
                                  : "本轮没有文本结果。"}
                              </p>
                            )}
                            {turn.error && (
                              <div className="inline-error">{turn.error}</div>
                            )}
                            {(turn.events || []).length > 0 && (
                              <details>
                                <summary>
                                  工具与运行记录 · {turn.events.length} 条
                                </summary>
                                {turn.events.map((e: Data, i: number) => (
                                  <details key={i}>
                                    <summary>
                                      {e.type || e.event || "运行事件"}
                                    </summary>
                                    <pre>{JSON.stringify(e, null, 2)}</pre>
                                  </details>
                                ))}
                              </details>
                            )}
                          </article>
                        </div>
                      ))}
                    </>
                  ) : (
                    <div className="chat-empty">
                      <span>
                        <MessagesSquare size={30} />
                      </span>
                      <h2>从一个明确的任务开始</h2>
                      <p>
                        Claude
                        在远端项目里读取和执行。结果保留为会话，随时继续追问。
                      </p>
                    </div>
                  )}
                  {task?.state === "waiting_approval" &&
                    (task.approvals || []).map((a: Data) => (
                      <div className="approval" key={a.request_id}>
                        <ShieldCheck />
                        <div>
                          <strong>需要确认 · {a.tool_name}</strong>
                          <pre>{JSON.stringify(a.input, null, 2)}</pre>
                          <div className="toolbar">
                            <Button
                              disabled={busy.has("task_approve")}
                              onClick={() =>
                                safe(
                                  request("task_approve", {
                                    task_id: task.task_id,
                                    request_id: a.request_id,
                                    allow: true,
                                  }).then(() => fetchTask(task.task_id)),
                                )
                              }
                            >
                              允许本次
                            </Button>
                            <Button
                              variant="outline"
                              disabled={busy.has("task_approve")}
                              onClick={() =>
                                safe(
                                  request("task_approve", {
                                    task_id: task.task_id,
                                    request_id: a.request_id,
                                    allow: false,
                                  }).then(() => fetchTask(task.task_id)),
                                )
                              }
                            >
                              拒绝
                            </Button>
                          </div>
                        </div>
                      </div>
                    ))}
                </div>
                <div className="composer">
                  <div className="composer-options">
                    <Select value={mode} onValueChange={setMode}>
                      <SelectTrigger className="w-36">
                        <SelectValue />
                      </SelectTrigger>
                      <SelectContent>
                        <SelectItem
                          value="automatic"
                          disabled={!profile.allow_auto}
                        >
                          全自动
                        </SelectItem>
                        <SelectItem value="approval">审批模式</SelectItem>
                      </SelectContent>
                    </Select>
                    <Select value={modelPolicy} onValueChange={setModelPolicy}>
                      <SelectTrigger className="w-44">
                        <SelectValue />
                      </SelectTrigger>
                      <SelectContent>
                        <SelectItem value="inherit">
                          {task ? "沿用上轮模型" : "服务器默认模型"}
                        </SelectItem>
                        <SelectItem value="remote_default">
                          远端默认模型
                        </SelectItem>
                        <SelectItem value="explicit">指定模型…</SelectItem>
                      </SelectContent>
                    </Select>
                    {modelPolicy === "explicit" && (
                      <Input
                        className="model-input"
                        value={model}
                        onChange={(e) => setModel(e.target.value)}
                        placeholder="sonnet / opus / 完整 ID"
                      />
                    )}
                  </div>
                  <Textarea
                    aria-label="任务指令"
                    value={draft}
                    onChange={(e) => setDraft(e.target.value)}
                    placeholder={
                      task && !newConversation
                        ? "继续追问当前对话…"
                        : "描述要在远端项目中完成的任务…"
                    }
                    onKeyDown={(e) => {
                      if ((e.metaKey || e.ctrlKey) && e.key === "Enter") {
                        e.preventDefault();
                        safe(send());
                      }
                    }}
                  />
                  <div className="composer-footer">
                    <span>
                      {task && !newConversation
                        ? "继续当前会话 · 保留原目录"
                        : "新任务 · 使用当前项目"}{" "}
                      · ⌘ Enter 发送
                    </span>
                    <div>
                      {task && active(task) && (
                        <Button
                          variant="outline"
                          onClick={() =>
                            safe(
                              request("task_cancel", {
                                task_id: task.task_id,
                              }).then(() => fetchTask(task.task_id)),
                            )
                          }
                        >
                          <Square size={14} />
                          停止
                        </Button>
                      )}
                      <Button
                        disabled={
                          !connected ||
                          !draft.trim() ||
                          busy.has("task_start") ||
                          busy.has("task_reply") ||
                          (!!task && active(task)) ||
                          (modelPolicy === "explicit" && !model.trim()) ||
                          (mode === "automatic" && !profile.allow_auto)
                        }
                        onClick={() => safe(send())}
                      >
                        <Send size={15} />
                        {task && !newConversation ? "继续对话" : "派发任务"}
                      </Button>
                    </div>
                  </div>
                </div>
              </section>
            </div>
          )}
          {page === "codex" && (
            <>
              <Panel
                title="一键配置 Codex"
                description="使用 /Applications 中的稳定路径；保留其他 MCP 和已有配置。"
              >
                <div className="step">
                  <span>1</span>
                  <div>
                    <h3>安装并连接</h3>
                    <p>
                      当前版本 {version}。在服务器页连接
                      SSH，在项目页选择远端工作区。
                    </p>
                  </div>
                </div>
                <div className="step">
                  <span>2</span>
                  <div>
                    <h3>自动写入 MCP 配置</h3>
                    <p>仅合并 ccbridge 配置，并生成原配置备份。</p>
                    <Button
                      onClick={() =>
                        safe(
                          invoke<string>("configure_codex").then(
                            setIntegration,
                          ),
                        )
                      }
                    >
                      <Cable size={16} />
                      配置 Codex MCP
                    </Button>
                    {integration && (
                      <p className="success-text">{integration}</p>
                    )}
                  </div>
                </div>
                <div className="step">
                  <span>3</span>
                  <div>
                    <h3>重新加载 MCP，再开始对话</h3>
                    <p>
                      Codex 检查连接和项目后，通过 cc_start 派发，cc_reply
                      续聊，cc_result 等待结果。模型与模式按每轮请求记录。
                    </p>
                  </div>
                </div>
              </Panel>
              <Panel title="稳定的调用入口">
                <pre>{`[mcp_servers.ccbridge]\ncommand = "/Applications/CCBridge.app/Contents/MacOS/ccbridge"\nargs = ["mcp"]\ntool_timeout_sec = 960`}</pre>
                <p className="hint">
                  不写死某个人的用户名或本地仓库路径。每个同事在 App 保存自己的
                  SSH 账号。
                </p>
              </Panel>
            </>
          )}
          {page === "updates" && (
            <>
              <Panel
                title="CCBridge 更新"
                description="GitHub Releases · 更新包在安装前验证发布签名。"
              >
                <div className="update-hero">
                  <span className="brand-icon">
                    <Terminal size={27} />
                  </span>
                  <div>
                    <h2>CCBridge {version}</h2>
                    <p className="muted">
                      Apple Silicon · 开源版 · 无需 Developer ID
                    </p>
                  </div>
                  <Badge variant="outline">稳定通道</Badge>
                </div>
                <div className="update-status">
                  {
                    {
                      idle: "检查是否有新版本",
                      checking: "正在检查 GitHub…",
                      current: "你正在使用最新版本",
                      available: `发现新版本 ${update.version || ""}`,
                      downloading: "正在下载并验证签名…",
                      verified: "签名验证通过，更新已就绪",
                      installing: "正在排空活动并安装…",
                      failed: "更新未完成，当前 App 保持可用",
                    }[updatePhase]
                  }
                </div>
                {update.body && (
                  <details open>
                    <summary>版本说明</summary>
                    <Markdown text={update.body} />
                  </details>
                )}
                {updatePhase === "downloading" && <Progress value={progress} />}
                <div className="toolbar">
                  <Button
                    variant="outline"
                    disabled={[
                      "checking",
                      "downloading",
                      "installing",
                    ].includes(updatePhase)}
                    onClick={() => safe(checkUpdate())}
                  >
                    <RefreshCw size={16} />
                    检查更新
                  </Button>
                  {updatePhase === "available" && (
                    <Button onClick={() => safe(downloadUpdate())}>
                      <Download size={16} />
                      下载更新
                    </Button>
                  )}
                  {updatePhase === "verified" && (
                    <Button
                      onClick={() => {
                        setUpdatePhase("installing");
                        safe(
                          invoke("apply_update").catch((e) => {
                            setUpdatePhase("verified");
                            toast.error(String(e));
                            throw e;
                          }),
                        );
                      }}
                    >
                      安装并重启
                    </Button>
                  )}
                </div>
                <p className="hint">
                  运行中的任务、审批、克隆或 MCP
                  等待会阻止安装。请先完成活动，再安装；不会强制中断远端任务。
                </p>
              </Panel>
              <Panel title="更新与安全">
                <div className="security-lines">
                  <p>
                    <ShieldCheck size={18} />
                    使用独立的开源发布签名，不需要 Apple 开发者账号。
                  </p>
                  <p>
                    <Check size={18} />
                    签名、下载或检查失败，不替换当前 App。
                  </p>
                  <p>
                    <Folder size={18} />
                    账号配置和会话保留在原有用户目录。
                  </p>
                </div>
              </Panel>
            </>
          )}
          {page === "settings" && (
            <Panel title="兼容与存储">
              <dl>
                <dt>当前版本</dt>
                <dd>{version}</dd>
                <dt>数据目录</dt>
                <dd>~/Library/Application Support/CCBridge</dd>
                <dt>密码存储</dt>
                <dd>macOS 钥匙串 · 沿用现有账号身份</dd>
                <dt>数据库功能</dt>
                <dd>暂不开发，已有配置保持不变</dd>
                <dt>发布方式</dt>
                <dd>开源签名更新 · ad-hoc App 签名 · 无 Developer ID</dd>
              </dl>
            </Panel>
          )}
        </div>
      </main>
      <Toaster richColors position="bottom-right" />
      <Dialog open={serverEdit} onOpenChange={setServerEdit}>
        <DialogContent className="server-dialog">
          <DialogHeader>
            <DialogTitle>SSH 服务器</DialogTitle>
            <DialogDescription>
              密码只用于 SSH 登录；记住密码时保存在本机钥匙串。
            </DialogDescription>
          </DialogHeader>
          <Field label="连接名称（可选）">
            {input("name", "text", "例如：我的开发服务器")}
          </Field>
          <Field label="服务器地址">
            {input("host", "text", "服务器域名或 IP")}
          </Field>
          <div className="form-columns">
            <Field label="SSH 端口">{input("port", "number")}</Field>
            <Field label="用户名">{input("user")}</Field>
          </div>
          <Field
            label="密码"
            hint={
              profile.remember_password
                ? "留空可使用钥匙串已保存密码"
                : "只在本次连接中使用"
            }
          >
            <Input
              type="password"
              value={password}
              autoComplete="off"
              onChange={(e) => setPassword(e.target.value)}
            />
          </Field>
          <div className="switch-row">
            <Switch
              checked={!!profile.remember_password}
              onCheckedChange={(v) => patch("remember_password", v)}
            />
            <Label>在钥匙串记住密码</Label>
          </div>
          <details>
            <summary>高级设置 · 通常留空</summary>
            <Field label="Runner 路径">{input("runner")}</Field>
            <Field label="Claude Code 路径">{input("claude")}</Field>
          </details>
          <DialogFooter>
            <Button
              variant="outline"
              disabled={busy.has("save_profile")}
              onClick={() =>
                safe(
                  request("save_profile", { profile, password }).then(() =>
                    toast.success("连接配置已保存"),
                  ),
                )
              }
            >
              保存配置
            </Button>
            <Button
              disabled={
                busy.has("connect_ssh") ||
                !profile.host ||
                !profile.user ||
                !profile.port
              }
              onClick={() => safe(connect())}
            >
              {busy.has("connect_ssh") ? "连接中…" : "连接 SSH"}
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>
      <Dialog open={autoConfirm} onOpenChange={setAutoConfirm}>
        <DialogContent>
          <DialogHeader>
            <DialogTitle>允许全自动任务？</DialogTitle>
            <DialogDescription>
              Claude 将跳过工具审批，拥有当前 SSH
              账号的文件、命令和网络权限。仅对可信任务和专用低权限账号开启。
            </DialogDescription>
          </DialogHeader>
          <DialogFooter>
            <Button variant="outline" onClick={() => setAutoConfirm(false)}>
              保持审批
            </Button>
            <Button
              onClick={() =>
                safe(
                  request("set_auto", { allow: true }).then(() => {
                    setAutoConfirm(false);
                    return refreshConnection();
                  }),
                )
              }
            >
              确认允许全自动
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>
      <Dialog open={!!hostKey} onOpenChange={(v) => !v && setHostKey("")}>
        <DialogContent>
          <DialogHeader>
            <DialogTitle>确认 SSH 主机身份</DialogTitle>
            <DialogDescription>
              请通过可信渠道核对服务器指纹。确认后才会保存并重新连接。
            </DialogDescription>
          </DialogHeader>
          <pre>{hostKey}</pre>
          <DialogFooter>
            <Button variant="outline" onClick={() => setHostKey("")}>
              取消
            </Button>
            <Button
              onClick={() => {
                const fingerprint = hostKey
                  .split("HOST_KEY:")[1]
                  ?.trim()
                  .split(/\s/)[0];
                if (fingerprint) {
                  patch("fingerprint", fingerprint);
                  setHostKey("");
                  toast.info("已填入指纹，请再次连接确认。");
                }
              }}
            >
              信任此指纹
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>
      <Dialog open={cloneOpen} onOpenChange={setCloneOpen}>
        <DialogContent>
          <DialogHeader>
            <DialogTitle>克隆仓库</DialogTitle>
            <DialogDescription>
              仓库将克隆到 {path}。每行一个 HTTPS / SSH 地址，已有目录不会覆盖。
            </DialogDescription>
          </DialogHeader>
          <Textarea
            value={cloneUrls}
            onChange={(e) => setCloneUrls(e.target.value)}
            placeholder="https://github.com/owner/repository.git"
          />
          <div className="switch-row">
            <Switch checked={localAuth} onCheckedChange={setLocalAuth} />
            <Label>使用本机 Git 认证传输（不上传凭据）</Label>
          </div>
          <DialogFooter>
            <Button variant="outline" onClick={() => setCloneOpen(false)}>
              取消
            </Button>
            <Button
              disabled={!cloneUrls.trim() || busy.has("clone_repository")}
              onClick={() =>
                safe(
                  request("clone_repository", {
                    path,
                    urls: cloneUrls,
                    local_auth: localAuth,
                  }).then((r) => {
                    if (r?.complete === false) {
                      toast.error(r.error || "部分仓库克隆失败");
                      return;
                    }
                    setCloneOpen(false);
                    return browse(path);
                  }),
                )
              }
            >
              {busy.has("clone_repository") ? "正在克隆…" : "克隆到当前目录"}
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>
      <Dialog open={!!preview} onOpenChange={(v) => !v && setPreview("")}>
        <DialogContent className="preview-dialog">
          <DialogHeader>
            <DialogTitle>文件预览</DialogTitle>
          </DialogHeader>
          <pre>{preview}</pre>
        </DialogContent>
      </Dialog>
    </div>
  );
}
