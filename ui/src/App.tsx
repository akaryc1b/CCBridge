import { Workbench } from "@/components/Workbench";
import { DropdownMenu as Menu } from "radix-ui";
import { CodeupAuth } from "@/components/CodeupAuth";
import {
  FileActions,
  FileContextMenu,
  FileOperation,
  TrashDialog,
  BranchDialog,
} from "@/components/WorkspaceTools";
import { containingProject, latestNavigation } from "@/lib/navigation";
import { Databases } from "@/components/Databases";
import {
  normalizeTask,
  conversationTurns,
  groupSessions,
  nextModelLabel,
  displayModel,
  eventText,
  eventLabel,
} from "@/lib/conversation";
import {
  useEffect,
  useRef,
  useState,
  useId,
  isValidElement,
  cloneElement,
  type ReactNode,
  type ReactElement,
} from "react";
import { invoke, isTauri } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import {
  Activity,
  Sun,
  Moon,
  Monitor,
  MoreHorizontal,
  GitBranch,
  Search,
  Command,
  FolderOpen,
  Server,
  FolderGit2,
  MessagesSquare,
  Cable,
  Database,
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
import { Progress as ProgressPrimitive } from "radix-ui";
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
  ["overview", "工作台", Activity],
  ["projects", "项目与文件", FolderGit2],
  ["chat", "会话", MessagesSquare],
  ["databases", "数据库", Database],
  ["servers", "服务器", Server],
  ["codex", "Codex / MCP", Cable],
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
  const id = useId();
  return (
    <div className="field">
      <Label htmlFor={id}>{label}</Label>
      {isValidElement(children)
        ? cloneElement(children as ReactElement<Data>, { id })
        : children}
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
    [version, setVersion] = useState("0.5.0"),
    [architecture, setArchitecture] = useState("macOS");
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
    [browsing, setBrowsing] = useState(false),
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
    [integration, setIntegration] = useState(""),
    [integrationState, setIntegrationState] = useState("idle"),
    [cloneProgress, setCloneProgress] = useState<Data>({}),
    [cloneError, setCloneError] = useState("");
  const [authOpen, setAuthOpen] = useState(false),
    [authScope, setAuthScope] = useState("local"),
    [fileOperation, setFileOperation] = useState<Data | null>(null),
    [trashOpen, setTrashOpen] = useState(false),
    [branchesOpen, setBranchesOpen] = useState(false);
  const [cloneResults, setCloneResults] = useState<Data[]>([]),
    [cloneElapsed, setCloneElapsed] = useState(0);
  const cloneRetry = useRef<{ signature: string; urls: string[] } | null>(null);
  const [modelEditing, setModelEditing] = useState(false);
  const [mcpStatus, setMcpStatus] = useState<Data>({});
  const [theme, setTheme] = useState(
    localStorage.getItem("ccbridge-theme") || "system",
  );
  const [preferences, setPreferences] = useState<Data>({});
  const [installLocation, setInstallLocation] = useState<Data>({
    installed: true,
  });
  const [storageAck, setStorageAck] = useState(false);
  const [storageConsent, setStorageConsent] = useState(false);
  const [utilityDialog, setUtilityDialog] = useState("");
  const [draftProject, setDraftProject] = useState("");
  const [fileFilter, setFileFilter] = useState("");
  const [sessionFilter, setSessionFilter] = useState("");
  const [dataDirectory, setDataDirectory] = useState("");
  const autoStarted = useRef(false);
  const composerStates = useRef<Record<string, Data>>({});
  useEffect(() => {
    const media = matchMedia("(prefers-color-scheme: dark)");
    const apply = () => {
      document.documentElement.dataset.theme =
        theme === "system" ? (media.matches ? "dark" : "light") : theme;
      document.documentElement.classList.toggle(
        "dark",
        document.documentElement.dataset.theme === "dark",
      );
    };
    apply();
    localStorage.setItem("ccbridge-theme", theme);
    if (isTauri())
      void invoke("set_window_theme", { theme }).catch((e) =>
        toast.error(String(e)),
      );
    media.addEventListener("change", apply);
    return () => media.removeEventListener("change", apply);
  }, [theme]);
  function startDraft(project: string) {
    switchConversation("");
    setDraftProject(project);
    setPage("chat");
  }
  const drafts = useRef<Record<string, string>>({});
  const messagesRef = useRef<HTMLDivElement>(null),
    following = useRef(true);

  const submission = useRef<{ signature: string; id: string } | null>(null);
  const detailsRef = useRef(details);
  detailsRef.current = details;

  function switchConversation(id: string) {
    following.current = true;
    drafts.current[selected || "__new__"] = draft;
    composerStates.current[selected || "__new__"] = {
      mode,
      modelPolicy,
      model,
    };
    setDraft(drafts.current[id || "__new__"] || "");
    setSelected(id);
    setNewConversation(!id);
    const stored = composerStates.current[id || "__new__"];
    setModelPolicy(stored?.modelPolicy || "inherit");
    setModel(stored?.model || "");
    const previous = details[id] || tasks.find((t) => t.task_id === id);
    setMode(stored?.mode || previous?.mode || "automatic");
  }
  const requestSequence = useRef(new Map<string, number>());
  const inflight = useRef(new Map<string, number>()),
    selectedRef = useRef(selected);
  selectedRef.current = selected;
  const connected = connection.connected === true,
    task = details[selected] || tasks.find((t) => t.task_id === selected),
    runner = connection.probe || {};
  const timeline = conversationTurns(task, details);
  const transcriptRevision = timeline
    .map((t) =>
      [
        t.task_id,
        t.state,
        t.updated_at,
        t.events?.length,
        t.result?.result?.length,
      ].join(":"),
    )
    .join("|");
  useEffect(() => {
    if (following.current && messagesRef.current)
      messagesRef.current.scrollTop = messagesRef.current.scrollHeight;
  }, [selected, transcriptRevision]);
  const currentProject =
    connection.profile?.active_remote_workspace ||
    profile.active_remote_workspace ||
    "";
  const request = async (
    command: string,
    args: Data = {},
    quiet = false,
  ): Promise<any> => {
    if (
      inflight.current.has(command) &&
      !["directory_list", "file_preview"].includes(command)
    )
      return undefined;
    const sequence = (requestSequence.current.get(command) || 0) + 1;
    requestSequence.current.set(command, sequence);
    inflight.current.set(command, (inflight.current.get(command) || 0) + 1);
    setBusy(new Set(inflight.current.keys()));
    try {
      const data = await invoke(command, { request: args });
      if (requestSequence.current.get(command) === sequence)
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
      if (requestSequence.current.get(command) === sequence) {
        setErrors((old) => ({ ...old, [command]: { page, message } }));
        // Render once beside the affected page; callers with inline errors own their feedback.
      }
      throw e;
    } finally {
      const left = (inflight.current.get(command) || 1) - 1;
      if (left) inflight.current.set(command, left);
      else inflight.current.delete(command);
      setBusy(new Set(inflight.current.keys()));
    }
  };
  useEffect(() => {
    if (!busy.has("clone_repository")) return;
    let active = true;
    let polling = false;
    const poll = async () => {
      if (polling) return;
      polling = true;
      try {
        const p = await invoke<Data>("clone_progress", { request: {} });
        if (active && p.stage) setCloneProgress(p);
      } catch {
        /* The clone request itself reports connection failures. */
      } finally {
        polling = false;
      }
    };
    const timer = setInterval(poll, 1000);
    void poll();
    return () => {
      active = false;
      clearInterval(timer);
    };
  }, [busy.has("clone_repository")]);
  async function configureCodex() {
    if (integrationState === "saving") return;
    setIntegrationState("saving");
    setIntegration("正在备份并写入 Codex MCP 配置…");
    try {
      const message = await invoke<string>("configure_codex");
      setIntegration(message);
      setIntegrationState("success");
      invoke<Data>("codex_configuration_status")
        .then(setMcpStatus)
        .catch(() => {});
    } catch (e) {
      const message =
        typeof e === "object" && e !== null && "message" in e
          ? String((e as Data).message)
          : String(e);
      setIntegration(`配置失败：${message}`);
      setIntegrationState("error");
    }
  }
  async function cloneRepositories() {
    setCloneError("");
    setCloneProgress({ stage: "正在检查连接和目标目录…" });
    const destination = listedPath;
    const signature = JSON.stringify([destination, cloneUrls, localAuth]);
    const urls =
      cloneRetry.current?.signature === signature
        ? cloneRetry.current.urls
        : Array.from(new Set(cloneUrls.trim().split(/\s+/).filter(Boolean)));
    const started = Date.now();
    setCloneElapsed(0);
    const timer = setInterval(
      () => setCloneElapsed(Math.floor((Date.now() - started) / 1000)),
      1000,
    );
    try {
      const r = await request("clone_repository", {
        path: destination,
        urls: urls.join("\n"),
        local_auth: localAuth,
      });
      if (!r) return;
      setCloneResults(r.repositories || []);
      cloneRetry.current =
        r.complete === false
          ? { signature, urls: urls.slice((r.repositories || []).length) }
          : null;
      if (r.complete === false)
        throw new Error(
          `${r.failed_repository || "仓库"}：${r.error || "克隆失败"}`,
        );
      setCloneProgress({
        stage: `处理完成：${r.repositories?.length || 0} 个仓库（已有仓库保持不变）`,
      });
      toast.success("仓库已准备好");
      await browse(destination);
    } catch (e) {
      const message =
        e instanceof Error
          ? e.message
          : typeof e === "object" && e !== null && "message" in e
            ? String((e as Data).message)
            : String(e);
      setCloneError(message);
    } finally {
      clearInterval(timer);
    }
  }
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
  const browserNavigation = useRef<ReturnType<
    typeof latestNavigation<Data>
  > | null>(null);
  if (!browserNavigation.current)
    browserNavigation.current = latestNavigation<Data>(
      (p) => request("directory_list", { path: p, hidden: false }),
      (r) => {
        setListing(r);
        setPath(r.path);
        setPreview("");
      },
      setBrowsing,
    );
  const browserAccount = useRef("");
  useEffect(() => {
    if (!connection.connected) return;
    const identity = JSON.stringify([
      connection.profile?.host,
      connection.profile?.port,
      connection.profile?.user,
    ]);
    if (browserAccount.current && browserAccount.current !== identity) {
      browserNavigation.current?.invalidate();
      setListing({ entries: [] });
      setPath("");
      setProjects([]);
      setTasks([]);
      setDetails({});
      setSelected("");
      setDraft("");
      setPreview("");
      drafts.current = {};
      composerStates.current = {};
    }
    browserAccount.current = identity;
  }, [
    connection.connected,
    connection.profile?.host,
    connection.profile?.port,
    connection.profile?.user,
  ]);
  const previewGeneration = useRef(0);
  async function previewFile(p: string) {
    const id = ++previewGeneration.current;
    const r = await request("file_preview", { path: p });
    if (id === previewGeneration.current && r)
      setPreview(
        typeof r.text === "string" ? r.text : JSON.stringify(r, null, 2),
      );
  }
  async function browse(p: string) {
    previewGeneration.current++;
    setPreview("");
    setPath(p);
    await browserNavigation.current!(p);
  }
  const listedPath = listing.path || "";
  const browsedProject = containingProject(
    listedPath,
    projects
      .map((p) => ({ path: p.path || p.workspace || "" }))
      .filter((p) => p.path),
  );
  const locationReady =
    connected && !browsing && !!listedPath && path === listedPath;
  async function fetchTask(id: string) {
    let currentId: string | undefined = id;
    const seen = new Set<string>();
    while (currentId && !seen.has(currentId)) {
      seen.add(currentId);
      const r = await request(
        "task_status",
        {
          task_id: currentId,
          cursor: detailsRef.current[currentId]?.next_cursor || 0,
        },
        true,
      );
      if (!r) break;
      const key = currentId;
      setDetails((d) => ({
        ...d,
        [key]: {
          ...r,
          events: [...(d[key]?.events || []), ...(r.events || [])],
        },
      }));
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
        setProfile({ ...profile, ...s.profile, credential_store: "file" });
        setSaved(s.profiles || {});
        setVersion(s.version);
        setPreferences(s.preferences || {});
        setStorageConsent(s.local_storage_ack === true);
        setDataDirectory(s.data_directory || "");
        invoke<Data>("codex_configuration_status")
          .then(setMcpStatus)
          .catch((e) => setMcpStatus({ message: String(e) }));
        invoke<Data>("install_location")
          .then(setInstallLocation)
          .catch(() => {});
        setArchitecture(
          s.architecture === "x86_64"
            ? "Intel x86_64"
            : s.architecture === "aarch64"
              ? "Apple Silicon"
              : "macOS",
        );
        const c = await refreshConnection();
        if (!c?.connected && !autoStarted.current) {
          autoStarted.current = true;
          try {
            const result = await request("auto_connect");
            if (result?.connected) {
              setConnection(result);
              setProfile(result.profile);
            }
          } catch {
            /* single attempt; visible page error remains */
          }
        }
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
  }, [connected, runner.runner_version]);
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
      const c = await request("connect_ssh", {
        profile,
        password,
        storage_ack: storageAck,
      });
      if (c) {
        setPassword("");
        setConnection(c);
        setProfile(c.profile || { ...profile, credential_store: "file" });
        const stored = c.profile || profile;
        setSaved((x) => ({
          ...x,
          [JSON.stringify([stored.host, stored.port, stored.user])]: stored,
        }));
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
    if (
      !prompt ||
      !connected ||
      busy.has("task_start") ||
      busy.has("task_reply") ||
      (task && !newConversation && active(task)) ||
      (mode === "automatic" && !profile.allow_auto) ||
      (modelPolicy === "explicit" && !model.trim())
    )
      return;
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
      (draftProject || currentProject) &&
      projects.some(
        (p) => (p.path || p.workspace) === (draftProject || currentProject),
      )
    )
      args.remote_project = draftProject || currentProject;
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
      following.current = true;
      drafts.current[selected || "__new__"] = "";
      setDraft("");
      setNewConversation(false);
      setModelEditing(false);
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
      setErrors((x) => ({
        ...x,
        update: { page: "updates", message: String(e) },
      }));
    }
  }
  async function downloadUpdate() {
    setUpdatePhase("downloading");
    try {
      await invoke("download_update");
      setUpdatePhase("verified");
    } catch (e) {
      setUpdatePhase("failed");
      setErrors((x) => ({
        ...x,
        update: { page: "updates", message: String(e) },
      }));
    }
  }
  const errorEntries = Object.entries(errors).filter(
    ([key, value]) =>
      (!value?.page || value.page === page) &&
      !(
        serverEdit &&
        ["save_profile", "connect_ssh", "migrate_password"].includes(key)
      ) &&
      key !== "clone_repository" &&
      !key.startsWith("database_") &&
      !["task_list", "task_status", "project_list"].includes(key),
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
            <Command size={19} />
          </span>
          <div>CCBridge</div>
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
          <div className="theme-switch">
            <span>外观</span>
            <div>
              {(
                [
                  ["light", Sun, "亮色"],
                  ["dark", Moon, "暗色"],
                  ["system", Monitor, "跟随系统"],
                ] as const
              ).map(([v, Icon, label]) => (
                <Button
                  key={v}
                  aria-label={label}
                  title={label}
                  variant="ghost"
                  className={theme === v ? "chosen" : ""}
                  onClick={() => setTheme(v)}
                >
                  <Icon size={14} />
                </Button>
              ))}
            </div>
          </div>
        </div>
      </aside>
      <main>
        <header>
          <h1>{pages.find((p) => p[0] === page)?.[1]}</h1>
          <div className="page-actions">
            {(page === "overview" || page === "chat") && (
              <Button onClick={() => startDraft(currentProject)}>
                <Plus size={15} />
                新任务
              </Button>
            )}
            {page === "projects" && (
              <>
                <Button
                  variant="outline"
                  disabled={!locationReady}
                  onClick={() => setBranchesOpen(true)}
                >
                  <GitBranch size={15} />
                  {listing.branch || "分支"}
                </Button>
                <Menu.Root>
                  <Menu.Trigger asChild>
                    <Button variant="outline" disabled={!locationReady}>
                      <Plus size={15} />
                      新建
                    </Button>
                  </Menu.Trigger>
                  <Menu.Portal>
                    <Menu.Content className="context-menu" sideOffset={5}>
                      <Menu.Item
                        onSelect={() =>
                          setFileOperation({
                            action: "create",
                            entry: { path: listedPath, name: "" },
                          })
                        }
                      >
                        文件
                      </Menu.Item>
                      <Menu.Item onSelect={() => setUtilityDialog("folder")}>
                        文件夹
                      </Menu.Item>
                    </Menu.Content>
                  </Menu.Portal>
                </Menu.Root>
                <Button
                  disabled={!locationReady}
                  onClick={() => setCloneOpen(true)}
                >
                  克隆仓库
                </Button>
                <Menu.Root>
                  <Menu.Trigger asChild>
                    <Button aria-label="更多项目操作" variant="ghost">
                      <MoreHorizontal size={17} />
                    </Button>
                  </Menu.Trigger>
                  <Menu.Portal>
                    <Menu.Content className="context-menu" sideOffset={5}>
                      <Menu.Item onSelect={() => setUtilityDialog("project")}>
                        添加为项目…
                      </Menu.Item>
                      <Menu.Item
                        disabled={
                          !locationReady || listedPath === currentProject
                        }
                        onSelect={() =>
                          safe(
                            request("select_workspace", {
                              path: listedPath,
                            }).then(refreshConnection),
                          )
                        }
                      >
                        设为MCP默认项目
                      </Menu.Item>
                      <Menu.Item onSelect={() => setTrashOpen(true)}>
                        回收区
                      </Menu.Item>
                      <Menu.Item
                        onSelect={() => {
                          setAuthScope(localAuth ? "local" : "remote");
                          setAuthOpen(true);
                        }}
                      >
                        Codeup认证
                      </Menu.Item>
                    </Menu.Content>
                  </Menu.Portal>
                </Menu.Root>
              </>
            )}
          </div>
        </header>
        <div
          className={`page page-${page} ${page === "chat" ? "chat-page" : ""}`}
        >
          {!installLocation.installed && (
            <div className="notice">
              当前App不在“应用程序”中。
              <Button
                onClick={() =>
                  safe(
                    invoke("move_to_applications").catch((e) =>
                      toast.error(String(e)),
                    ),
                  )
                }
              >
                安装到Applications并重新打开
              </Button>
            </div>
          )}
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
                  <span>{String(v?.message || v)}</span>
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
            <Workbench
              projects={projects}
              tasks={tasks}
              currentProject={currentProject}
              connection={connection}
              onFiles={(p) => {
                setPage("projects");
                safe(browse(p));
              }}
              onNew={startDraft}
              onSession={(id) => {
                setPage("chat");
                switchConversation(id);
              }}
              onPage={setPage}
            />
          )}
          {page === "servers" && (
            <>
              <Panel title="当前服务器">
                <div className="server-row">
                  <div className="server-symbol">
                    <Server />
                  </div>
                  <div className="grow">
                    <h3>
                      {profile.name ||
                        (profile.host
                          ? `${profile.user}@${profile.host}`
                          : "添加你的服务器")}
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
              <Panel title="执行权限">
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
                    <Label>允许全自动任务</Label>
                    <p className="muted">App 与 MCP 可按任务请求全自动</p>
                  </div>
                </div>
              </Panel>{" "}
              <Panel title="远端运行环境">
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
                      request("save_profile", {
                        profile,
                        password,
                        storage_ack: storageAck,
                      }).then(() => toast.success("默认模型已保存")),
                    )
                  }
                >
                  保存默认模型
                </Button>
              </Panel>
            </>
          )}
          {page === "projects" && (
            <div className="project-grid">
              <aside className="project-sidebar">
                <div className="section-title">
                  <span>项目</span>
                  <Button
                    variant="ghost"
                    size="icon"
                    aria-label="添加项目"
                    onClick={() => setUtilityDialog("project")}
                  >
                    <Plus size={14} />
                  </Button>
                </div>
                {projects.map((p) => (
                  <button
                    className={`project-item ${p.path === browsedProject ? "active" : ""}`}
                    key={p.path}
                    onClick={() => safe(browse(p.path))}
                  >
                    <FolderOpen size={17} />
                    <div>
                      <strong>{p.name}</strong>
                      {p.path === currentProject && <small>MCP默认项目</small>}
                    </div>
                  </button>
                ))}
                <div className="section-divider" />
                <Button
                  variant="ghost"
                  disabled={!connected}
                  onClick={() =>
                    safe(browse(connection.environment?.home || ""))
                  }
                >
                  <Folder size={16} />
                  账号 HOME
                </Button>
              </aside>
              <section className="file-workspace">
                <div className="path-toolbar">
                  <Input
                    aria-label="远端目录"
                    value={path}
                    onChange={(e) => setPath(e.target.value)}
                    onKeyDown={(e) => e.key === "Enter" && safe(browse(path))}
                  />
                  <Button
                    aria-label="上一级"
                    variant="ghost"
                    disabled={!locationReady}
                    onClick={() =>
                      safe(browse(path.slice(0, path.lastIndexOf("/")) || "/"))
                    }
                  >
                    <ArrowUp size={15} />
                  </Button>
                  <Button
                    aria-label="刷新目录"
                    variant="ghost"
                    disabled={!connected}
                    onClick={() => safe(browse(path))}
                  >
                    <RefreshCw className={browsing ? "spin" : ""} size={15} />
                  </Button>
                  <Input
                    className="file-filter"
                    aria-label="筛选文件"
                    placeholder="筛选文件…"
                    value={fileFilter}
                    onChange={(e) => setFileFilter(e.target.value)}
                  />
                </div>
                <div className="file-list" aria-busy={browsing}>
                  <div className="file-head">
                    <span>名称</span>
                    <span>类型</span>
                  </div>
                  {(listing.entries || [])
                    .filter((e: Data) =>
                      e.name.toLowerCase().includes(fileFilter.toLowerCase()),
                    )
                    .map((e: Data) => (
                      <FileContextMenu
                        key={e.path}
                        entry={e}
                        disabled={!locationReady}
                        onAction={(action, entry) =>
                          setFileOperation({ action, entry })
                        }
                      >
                        <div
                          className="file-row"
                          onKeyDown={(event) => {
                            if (!locationReady) return;
                            if (event.key === "F2") {
                              event.preventDefault();
                              setFileOperation({ action: "rename", entry: e });
                            } else if (
                              event.metaKey &&
                              event.key === "Backspace"
                            ) {
                              event.preventDefault();
                              setFileOperation({ action: "trash", entry: e });
                            }
                          }}
                        >
                          <button
                            disabled={!locationReady}
                            onClick={() =>
                              ["directory", "repository"].includes(e.kind)
                                ? safe(browse(e.path))
                                : safe(previewFile(e.path))
                            }
                          >
                            {e.kind === "file" ? (
                              <FileText size={17} />
                            ) : (
                              <Folder size={17} />
                            )}
                            <span>{e.name}</span>
                            {e.kind === "repository" && (
                              <Badge variant="outline">Git</Badge>
                            )}
                            <small>
                              {e.kind === "file"
                                ? "文件"
                                : e.kind === "symlink"
                                  ? "链接"
                                  : "文件夹"}
                            </small>
                          </button>
                          <FileActions
                            entry={e}
                            disabled={!locationReady}
                            onAction={(action, entry) =>
                              setFileOperation({ action, entry })
                            }
                          />
                        </div>
                      </FileContextMenu>
                    ))}
                  {!listing.entries?.length && (
                    <p className="empty">
                      {browsing
                        ? "正在打开…"
                        : listedPath
                          ? "空文件夹"
                          : "选择项目或输入目录"}
                    </p>
                  )}
                </div>
                <div className="file-status">
                  {listing.entries?.length || 0}项
                  {listing.truncated ? " · 仅显示前1000项" : ""}
                  <span>服务器文件</span>
                </div>
              </section>
            </div>
          )}
          {page === "chat" && (
            <div className="chat-layout">
              <aside className="sessions">
                <div className="sessions-heading">
                  <Input
                    aria-label="搜索会话"
                    placeholder="搜索会话…"
                    value={sessionFilter}
                    onChange={(e) => setSessionFilter(e.target.value)}
                  />
                  <Button
                    variant="ghost"
                    size="icon"
                    disabled={!connected || busy.has("task_list")}
                    onClick={() => safe(refreshTasks())}
                  >
                    <RefreshCw size={15} />
                  </Button>
                </div>
                {groupSessions(tasks, projects).map((group) => (
                  <details
                    className="session-project"
                    key={group.path || "__unknown"}
                    open
                  >
                    <summary>
                      <FolderGit2 size={15} />
                      <span>{group.name}</span>
                      <Badge variant="secondary">{group.sessions.length}</Badge>
                    </summary>
                    <Button
                      className="group-new"
                      disabled={!group.registered}
                      title={
                        group.registered ? "新任务" : "请先在项目页注册此目录"
                      }
                      variant="ghost"
                      aria-label={`为${group.name}新建任务`}
                      onClick={() =>
                        startDraft(
                          group.registered ? group.path : currentProject,
                        )
                      }
                    >
                      <Plus size={13} />
                    </Button>
                    {group.sessions
                      .filter((t) =>
                        (t.prompt || t.title || "")
                          .toLowerCase()
                          .includes(sessionFilter.toLowerCase()),
                      )
                      .map((t) => (
                        <button
                          className={selected === t.task_id ? "active" : ""}
                          key={t.task_id}
                          onClick={() => switchConversation(t.task_id)}
                        >
                          <strong>
                            {String(t.title || t.prompt || t.task_id)
                              .split("\n")
                              .find((s) => s.trim())
                              ?.replace(/^#+\s*/, "")
                              .slice(0, 48)}
                          </strong>
                          <span>
                            {t.state !== "completed" && (
                              <>
                                <i
                                  className={`dot ${active(t) ? "live" : ""}`}
                                />
                                {statusLabels[t.state] || t.state} ·{" "}
                              </>
                            )}
                            {new Date(
                              (t.updated_at || t.created_at) * 1000,
                            ).toLocaleDateString()}
                          </span>
                          {t.workspace !== group.path && (
                            <small>{t.workspace}</small>
                          )}
                        </button>
                      ))}
                  </details>
                ))}
                {!tasks.length && <p className="empty">还没有远端任务</p>}
              </aside>
              <section className="conversation">
                <div className="conversation-heading">
                  <div>
                    <strong>
                      {task && !newConversation
                        ? String(task.title || task.prompt || "会话")
                            .split("\n")[0]
                            .slice(0, 40)
                        : "新任务"}
                    </strong>
                    <small>
                      {task && !newConversation
                        ? task.workspace
                        : draftProject || currentProject || "选择项目"}
                    </small>
                  </div>
                  {task && !newConversation && (
                    <Badge variant="outline">
                      {task.mode === "automatic" ? "全自动" : "审批模式"}
                    </Badge>
                  )}
                </div>
                {task?.database_snapshot?.mappings?.length > 0 && (
                  <div className="px-5 py-2 text-sm text-muted-foreground">
                    本会话数据库：
                    {task.database_snapshot.mappings
                      .map(
                        (d: Data) => `${d.alias} → ${d.name} / ${d.database}`,
                      )
                      .join(" · ")}
                    （只读）
                  </div>
                )}
                {task?.database_snapshot?.changed && (
                  <div className="notice">
                    项目连接已变更；续聊仍使用原快照。要使用新映射或新凭据，请新建对话。
                  </div>
                )}
                <div
                  className="messages"
                  ref={messagesRef}
                  onScroll={(e) => {
                    const el = e.currentTarget;
                    following.current =
                      el.scrollHeight - el.scrollTop - el.clientHeight < 64;
                  }}
                >
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
                            ) : (turn.events || []).some((e: Data) =>
                                eventText(e),
                              ) ? (
                              <>
                                {(turn.events || []).map(
                                  (e: Data, i: number) =>
                                    eventText(e) && (
                                      <Markdown key={i} text={eventText(e)} />
                                    ),
                                )}
                              </>
                            ) : (
                              <p className="muted">
                                {active(task)
                                  ? "正在执行；状态每 5 秒更新，不会重复派发任务。"
                                  : "本轮没有文本结果。"}
                              </p>
                            )}
                            {turn.error && (
                              <div className="inline-error">
                                <p>
                                  {/^[\u4e00-\u9fff]/.test(turn.error)
                                    ? turn.error.split("\n")[0]
                                    : "本轮执行失败，请展开诊断查看原因。"}
                                </p>
                                <details>
                                  <summary>诊断详情</summary>
                                  <pre>{turn.error}</pre>
                                </details>
                              </div>
                            )}
                            {(turn.events || []).length > 0 && (
                              <details>
                                <summary>
                                  运行记录 · {turn.events.length} 条
                                </summary>
                                {turn.events.map((e: Data, i: number) => (
                                  <details key={i}>
                                    <summary>{eventLabel(e)}</summary>
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
                      <FolderOpen size={22} />
                      <Select
                        value={draftProject || currentProject || undefined}
                        onValueChange={setDraftProject}
                      >
                        <SelectTrigger aria-label="新任务项目">
                          <SelectValue placeholder="选择项目" />
                        </SelectTrigger>
                        <SelectContent>
                          {projects.map((p) => (
                            <SelectItem key={p.path} value={p.path}>
                              {p.name}
                            </SelectItem>
                          ))}
                        </SelectContent>
                      </Select>
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
                    <Button
                      variant="outline"
                      className="composer-project"
                      title={
                        task && !newConversation
                          ? task.workspace
                          : draftProject || currentProject
                      }
                      onClick={() => {
                        if (!task || newConversation)
                          setUtilityDialog("draft-project");
                      }}
                    >
                      <FolderOpen size={13} />
                      {projects.find(
                        (p) =>
                          p.path ===
                          (task && !newConversation
                            ? task.workspace
                            : draftProject || currentProject),
                      )?.name || "项目"}
                    </Button>
                    <Select value={mode} onValueChange={setMode}>
                      <SelectTrigger className="mode-select">
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
                    <Select
                      value={modelPolicy}
                      onValueChange={(v) => {
                        setModelPolicy(v);
                        setModelEditing(v === "explicit");
                      }}
                    >
                      <SelectTrigger
                        className="model-select"
                        aria-label="下一轮模型"
                      >
                        <span>
                          {modelPolicy === "explicit"
                            ? model.trim() || "指定模型…"
                            : modelPolicy === "remote_default"
                              ? "远端配置"
                              : nextModelLabel(
                                  !newConversation ? task : undefined,
                                  profile.model,
                                )}
                        </span>
                      </SelectTrigger>
                      <SelectContent>
                        <SelectItem value="inherit">
                          {nextModelLabel(
                            !newConversation ? task : undefined,
                            profile.model,
                          )}{" "}
                          · {newConversation ? "账号默认配置" : "当前会话配置"}
                        </SelectItem>
                        <SelectItem value="remote_default">
                          远端配置（不指定模型）
                        </SelectItem>
                        <SelectItem value="explicit">指定其他模型…</SelectItem>
                      </SelectContent>
                    </Select>
                    {modelPolicy === "explicit" &&
                      (modelEditing || !model.trim()) && (
                        <Input
                          className="model-input"
                          onBlur={() => {
                            if (model.trim()) setModelEditing(false);
                          }}
                          value={model}
                          onChange={(e) => setModel(e.target.value)}
                          placeholder="sonnet / opus / 完整 ID"
                        />
                      )}
                    {modelPolicy === "explicit" &&
                      !modelEditing &&
                      model.trim() && (
                        <Button
                          variant="ghost"
                          size="sm"
                          onClick={() => setModelEditing(true)}
                        >
                          修改模型
                        </Button>
                      )}
                  </div>
                  <Textarea
                    aria-label="任务指令"
                    value={draft}
                    onChange={(e) => setDraft(e.target.value)}
                    placeholder={
                      task && !newConversation ? "继续追问…" : "描述任务…"
                    }
                    onKeyDown={(e) => {
                      if ((e.metaKey || e.ctrlKey) && e.key === "Enter") {
                        e.preventDefault();
                        safe(send());
                      }
                    }}
                  />
                  <div className="composer-footer">
                    <span>Shift ↵ 换行</span>
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
                        发送 ⌘ ↵
                      </Button>
                    </div>
                  </div>
                </div>
              </section>
            </div>
          )}
          {page === "databases" && (
            <Databases
              connected={connected}
              projects={projects}
              currentProject={currentProject}
              request={request}
              refreshProjects={refreshProjects}
            />
          )}
          {page === "codex" && (
            <Panel title="Codex MCP">
              <p role="status" className="muted">
                {mcpStatus.message || "正在检查配置…"}
              </p>
              <div className="toolbar">
                <Button
                  disabled={
                    !installLocation.installed || integrationState === "saving"
                  }
                  onClick={() => void configureCodex()}
                >
                  <Cable size={16} />
                  {integrationState === "saving"
                    ? "正在配置…"
                    : "自动写入MCP配置"}
                </Button>
                <Button
                  variant="outline"
                  onClick={() =>
                    safe(
                      navigator.clipboard
                        .writeText(
                          `[mcp_servers.ccbridge]\ncommand = "/Applications/CCBridge.app/Contents/MacOS/ccbridge"\nargs = ["mcp"]\ntool_timeout_sec = 960`,
                        )
                        .then(() => toast.success("配置已复制"))
                        .catch(() => toast.error("复制失败")),
                    )
                  }
                >
                  复制配置
                </Button>
              </div>
              {integration && (
                <p
                  role={integrationState === "error" ? "alert" : "status"}
                  className={
                    integrationState === "error" ? "text-destructive" : "hint"
                  }
                >
                  {integration}
                </p>
              )}
              <details>
                <summary>查看配置片段</summary>
                <pre>{`[mcp_servers.ccbridge]\ncommand = "/Applications/CCBridge.app/Contents/MacOS/ccbridge"\nargs = ["mcp"]\ntool_timeout_sec = 960`}</pre>
              </details>
              <p className="hint">
                写入后在Codex重新加载MCP。配置已保存不代表当前会话已加载工具。
              </p>
            </Panel>
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
                    <p className="muted">{architecture}</p>
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
            </>
          )}
          {page === "settings" && (
            <>
              <Panel title="启动">
                <div className="switch-row">
                  <Switch
                    aria-label="启动时自动连接SSH"
                    checked={preferences.auto_connect === true}
                    onCheckedChange={(v) =>
                      safe(
                        request("preferences_save", { auto_connect: v }).then(
                          () => {
                            setPreferences({ auto_connect: v });
                            toast.success(
                              v
                                ? "已开启：下次启动连接上次账号"
                                : "已关闭自动连接",
                            );
                          },
                        ),
                      )
                    }
                  />
                  <div>
                    <Label>启动App时自动连接SSH</Label>
                    <p className="hint">
                      {profile.user && profile.host
                        ? `${profile.user}@${profile.host} · 需要已保存密码和确认指纹`
                        : "先保存服务器账号"}
                    </p>
                  </div>
                </div>
              </Panel>
              <Panel title="外观">
                <div className="toolbar">
                  {[
                    ["light", "亮色"],
                    ["dark", "暗色"],
                    ["system", "跟随系统"],
                  ].map(([v, label]) => (
                    <Button
                      key={v}
                      variant={theme === v ? "default" : "outline"}
                      onClick={() => setTheme(v)}
                    >
                      {label}
                    </Button>
                  ))}
                </div>
              </Panel>
              <Panel title="凭据">
                <p className="muted">
                  SSH：
                  {profile.remember_password
                    ? (
                        Object.values(saved).find(
                          (p: any) =>
                            p.host === profile.host &&
                            p.port === profile.port &&
                            p.user === profile.user,
                        ) as Data
                      )?.credential_store === "file"
                      ? "本机保存"
                      : "旧凭据待迁移"
                    : "未保存"}
                </p>
                <div className="toolbar">
                  <Button variant="outline" onClick={() => setServerEdit(true)}>
                    管理SSH密码
                  </Button>
                  <Button
                    variant="outline"
                    onClick={() => {
                      setAuthScope("local");
                      setAuthOpen(true);
                    }}
                  >
                    本机Codeup
                  </Button>
                  <Button
                    variant="outline"
                    disabled={!connected}
                    onClick={() => {
                      setAuthScope("remote");
                      setAuthOpen(true);
                    }}
                  >
                    服务器Codeup
                  </Button>
                </div>
              </Panel>
              <Panel title="关于">
                <dl>
                  <dt>版本</dt>
                  <dd>
                    {version} · {architecture}
                  </dd>
                  <dt>数据目录</dt>
                  <dd>{dataDirectory}</dd>
                </dl>
                <Button variant="outline" onClick={() => setPage("updates")}>
                  检查应用更新
                </Button>
              </Panel>
            </>
          )}
        </div>
      </main>
      <Dialog
        open={!!utilityDialog}
        onOpenChange={(v) => !v && setUtilityDialog("")}
      >
        <DialogContent>
          <DialogHeader>
            <DialogTitle>
              {utilityDialog === "folder"
                ? "新建文件夹"
                : utilityDialog === "draft-project"
                  ? "新任务项目"
                  : "添加为项目"}
            </DialogTitle>
            <DialogDescription>
              {utilityDialog === "draft-project"
                ? "仅用于此草稿，不改变MCP默认项目"
                : listedPath || "先选择远端目录"}
            </DialogDescription>
          </DialogHeader>
          {utilityDialog === "draft-project" ? (
            <Select
              value={draftProject || currentProject || undefined}
              onValueChange={(v) => {
                setDraftProject(v);
                setUtilityDialog("");
              }}
            >
              <SelectTrigger>
                <SelectValue placeholder="选择项目" />
              </SelectTrigger>
              <SelectContent>
                {projects.map((p) => (
                  <SelectItem value={p.path} key={p.path}>
                    {p.name}
                  </SelectItem>
                ))}
              </SelectContent>
            </Select>
          ) : (
            <>
              <Input
                aria-label="名称"
                value={utilityDialog === "folder" ? folderName : projectName}
                onChange={(e) =>
                  utilityDialog === "folder"
                    ? setFolderName(e.target.value)
                    : setProjectName(e.target.value)
                }
              />
              <DialogFooter>
                <Button
                  disabled={
                    !locationReady ||
                    !(
                      utilityDialog === "folder" ? folderName : projectName
                    ).trim()
                  }
                  onClick={() =>
                    safe(
                      (utilityDialog === "folder"
                        ? request("create_directory", {
                            path: listedPath,
                            name: folderName,
                          })
                        : request("project_save", {
                            path: listedPath,
                            workspace: listedPath,
                            name: projectName,
                          })
                      ).then(async () => {
                        await browse(listedPath);
                        await refreshProjects();
                        setUtilityDialog("");
                        setFolderName("");
                        setProjectName("");
                      }),
                    )
                  }
                >
                  确认
                </Button>
              </DialogFooter>
            </>
          )}
        </DialogContent>
      </Dialog>
      <CodeupAuth
        key={`${connection.profile?.host}:${connection.profile?.user}:${connection.profile?.port}`}
        open={authOpen}
        onOpenChange={setAuthOpen}
        connected={connected}
        account={
          connected
            ? `${connection.profile?.user}@${connection.profile?.host}:${connection.profile?.port}`
            : ""
        }
        url={cloneUrls.trim().split(/\s+/)[0] || ""}
        initialScope={authScope}
        consented={storageConsent}
        onConsent={() => setStorageConsent(true)}
        onSaved={
          cloneOpen && cloneError
            ? () => {
                void cloneRepositories();
              }
            : undefined
        }
      />
      <FileOperation
        key={`${fileOperation?.entry.path}:${fileOperation?.action}`}
        operation={fileOperation}
        onClose={() => setFileOperation(null)}
        onDone={async () => {
          await browse(listedPath);
        }}
        home={connection.environment?.home || ""}
      />
      <TrashDialog
        open={trashOpen}
        onClose={() => setTrashOpen(false)}
        onDone={async () => {
          await browse(listedPath);
        }}
      />
      <BranchDialog
        key={listedPath}
        open={branchesOpen}
        onClose={() => setBranchesOpen(false)}
        path={listedPath}
      />
      <Toaster
        theme={theme === "system" ? "system" : (theme as "light" | "dark")}
        richColors
        position="bottom-right"
      />
      <Dialog open={serverEdit} onOpenChange={setServerEdit}>
        <DialogContent className="server-dialog">
          <DialogHeader>
            <DialogTitle>SSH 服务器</DialogTitle>
            <DialogDescription>保存你的SSH账号。</DialogDescription>
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
                ? "已保存，留空即可"
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
            <Label>记住密码</Label>
          </div>
          {profile.remember_password && (
            <>
              <p className="hint">密码保存在本机，仅当前Mac账号可访问。</p>
              {profile.credential_store === "file" && !storageConsent && (
                <label className="storage-warning">
                  <input
                    type="checkbox"
                    checked={storageAck}
                    onChange={(e) => setStorageAck(e.target.checked)}
                  />
                  我了解：本机保存不会弹钥匙串授权，但当前Mac账号下的其他程序可能读取密码。
                </label>
              )}
            </>
          )}
          <details>
            <summary>高级设置 · 通常留空</summary>
            <Field label="Runner 路径">{input("runner")}</Field>
            <Field label="Claude Code 路径">{input("claude")}</Field>
          </details>
          {(errors.migrate_password ||
            errors.save_profile ||
            errors.connect_ssh) && (
            <p role="alert" className="text-destructive">
              {String(
                (
                  errors.migrate_password ||
                  errors.save_profile ||
                  errors.connect_ssh
                )?.message ||
                  errors.save_profile ||
                  errors.connect_ssh,
              )}
            </p>
          )}
          {profile.credential_store === "file" &&
            Object.values(saved).some(
              (p: any) =>
                p.host === profile.host &&
                p.port === profile.port &&
                p.user === profile.user &&
                p.remember_password &&
                p.credential_store !== "file",
            ) && (
              <div className="migration">
                <Button
                  disabled={
                    busy.has("migrate_password") ||
                    (!storageConsent && !storageAck)
                  }
                  variant="outline"
                  onClick={() =>
                    safe(
                      request("migrate_password", {
                        account_key: JSON.stringify([
                          profile.host,
                          profile.port,
                          profile.user,
                        ]),
                        storage_ack: storageAck,
                      }).then((r) => {
                        if (r) {
                          setProfile({
                            ...r.profile,
                            credential_store: "file",
                          });
                          setStorageConsent(true);
                          setPassword("");
                          setSaved((x) => ({
                            ...x,
                            [JSON.stringify([
                              r.profile.host,
                              r.profile.port,
                              r.profile.user,
                            ])]: r.profile,
                          }));
                          toast.success(r.message);
                          setServerEdit(false);
                        }
                      }),
                    )
                  }
                >
                  {busy.has("migrate_password")
                    ? "正在迁移…"
                    : "迁移此账号已保存的密码"}
                </Button>
                <p className="hint">
                  只读取此账号的钥匙串密码并保存到本机，原副本保留；macOS可能要求一次授权。
                </p>
              </div>
            )}
          <DialogFooter>
            <Button
              variant="outline"
              disabled={busy.has("save_profile")}
              onClick={() =>
                safe(
                  request("save_profile", {
                    profile,
                    password,
                    storage_ack: storageAck,
                  }).then((r) => {
                    if (r) {
                      setPassword("");
                      setSaved((x) => ({
                        ...x,
                        [JSON.stringify([
                          profile.host,
                          profile.port,
                          profile.user,
                        ])]: profile,
                      }));
                      if (profile.credential_store === "file")
                        setStorageConsent(true);
                      toast.success("连接配置已保存");
                      setServerEdit(false);
                    }
                  }),
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
      <Dialog
        open={cloneOpen}
        onOpenChange={(open) => {
          if (!busy.has("clone_repository")) setCloneOpen(open);
        }}
      >
        <DialogContent>
          <DialogHeader>
            <DialogTitle>克隆仓库</DialogTitle>
            <DialogDescription>
              仓库将克隆到 {path}。每行一个 HTTPS 地址，已有目录不会覆盖。
            </DialogDescription>
          </DialogHeader>
          <Textarea
            value={cloneUrls}
            disabled={busy.has("clone_repository")}
            onChange={(e) => setCloneUrls(e.target.value)}
            placeholder="https://github.com/owner/repository.git"
          />
          <Label>获取方式</Label>
          <Select
            value={localAuth ? "local" : "remote"}
            onValueChange={(v) => setLocalAuth(v === "local")}
            disabled={busy.has("clone_repository")}
          >
            <SelectTrigger>
              <SelectValue />
            </SelectTrigger>
            <SelectContent>
              <SelectItem value="local">本机获取，再通过SSH传输</SelectItem>
              <SelectItem value="remote">服务器直接获取</SelectItem>
            </SelectContent>
          </Select>
          <Button
            variant="outline"
            disabled={busy.has("clone_repository")}
            onClick={() => {
              setAuthScope(localAuth ? "local" : "remote");
              setAuthOpen(true);
            }}
          >
            配置 Codeup 认证 / 检测仓库访问
          </Button>
          {(busy.has("clone_repository") ||
            cloneProgress.stage ||
            cloneError) && (
            <div className="space-y-2" aria-live="polite">
              {busy.has("clone_repository") && (
                <ProgressPrimitive.Root
                  value={null}
                  aria-label="仓库克隆进行中"
                  className="h-2 w-full overflow-hidden rounded-full bg-primary/15"
                >
                  <ProgressPrimitive.Indicator className="h-full w-1/3 animate-pulse rounded-full bg-primary" />
                </ProgressPrimitive.Root>
              )}
              {!cloneError && (
                <p role="status">
                  {cloneProgress.stage}
                  {busy.has("clone_repository") &&
                    ` · ${Math.floor(cloneElapsed / 60)}:${String(cloneElapsed % 60).padStart(2, "0")}`}
                  {cloneProgress.total > 0 &&
                    ` · 已处理 ${cloneProgress.completed}/${cloneProgress.total} 个仓库`}
                </p>
              )}
              {cloneError && (
                <p role="alert" className="text-destructive">
                  {cloneError}
                </p>
              )}
              {busy.has("clone_repository") && (
                <p className="hint">
                  正在执行，请勿重复提交；本机 Git 认证可能需要系统钥匙串授权。
                </p>
              )}
            </div>
          )}
          {cloneResults.map((r) => (
            <p key={r.path} className="hint">
              {r.name} ·{" "}
              {r.state === "already_exists" ? "已存在，未改动" : "已克隆"}
            </p>
          ))}
          <DialogFooter>
            <Button
              disabled={busy.has("clone_repository")}
              variant="outline"
              onClick={() => setCloneOpen(false)}
            >
              取消
            </Button>
            <Button
              disabled={!cloneUrls.trim() || busy.has("clone_repository")}
              onClick={() => void cloneRepositories()}
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
