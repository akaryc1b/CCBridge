import { maintenanceLabel } from "./lib/runnerMaintenance";
import { ProjectPolicy } from "./components/ProjectPolicy";
import { TaskQueue } from "./components/TaskQueue";
import {
  queueLabel,
  shouldRefreshQueued,
  taskCenterRecord,
  shouldRefreshTaskList,
} from "./lib/taskQueue";
import { ProjectMenu } from "./components/ProjectMenu";
import { ProjectDialog } from "./components/ProjectDialog";
import { projectAtPath } from "./lib/projectEntries";
import { useTaskCenter } from "./lib/useTaskCenter";
import { useSessions } from "./lib/useSessions";
import { SessionList } from "./components/SessionList";
import {
  BackgroundTasks,
  type BackgroundTask,
} from "./components/BackgroundTasks";
import { taskAction } from "./lib/taskRead";
import { operationSummary } from "./lib/backgroundOperation";
import { AppIcon } from "@/components/AppIcon";
import { useClaudeInfo } from "@/lib/useClaudeInfo";
import {
  ClaudeUsage,
  RemoteModelSelect,
  modelLabel,
} from "@/components/ClaudeInfo";
import {
  OperationBar,
  InlineAlert,
  StatusRow,
} from "@/components/common/Feedback";
import { Confirm } from "@/components/common/Confirm";
import { useOperation } from "@/lib/useOperation";
import { time, elapsed, bytes, phaseText } from "@/lib/format";
import { GitUpdate } from "@/components/GitUpdate";
import {
  isConnecting,
  clearConnectionErrors,
  hostKeyCandidate,
} from "@/lib/connection";
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
  nextModelLabel,
  displayModel,
  eventText,
  taskFailure,
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
  !t.ownership_unknown &&
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
      {(title || description) && (
        <CardHeader>
          {title && <CardTitle>{title}</CardTitle>}
          {description && <CardDescription>{description}</CardDescription>}
        </CardHeader>
      )}
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
        components={{
          table: ({ children }) => (
            <div className="markdown-table">
              <table>{children}</table>
            </div>
          ),
        }}
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
    [modelEdit, setModelEdit] = useState(false),
    [modelDraft, setModelDraft] = useState("");
  const [autoConfirm, setAutoConfirm] = useState(false),
    [hostKey, setHostKey] = useState<{
      fingerprint: string;
      profile: Data;
      ack: boolean;
      secret: string;
    } | null>(null);
  const [projects, setProjects] = useState<Data[]>([]),
    [listing, setListing] = useState<Data>({ entries: [] }),
    [path, setPath] = useState(""),
    [browsing, setBrowsing] = useState(false),
    [preview, setPreview] = useState(""),
    [cloneOpen, setCloneOpen] = useState(false),
    [cloneUrls, setCloneUrls] = useState(""),
    [localAuth, setLocalAuth] = useState(false),
    [folderName, setFolderName] = useState("");
  const [tasks, setTasks] = useState<Data[]>([]),
    [selected, setSelected] = useState(""),
    [details, setDetails] = useState<Data>({}),
    [draft, setDraft] = useState(""),
    [mode, setMode] = useState("automatic"),
    [accessChoice, setAccessChoice] = useState("inherit"),
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
  const [authOpen, setAuthVisible] = useState(false),
    [authScope, setAuthScope] = useState("local"),
    [fileOperation, setFileOperation] = useState<Data | null>(null),
    [trashOpen, setTrashOpen] = useState(false),
    [branchesOpen, setBranchesVisible] = useState(false);
  const [cloneResults, setCloneResults] = useState<Data[]>([]),
    [cloneElapsed, setCloneElapsed] = useState(0);
  const cloneRetry = useRef<{ signature: string; urls: string[] } | null>(null);
  const [mcpStatus, setMcpStatus] = useState<Data>({});
  const [theme, setTheme] = useState(
    localStorage.getItem("ccbridge-theme") || "system",
  );
  const [gitUpdateOpen, setGitUpdateVisible] = useState(false);
  const [gitDialogPath, setGitDialogPath] = useState("");
  const [branchDialogPath, setBranchDialogPath] = useState("");
  const [gitCenterId, setGitCenterId] = useState<string>();
  const [branchCenterId, setBranchCenterId] = useState<string>();
  const [gitOperation, setGitOperation] = useState<Data | null>(null);
  const [branchOperation, setBranchOperation] = useState<Data | null>(null);
  const [authActivity, setAuthActivity] = useState("");
  const [authOperationId, setAuthOperationId] = useState("");
  const [authRequestVersion, setAuthRequestVersion] = useState(0);
  const [cloneId, setCloneId] = useState<string>();
  const [installId, setInstallId] = useState<string>();
  const [updateTransfer, setUpdateTransfer] = useState<Data>({});
  const [updateStarted, setUpdateStarted] = useState(0);
  const [localConfirm, setLocalConfirm] = useState<null | { action: string }>(
    null,
  );
  const [previewName, setPreviewName] = useState("");
  const [tick, setTick] = useState(Date.now());
  useEffect(() => {
    if (!tasks.some(active) && updatePhase !== "downloading") return;
    const t = setInterval(() => setTick(Date.now()), 1000);
    return () => clearInterval(t);
  }, [tasks.some(active), updatePhase]);

  const [authUrl, setAuthUrl] = useState("");
  const [preferences, setPreferences] = useState<Data>({});
  const [autoTarget, setAutoTarget] = useState<Data>({});
  const [installLocation, setInstallLocation] = useState<Data>({
    installed: true,
  });
  const [storageAck, setStorageAck] = useState(false);
  const [storageConsent, setStorageConsent] = useState(false);
  const [utilityDialog, setUtilityDialog] = useState("");
  const [draftProject, setDraftProject] = useState("");
  const [fileFilter, setFileFilter] = useState("");

  const [dataDirectory, setDataDirectory] = useState("");
  const autoStarted = useRef(false);
  const composerStates = useRef<Record<string, Data>>({});
  useEffect(() => {
    const media = matchMedia("(prefers-color-scheme: dark)");
    const root = document.documentElement;
    let disposed = false,
      frame = 0;
    // Change the palette atomically: native appearance changes can interrupt WebKit color transitions.
    root.classList.add("theme-changing");
    const apply = () => {
      root.dataset.theme =
        theme === "system" ? (media.matches ? "dark" : "light") : theme;
      root.classList.toggle("dark", root.dataset.theme === "dark");
    };
    const finish = () => {
      if (disposed) return;
      void getComputedStyle(root).color;
      frame = requestAnimationFrame(() => {
        if (!disposed) root.classList.remove("theme-changing");
      });
    };
    apply();
    localStorage.setItem("ccbridge-theme", theme);
    if (isTauri())
      void invoke("set_window_theme", { theme })
        .catch((e) => toast.error(String(e)))
        .finally(finish);
    else finish();
    media.addEventListener("change", apply);
    return () => {
      disposed = true;
      cancelAnimationFrame(frame);
      root.classList.remove("theme-changing");
      media.removeEventListener("change", apply);
    };
  }, [theme]);
  function startDraft(project: string) {
    switchConversation("");
    setDraftProject(project);
    setPage("chat");
  }
  const [projectEntry, setProjectEntry] = useState<{
    path: string;
    currentOnly: boolean;
    connectionId: string;
  } | null>(null);
  function openProject(currentOnly = false) {
    setProjectEntry({
      path: currentOnly ? listedPath : connection.environment?.home || "",
      currentOnly,
      connectionId: connection.connection_id || "",
    });
  }
  const drafts = useRef<Record<string, string>>({});
  const messagesRef = useRef<HTMLDivElement>(null),
    following = useRef(true);

  const submission = useRef<{ signature: string; id: string } | null>(null);
  const detailsRef = useRef(details);
  detailsRef.current = details;

  function switchConversation(id: string) {
    setAccessChoice("inherit");
    setErrors((old) => {
      const next = { ...old };
      delete next.task_start;
      delete next.task_reply;
      return next;
    });
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
  const connectionIdentity = useRef(connection.connection_id);
  connectionIdentity.current = connection.connection_id;
  const requestSequence = useRef(new Map<string, number>());
  const inflight = useRef(new Map<string, number>()),
    selectedRef = useRef(selected);
  selectedRef.current = selected;
  const connected = connection.connected === true,
    task = details[selected] || tasks.find((t) => t.task_id === selected),
    runner = connection.probe || {};
  const claudeInfoSupported =
    connected && runner.capabilities?.includes("claude_info_v1");
  const claudeInfo = useClaudeInfo(
    connected ? connection.connection_id : "",
    !!claudeInfoSupported,
  );
  useEffect(() => {
    if (["servers", "overview"].includes(page) && connected)
      void claudeInfo.refreshUsage();
  }, [page, task?.updated_at]);
  const selectedModelId =
    modelPolicy === "explicit"
      ? model
      : modelPolicy === "inherit"
        ? (!newConversation && (task?.model_requested || task?.model)) ||
          profile.model ||
          ""
        : "";
  const invalidModel =
    !!selectedModelId &&
    !claudeInfo.catalog.loading &&
    claudeInfo.catalog.models.length > 0 &&
    !claudeInfo.catalog.models.some((m) => m.value === selectedModelId);
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
  useEffect(() => {
    setGitOperation(connection.operations?.git_update || null);
  }, [connection.connection_id, connection.operations?.git_update?.revision]);
  useEffect(() => {
    setBranchOperation(connection.operations?.branch_fetch || null);
  }, [connection.connection_id, connection.operations?.branch_fetch?.revision]);
  const currentProject =
    connection.profile?.active_remote_workspace ||
    profile.active_remote_workspace ||
    "";
  const cloneOperation = useOperation(
    "clone_into_directory",
    cloneProgress.path || path,
    connection.connection_id || "",
    !!cloneId || cloneOpen,
    cloneId,
  );
  const installOperation = useOperation(
    "install_runner",
    "",
    connection.connection_id || "",
    !!installId,
    installId,
  );
  const taskCenter = useTaskCenter(connection);
  const backgroundTasks: BackgroundTask[] = [];
  for (const op of taskCenter.operations) {
    const labels: Data = {
      git_update: "Git 更新",
      clone_into_directory: "克隆仓库",
      branch_fetch: "获取分支",
      install_runner: "安装服务端程序",
      git_access: "仓库访问检测",
      git_vps_auth_test: "服务器认证检测",
      git_vps_auth_install: "同步服务器认证",
      git_vps_auth_remove: "移除服务器认证",
    };
    backgroundTasks.push({
      id: op.id,
      local: op.account === "local",
      label: labels[op.kind] || "后台任务",
      kind: labels[op.kind] || "后台任务",
      project:
        op.account === "local"
          ? "本机 Git"
          : projects.find(
              (p) => op.path === p.path || op.path?.startsWith(p.path + "/"),
            )?.name || op.path?.split("/").pop(),
      workspace: op.path,
      phase: phaseText[op.phase] || op.phase,
      started: op.started_at_ms,
      finished: op.finished_at_ms,
      state: op.remote_lease && op.state !== "running" ? "unknown" : op.state,
      ownership_unknown: !!op.remote_lease && op.state !== "running",
      release: op.remote_lease
        ? async () => {
            await invoke("operation_release", {
              request: {
                operation_id: op.id,
                confirmed: true,
              },
            });
          }
        : undefined,
      running: op.state === "running",
      summary: operationSummary(op),
      error: op.error,
      verification: op.verification,
      verifiedAt: op.verified_at,
      processed: op.processed,
      total: op.total,
      action:
        op.account === "local"
          ? "查看本机认证"
          : op.connection_id !== connection.connection_id
            ? "检查当前状态"
            : op.kind === "clone_into_directory" && op.id !== cloneId
              ? "查看目标目录"
              : op.kind.startsWith("git_vps_auth") || op.kind === "git_access"
                ? "查看认证"
                : taskAction(op.kind),
      open: () => {
        if (op.account === "local") {
          setAuthScope("local");
          setAuthOpen(true);
          return;
        }
        if (op.connection_id !== connection.connection_id) {
          safe(
            request("operation_verify", {
              operation_id: op.id,
              connection_id: connection.connection_id,
            }).then((r) => {
              if (r) toast.info(r.message);
            }),
          );
          return;
        }
        if (op.kind === "git_update") {
          setGitUpdateOpen(true, op.path, op.id);
        } else if (op.kind === "branch_fetch") {
          setBranchesOpen(true, op.path, op.id);
        } else if (op.kind === "clone_into_directory") {
          if (op.id === cloneId) setCloneOpen(true);
          else {
            setPage("projects");
            safe(browse(op.path));
          }
        } else if (op.kind.startsWith("git_")) {
          setAuthOpen(true, true);
        } else setPage("servers");
      },
    });
  }

  if (
    authActivity &&
    authOperationId &&
    !authOpen &&
    !taskCenter.operations.some((o) => o.id === authOperationId)
  )
    backgroundTasks.push({
      id: authOperationId,
      label: "Codeup 认证",
      summary: authActivity,
      running: authActivity === "认证处理中…",
      state:
        authActivity === "认证处理中…"
          ? "running"
          : authActivity === "认证操作失败"
            ? "failed"
            : "done",
      action: "查看认证",
      open: () => setAuthOpen(true, true),
    });
  if (
    gitOperation?.id &&
    gitOperation.connection_id === connection.connection_id &&
    !gitUpdateOpen
  )
    backgroundTasks.push({
      id: gitOperation.id,
      label:
        gitOperation.source === "mcp" ? "Git 更新 · 来自 Codex" : "Git 更新",
      summary: operationSummary(gitOperation),
      running: gitOperation.state === "running",
      state: gitOperation.state,
      finished: gitOperation.finished_at_ms,
      action: "查看更新结果",
      open: () => {
        setPage("projects");
        setGitUpdateOpen(true, gitOperation.path);
      },
    });
  if (
    branchOperation?.id &&
    branchOperation.connection_id === connection.connection_id &&
    !branchesOpen
  )
    backgroundTasks.push({
      id: branchOperation.id,
      label: "获取分支",
      summary: operationSummary(branchOperation),
      running: branchOperation.state === "running",
      state: branchOperation.state,
      finished: branchOperation.finished_at_ms,
      action: "查看分支",
      open: () => {
        setPage("projects");
        setBranchesOpen(true, branchOperation.path);
      },
    });
  if (cloneOperation.snap?.id && !cloneOpen)
    backgroundTasks.push({
      id: cloneOperation.snap.id,
      label: "克隆仓库",
      summary: operationSummary(cloneOperation.snap),
      running: cloneOperation.snap.state === "running",
      state: cloneOperation.snap.state,
      finished: cloneOperation.snap.finished_at_ms,
      action: "查看克隆进度",
      open: () => setCloneOpen(true),
    });
  const connecting = isConnecting(busy);
  const request = async (
    command: string,
    args: Data = {},
    quiet = false,
  ): Promise<any> => {
    const requestKey =
      command === "task_status" ? `${command}:${args.task_id}` : command;
    if (
      inflight.current.has(requestKey) &&
      !["directory_list", "file_preview"].includes(command)
    )
      throw new Error("该操作正在进行，请查看当前状态");
    const sequence = (requestSequence.current.get(requestKey) || 0) + 1;
    requestSequence.current.set(requestKey, sequence);
    inflight.current.set(
      requestKey,
      (inflight.current.get(requestKey) || 0) + 1,
    );
    setBusy(new Set(inflight.current.keys()));
    try {
      const data: any = await invoke(command, { request: args });
      if (data?.auto_target) setAutoTarget(data.auto_target);
      if (command === "connect_ssh" && data?.connected) {
        setAutoTarget(data.profile || {});
        setErrors(clearConnectionErrors);
      }
      if (requestSequence.current.get(requestKey) === sequence)
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
      if (
        hostKeyCandidate(e) &&
        ["connect_ssh", "auto_connect"].includes(command)
      ) {
        setErrors(clearConnectionErrors);
      } else if (requestSequence.current.get(requestKey) === sequence) {
        setErrors((old) => ({
          ...old,
          [command]: {
            page: command === "auto_connect" ? "servers" : page,
            message,
          },
        }));
        // Render once beside the affected page; callers with inline errors own their feedback.
      }
      throw e;
    } finally {
      const left = (inflight.current.get(requestKey) || 1) - 1;
      if (left) inflight.current.set(requestKey, left);
      else inflight.current.delete(requestKey);
      setBusy(new Set(inflight.current.keys()));
    }
  };
  async function configureCodex() {
    if (integrationState === "saving") return;
    setIntegrationState("saving");
    setIntegration("");
    try {
      const message = await invoke<string>("configure_codex");
      setIntegration("已写入 · 需在 Codex 重新加载");
      setIntegrationState("success");
      invoke<Data>("codex_configuration_status")
        .then(setMcpStatus)
        .catch(() => {});
    } catch (e) {
      const message =
        typeof e === "object" && e !== null && "message" in e
          ? String((e as Data).message)
          : String(e);
      setIntegration(`写入失败：${message}`);
      setIntegrationState("error");
    }
  }
  function setAuthOpen(open: boolean, resume = false) {
    if (open && !resume) setAuthRequestVersion((v) => v + 1);
    setAuthVisible(open);
  }
  function setGitUpdateOpen(
    open: boolean,
    target = listedPath,
    operationId?: string,
  ) {
    if (open) setGitCenterId(operationId);
    if (
      open &&
      !operationId &&
      gitOperation?.state === "running" &&
      gitOperation.path !== target
    ) {
      toast.info("另一目录的 Git 更新正在进行，已打开该操作");
      target = gitOperation.path;
    }
    if (open) setGitDialogPath(target);
    setGitUpdateVisible(open);
  }
  function setBranchesOpen(
    open: boolean,
    target = listedPath,
    operationId?: string,
  ) {
    if (open) setBranchCenterId(operationId);
    if (
      open &&
      !operationId &&
      branchOperation?.state === "running" &&
      branchOperation.path !== target
    ) {
      toast.info("另一目录的分支获取正在进行，已打开该操作");
      target = branchOperation.path;
    }
    if (open) setBranchDialogPath(target);
    setBranchesVisible(open);
  }
  async function cloneRepositories() {
    setCloneError("");
    setCloneProgress({ stage: "检查目录", path: listedPath });
    const operationId = crypto.randomUUID();
    setCloneId(operationId);
    const destination = cloneProgress.path || listedPath;
    const cloneConnection = connection.connection_id;
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
        operation_id: operationId,
        connection_id: connection.connection_id,
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
        stage: `已处理 ${r.repositories?.length || 0} 个仓库`,
        path: destination,
      });

      if (
        locationRef.current.path === destination &&
        locationRef.current.connection === cloneConnection
      )
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
      if (c.connected) setErrors(clearConnectionErrors);
      if (c.profile)
        setProfile((p) => ({
          ...p,
          allow_auto: c.profile.allow_auto,
          active_remote_workspace: c.profile.active_remote_workspace,
        }));
    }
    return c;
  }
  const sessionManager = useSessions(
    connection,
    (command, args) => invoke(command, { request: args || {} }),
    tasks.map((t) => `${t.task_id}:${t.state}`).join("|"),
  );
  const selectedSummary = tasks.find((t) => t.session_id === task?.session_id);
  const selectedSession =
    sessionManager.metadata[task?.session_id] ||
    (selectedSummary?.session_meta
      ? { ...selectedSummary, ...selectedSummary.session_meta }
      : undefined);
  const groupSessionsFallback = tasks.map((t) => (
    <button key={t.task_id} onClick={() => switchConversation(t.task_id)}>
      {t.title || t.prompt}
    </button>
  ));
  async function refreshTasks() {
    const identity = connectionIdentity.current;
    const r = await request("task_list", {}, true);
    if (r && identity === connectionIdentity.current)
      setTasks((r.tasks || []).map(normalizeTask));
  }
  async function refreshProjects() {
    const identity = connectionIdentity.current;
    const r = await request("project_list", {}, true);
    if (r && identity === connectionIdentity.current)
      setProjects(r.projects || []);
  }
  function openFileOperation(operation: Data) {
    if (!connected || !connection.connection_id) return;
    setFileOperation({ ...operation, connectionId: connection.connection_id });
  }
  useEffect(() => {
    setFileOperation(null);
    setTrashOpen(false);
  }, [connected, connection.connection_id]);
  const [removingProject, setRemovingProject] = useState(false);
  const removeLock = useRef(false);
  async function removeProject(project: Data) {
    if (removeLock.current) return;
    if (!runner.capabilities?.includes("project_remove_v1")) {
      toast.error("更新服务端程序后可移除项目");
      setPage("servers");
      return;
    }
    const identity = connectionIdentity.current;
    removeLock.current = true;
    setRemovingProject(true);
    try {
      const result = await invoke<Data>("project_remove", {
        request: {
          workspace: project.path,
          expected_name: project.name,
          connection_id: identity,
        },
      });
      if (identity !== connectionIdentity.current) return;
      setProjects(result.projects || []);
      if (sessionManager.project === project.path)
        sessionManager.setProject("");
      if (
        (!task || newConversation) &&
        (draftProject || currentProject) === project.path
      )
        setDraftProject(connection.environment?.home || "");
      toast.success("已移除项目，文件和会话保留", {
        duration: 10000,
        action: {
          label: "撤销",
          onClick: () => {
            if (identity !== connectionIdentity.current) {
              toast.error("连接已变化，请在原账号重新添加项目");
              return;
            }
            invoke("project_save", {
              request: {
                workspace: project.path,
                path: project.path,
                name: project.name,
                create_only: true,
                connection_id: identity,
              },
            })
              .then(() => refreshProjects())
              .catch((e) => toast.error(e.message || String(e)));
          },
        },
      });
    } catch (e: any) {
      toast.error(e.message || String(e));
    } finally {
      removeLock.current = false;
      setRemovingProject(false);
    }
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
  const locationRef = useRef({
    path: listedPath,
    connection: connection.connection_id,
  });
  locationRef.current = {
    path: listedPath,
    connection: connection.connection_id,
  };
  const visibleEntries = (listing.entries || []).filter((entry: Data) =>
    entry.name.toLowerCase().includes(fileFilter.toLowerCase()),
  );
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
    const account = connection.connection_id;
    while (currentId && !seen.has(currentId) && seen.size < 20) {
      seen.add(currentId);
      const r = await request(
        "task_status",
        {
          task_id: currentId,
          cursor: detailsRef.current[currentId]?.next_cursor || 0,
        },
        true,
      );
      if (!r || account !== connectionIdentity.current) break;
      const key = currentId;
      setTasks((old) =>
        old.map((t) =>
          t.task_id === key
            ? {
                ...t,
                state: r.state,
                updated_at: r.updated_at,
                access: r.access,
                blocked_reason: r.blocked_reason,
                queue: r.queue,
                ownership_unknown: r.ownership_unknown,
                model_resolved: r.model_resolved,
              }
            : t,
        ),
      );
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
        setAutoTarget(s.profile || {});
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
              setAutoTarget(result.profile || {});
              // Do not replace an editor draft while startup connection is in flight.
            }
          } catch (e) {
            const fingerprint = hostKeyCandidate(e);
            if (fingerprint)
              setHostKey({
                fingerprint,
                profile: s.profile,
                ack: s.local_storage_ack === true,
                secret: "",
              });
          }
        }
      })(),
    );
  }, []);
  useEffect(() => {
    const timer = setInterval(() => safe(refreshConnection()), 5000);
    return () => clearInterval(timer);
  }, []);
  useEffect(() => {
    if (!connected) return;
    safe(refreshConnection());
    safe(refreshTasks());
    safe(refreshProjects());
    const timer = setInterval(() => {
      safe(refreshTasks());
      if (document.hidden) return;
      safe(refreshProjects());
      if (selectedRef.current) safe(fetchTask(selectedRef.current));
    }, 5000);
    return () => clearInterval(timer);
  }, [connected, runner.runner_version, connection.connection_id]);
  useEffect(() => {
    if (
      connected &&
      shouldRefreshTaskList(tasks, taskCenter.tasks) &&
      !inflight.current.has("task_list")
    )
      safe(refreshTasks());
  }, [connected, taskCenter.tasks]);
  useEffect(() => {
    if (selected && connected) safe(fetchTask(selected));
  }, [selected, connected]);
  useEffect(() => {
    if (
      connected &&
      selected &&
      shouldRefreshQueued(detailsRef.current[selected], taskCenter.tasks)
    )
      safe(fetchTask(selected));
  }, [connected, selected, taskCenter.tasks]);
  useEffect(() => {
    if (!isTauri()) return;
    let cleanup: (() => void) | undefined;
    listen<Data>("update-progress", (e) => {
      setUpdateTransfer(e.payload);
      setProgress(
        e.payload.total
          ? Math.round((e.payload.downloaded / e.payload.total) * 100)
          : 0,
      );
    })
      .then((f) => (cleanup = f))
      .catch((e) => toast.error(String(e)));
    return () => cleanup?.();
  }, []);
  useEffect(() => {
    if (!isTauri()) return;
    let cleanup: (() => void) | undefined;
    let disposed = false;
    listen<Data>("backend-event", (e) => {
      if (
        e.payload.response?.ok === false &&
        !hostKeyCandidate(e.payload.response.error)
      )
        setErrors((x) => ({
          ...x,
          backend: { page: "servers", message: e.payload.response.error },
        }));
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
  async function saveServer(ack = storageAck) {
    if (password && !storageConsent && !ack) {
      setLocalConfirm({ action: "save" });
      return;
    }
    const r = await request("save_profile", {
      profile,
      password,
      storage_ack: ack,
    });
    if (r) {
      setPassword("");
      setSaved((x) => ({
        ...x,
        [JSON.stringify([profile.host, profile.port, profile.user])]: profile,
      }));
      if (ack) setStorageConsent(true);
      setServerEdit(false);
    }
  }
  async function migrateServer(ack = storageAck) {
    if (!storageConsent && !ack) {
      setLocalConfirm({ action: "migrate" });
      return;
    }
    const r = await request("migrate_password", {
      account_key: JSON.stringify([profile.host, profile.port, profile.user]),
      storage_ack: ack,
    });
    if (r) {
      setProfile(r.profile);
      setStorageConsent(true);
      setPassword("");
      setSaved((x) => ({
        ...x,
        [JSON.stringify([profile.host, profile.port, profile.user])]: r.profile,
      }));
      setServerEdit(false);
    }
  }
  async function connect(
    nextProfile = profile,
    ack = storageAck,
    secret = password,
  ) {
    if (secret && !storageConsent && !ack) {
      setLocalConfirm({ action: "connect" });
      return;
    }
    try {
      const c = await request("connect_ssh", {
        profile: nextProfile,
        password: secret,
        storage_ack: ack,
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
        if (ack) setStorageConsent(true);
      }
    } catch (e) {
      const fingerprint = hostKeyCandidate(e);
      if (fingerprint)
        setHostKey({ fingerprint, profile: nextProfile, ack, secret });
    }
  }
  async function send() {
    const prompt = draft.trim();
    if (
      (task && !newConversation && !task.session_id) ||
      (selectedSession?.archived && !newConversation) ||
      !prompt ||
      !connected ||
      busy.has("task_start") ||
      busy.has("task_reply") ||
      (task && !newConversation && active(task)) ||
      (mode === "automatic" && !profile.allow_auto) ||
      (modelPolicy === "explicit" && !model.trim()) ||
      invalidModel
    )
      return;
    const continuing = task && !newConversation;
    const args: Data = {
      prompt,
      task_id: "",
      mode,
      ...(runner.capabilities?.includes("analysis_v1")
        ? {
            access: connection.policy?.read_only
              ? "analysis"
              : accessChoice === "inherit"
                ? continuing
                  ? task.access || "write"
                  : "analysis"
                : accessChoice,
          }
        : {}),
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
      setAccessChoice("inherit");
      setSelected(r.task_id);
      setDetails((d) => ({ ...d, [r.task_id]: r }));
      await refreshTasks();
    }
  }
  async function checkUpdate() {
    setErrors((x) => {
      const next = { ...x };
      delete next.update;
      return next;
    });
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
    setUpdateStarted(Date.now());
    setTick(Date.now());
    setUpdateTransfer({});
    setProgress(0);
    setErrors((x) => {
      const next = { ...x };
      delete next.update;
      return next;
    });
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
      key !== "update" &&
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
    <div
      className={`app ${navigator.platform.toLowerCase().includes("mac") ? "mac-chrome" : ""}`}
    >
      <aside className="sidebar">
        <div className="window-drag-space" data-tauri-drag-region />
        <div className="brand" data-tauri-drag-region>
          <AppIcon />
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
              {connecting
                ? "连接中…"
                : connected
                  ? `${connection.profile?.user}@${connection.profile?.host}`
                  : "未连接"}
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
        <header data-tauri-drag-region>
          <h1 data-tauri-drag-region>
            {pages.find((p) => p[0] === page)?.[1]}
          </h1>
          <div className="page-actions">
            <BackgroundTasks
              tasks={[
                ...backgroundTasks.filter(
                  (t, i, a) => a.findIndex((x) => x.id === t.id) === i,
                ),
                ...[
                  ...taskCenter.tasks,
                  ...tasks.filter(
                    (t) =>
                      !taskCenter.tasks.some((x) => x.task_id === t.task_id),
                  ),
                ]
                  .map((t) => taskCenterRecord(t, details[t.task_id]))
                  .map((t) => ({
                    id: t.task_id,
                    label:
                      sessionManager.metadata[t.session_id]?.title ||
                      t.session_meta?.title ||
                      t.title ||
                      "Claude 任务",
                    summary: queueLabel(t) || statusLabels[t.state] || t.state,
                    error: details[t.task_id]?.error || t.error,
                    running: active(t),
                    state: t.ownership_unknown ? "interrupted" : t.state,
                    queue: t.queue,
                    blocked_reason: t.blocked_reason,
                    access: t.access,
                    ownership_unknown: t.ownership_unknown,
                    cancel:
                      t.state === "queued"
                        ? () =>
                            safe(
                              request("task_cancel", {
                                task_id: t.task_id,
                              }).then(() => fetchTask(t.task_id)),
                            )
                        : undefined,
                    openBlocker: (id: string) => {
                      setPage("chat");
                      switchConversation(id);
                    },
                    kind:
                      t.access === "analysis" ? "Claude 分析" : "Claude 开发",
                    project:
                      projects.find(
                        (p) =>
                          t.workspace === p.path ||
                          t.workspace?.startsWith(p.path + "/"),
                      )?.name || t.workspace?.split("/").pop(),
                    workspace: t.workspace,
                    started: t.created_at ? t.created_at * 1000 : undefined,
                    finished: active(t)
                      ? undefined
                      : t.updated_at
                        ? t.updated_at * 1000
                        : undefined,
                    model:
                      details[t.task_id]?.model_resolved ||
                      t.model_resolved ||
                      (active(t) ? "确认中" : "未返回"),
                    mode: t.mode,
                    action: "打开会话",
                    open: () => {
                      setPage("chat");
                      switchConversation(t.task_id);
                    },
                  })),
              ]}
              warning={taskCenter.error}
              account={`${profile.user}@${profile.host}:${profile.port}`}
              connected={connected}
              onConnect={() => setPage("servers")}
            />
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
                  disabled={!locationReady || connecting}
                  onClick={() => setGitUpdateOpen(true)}
                >
                  <RefreshCw size={15} />
                  Git 更新
                </Button>
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
                          openFileOperation({
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
                  onClick={() => {
                    if (!busy.has("clone_repository")) {
                      setCloneId(undefined);
                      setCloneError("");
                      setCloneResults([]);
                      setCloneProgress({ path: listedPath });
                    }
                    setCloneOpen(true);
                  }}
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
                      <Menu.Item
                        disabled={
                          !locationReady ||
                          !!projectAtPath(listedPath, projects)
                        }
                        onSelect={() => openProject(true)}
                      >
                        {projectAtPath(listedPath, projects)
                          ? "已添加为项目"
                          : "添加为项目…"}
                      </Menu.Item>
                      <Menu.Item
                        disabled={
                          !locationReady || listedPath === currentProject
                        }
                        onSelect={() =>
                          safe(
                            request("select_workspace", {
                              path: listedPath,
                            }).then(async () => {
                              await refreshConnection();
                              toast.success("已设为 MCP 默认项目");
                            }),
                          )
                        }
                      >
                        设为 MCP 默认项目
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
                        Codeup 认证
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
                移到“应用程序”并重新打开
              </Button>
            </div>
          )}
          {!connected &&
            page !== "servers" &&
            page !== "updates" &&
            page !== "settings" && (
              <div className="notice">
                <Server size={18} />
                <span>{connecting ? "连接中…" : "未连接"}</span>
                <Button
                  variant="outline"
                  size="sm"
                  onClick={() =>
                    profile.host ? safe(connect()) : setServerEdit(true)
                  }
                >
                  连接服务器
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
              defaultModel={modelLabel(
                profile.model || "default",
                claudeInfo.catalog.models,
              )}
              usage={
                <ClaudeUsage
                  connected={connected}
                  data={claudeInfo.usage}
                  supported={!!claudeInfoSupported}
                  refresh={() => void claudeInfo.refreshUsage()}
                />
              }
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
              <Panel title="">
                <div className="server-row">
                  <div className="server-symbol">
                    <Server />
                  </div>
                  <div className="grow">
                    <h3>
                      {profile.name ||
                        (profile.host
                          ? `${profile.user}@${profile.host}`
                          : "还没有服务器")}
                    </h3>
                    <p className="muted">
                      {profile.host ? `端口 ${profile.port}` : ""}
                    </p>
                  </div>
                  <Button
                    variant="outline"
                    disabled={connecting}
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
                      disabled={connecting || !profile.host}
                      onClick={() => safe(connect())}
                    >
                      {connecting ? <Loader2 className="spin" /> : <Cable />}
                      {connecting ? "连接中…" : "连接 SSH"}
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
                  </div>
                </div>
                <div className="status-row">
                  <span>新会话默认</span>
                  <Badge variant="outline">
                    全自动 ·{" "}
                    {connection.policy?.read_only ? "分析" : "可选择分析或开发"}
                  </Badge>
                </div>
                <ProjectPolicy
                  connection={connection}
                  refresh={refreshConnection}
                  onTasks={() => {
                    const t = taskCenter.tasks.find(
                      (x) => x.access !== "analysis" && active(x),
                    );
                    setPage("chat");
                    if (t) switchConversation(t.task_id || t.id);
                  }}
                />
              </Panel>
              <Panel title="远端运行环境">
                <div className="environment-row compact-environment">
                  <Label title={connection.resolved_paths?.runner || ""}>
                    服务端程序
                  </Label>
                  <span className="grow" />
                  <span className="muted">
                    {runner.runner_version ||
                      (connected ? "未检测到" : "未连接")}
                  </span>
                  <span
                    role="status"
                    className={
                      connection.runner_maintenance?.state === "error"
                        ? "text-destructive"
                        : "muted"
                    }
                  >
                    {maintenanceLabel(connection, version)}
                  </span>
                  <Button
                    variant="ghost"
                    size="sm"
                    disabled={!connected}
                    onClick={() =>
                      safe(
                        request("runner_maintenance_retry").then(
                          refreshConnection,
                        ),
                      )
                    }
                  >
                    <RefreshCw size={14} />
                    重新检查
                  </Button>
                  {connection.runner_maintenance?.state === "action_required" &&
                    runner.runner_version !== version && (
                      <Button
                        disabled={
                          !connected ||
                          runner.runner_version === version ||
                          busy.has("install_runner")
                        }
                        onClick={() =>
                          safe(
                            request("install_runner", {
                              operation_id: (() => {
                                const id = crypto.randomUUID();
                                setInstallId(id);
                                return id;
                              })(),
                              connection_id: connection.connection_id,
                            }).then(() => {
                              toast.success("服务端程序已安装");
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
                          "安装 / 更新服务端程序"
                        )}
                      </Button>
                    )}
                </div>
                {connection.runner_maintenance?.connection_id ===
                  connection.connection_id &&
                  connection.runner_maintenance?.state === "error" && (
                    <InlineAlert>
                      {connection.runner_maintenance.message}
                    </InlineAlert>
                  )}
                {installOperation.snap?.state === "running" && (
                  <OperationBar operation={installOperation.snap} />
                )}
                <div className="environment-row compact-environment">
                  <Label title={connection.resolved_paths?.claude || ""}>
                    Claude Code
                  </Label>
                  <span className="grow" />
                  <span className="muted">
                    {runner.claude_version?.replace(" (Claude Code)", "") ||
                      runner.claude_error ||
                      "未检测到"}
                  </span>
                </div>
                <div className="environment-row compact-environment">
                  <Label>默认模型</Label>
                  <span className="grow" />
                  <span className="muted">
                    {modelLabel(
                      profile.model || "default",
                      claudeInfo.catalog.models,
                    )}
                  </span>
                  <Button
                    variant="ghost"
                    size="sm"
                    onClick={() => {
                      setModelDraft(profile.model || "");
                      setModelEdit(true);
                    }}
                  >
                    修改
                  </Button>
                </div>
              </Panel>
              <ClaudeUsage
                connected={connected}
                data={claudeInfo.usage}
                supported={!!claudeInfoSupported}
                refresh={() => void claudeInfo.refreshUsage()}
              />
              <Panel title="启动与密码">
                <div className="status-row">
                  <span>自动连接</span>
                  <Switch
                    aria-label="自动连接SSH"
                    checked={preferences.auto_connect === true}
                    disabled={connecting || busy.has("preferences_save")}
                    onCheckedChange={(v) =>
                      safe(
                        request("preferences_save", { auto_connect: v }).then(
                          () =>
                            setPreferences({ ...preferences, auto_connect: v }),
                        ),
                      )
                    }
                  />
                </div>
                {!connected && connection.reconnect?.state && (
                  <div className="status-row" role="status">
                    <span>
                      {connection.reconnect.state === "connecting"
                        ? "正在重新连接"
                        : connection.reconnect.state === "waiting"
                          ? "等待网络恢复 · 将自动重连"
                          : "自动连接已暂停"}
                    </span>
                    {connection.reconnect.error && (
                      <span className="muted">
                        {connection.reconnect.error}
                      </span>
                    )}
                  </div>
                )}
                <div className="status-row">
                  <span>SSH 密码</span>
                  <span className="status-value">
                    <span className="muted">
                      {profile.remember_password
                        ? profile.credential_store === "file"
                          ? "本机文件"
                          : "钥匙串"
                        : "未保存"}
                    </span>
                    <Button
                      variant="ghost"
                      size="sm"
                      onClick={() => setServerEdit(true)}
                    >
                      管理
                    </Button>
                  </span>
                </div>
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
                    onClick={() => openProject()}
                  >
                    <Plus size={14} />
                  </Button>
                </div>
                {projects.map((p) => (
                  <div
                    className={`project-entry-row ${p.path === browsedProject ? "active" : ""}`}
                    key={p.path}
                  >
                    <button
                      className="project-item"
                      onClick={() => safe(browse(p.path))}
                    >
                      <FolderOpen size={17} />
                      <div>
                        <strong>{p.name}</strong>
                        {p.path === currentProject && (
                          <small>MCP默认项目</small>
                        )}
                      </div>
                    </button>
                    <ProjectMenu
                      project={p}
                      disabled={!connected || removingProject}
                      onRemove={removeProject}
                    />
                  </div>
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
                  <Menu.Root>
                    <Menu.Trigger asChild>
                      <Button
                        variant="outline"
                        aria-label="当前目录操作"
                        disabled={!locationReady}
                      >
                        <MoreHorizontal size={16} />
                      </Button>
                    </Menu.Trigger>
                    <Menu.Portal>
                      <Menu.Content className="context-menu" align="end">
                        <Menu.Item
                          disabled={!!projectAtPath(listedPath, projects)}
                          onSelect={() => openProject(true)}
                        >
                          <FolderOpen size={15} />
                          {projectAtPath(listedPath, projects)
                            ? "已添加为项目"
                            : "添加为项目"}
                        </Menu.Item>
                      </Menu.Content>
                    </Menu.Portal>
                  </Menu.Root>
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
                    <span>分支</span>
                    <span>最近修改</span>
                  </div>
                  {visibleEntries.map((e: Data) => (
                    <FileContextMenu
                      key={e.path}
                      entry={e}
                      disabled={!locationReady}
                      onAction={(action, entry) =>
                        openFileOperation({ action, entry })
                      }
                    >
                      <div
                        className="file-row"
                        onKeyDown={(event) => {
                          if (!locationReady) return;
                          if (event.key === "F2") {
                            event.preventDefault();
                            openFileOperation({ action: "rename", entry: e });
                          } else if (
                            event.metaKey &&
                            event.key === "Backspace"
                          ) {
                            event.preventDefault();
                            openFileOperation({ action: "trash", entry: e });
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
                        <span
                          className="file-branch"
                          title={e.branch || undefined}
                        >
                          {e.branch || "—"}
                        </span>
                        <span className="file-modified">
                          {e.modified_at ? time(e.modified_at) : "—"}
                        </span>
                        <FileActions
                          entry={e}
                          disabled={!locationReady}
                          onAction={(action, entry) =>
                            openFileOperation({ action, entry })
                          }
                        />
                      </div>
                    </FileContextMenu>
                  ))}
                  {fileFilter &&
                    listing.entries?.length > 0 &&
                    visibleEntries.length === 0 && (
                      <p className="empty">没有匹配文件</p>
                    )}
                  {!listing.entries?.length && (
                    <p className="empty">
                      {browsing
                        ? "加载中…"
                        : listedPath
                          ? "空文件夹"
                          : "选择项目或输入目录"}
                    </p>
                  )}
                </div>
                <div className="file-status">
                  {fileFilter ? `${visibleEntries.length} / ` : ""}
                  {listing.entries?.length || 0}项
                  {listing.truncated ? " · 仅显示前1000项" : ""}
                </div>
              </section>
            </div>
          )}
          {page === "chat" && (
            <div className="chat-layout">
              <aside className="sessions">
                <SessionList
                  key={connection.connection_id || "offline"}
                  sessions={sessionManager}
                  projects={projects}
                  selected={selected}
                  selectedProject={
                    !newConversation && task
                      ? task.workspace
                      : draftProject || currentProject
                  }
                  onSelect={(id) => switchConversation(id)}
                  onUpdateRunner={() => setPage("servers")}
                  onAddProject={() => openProject()}
                  onRemoveProject={removeProject}
                  removingProject={removingProject}
                  onNewSession={(p) =>
                    startDraft(p || connection.environment?.home || "")
                  }
                />
                {!sessionManager.supported && groupSessionsFallback}
              </aside>
              <section className="conversation">
                <div className="conversation-heading">
                  <div>
                    <strong>
                      {task && !newConversation
                        ? String(
                            selectedSession?.title ||
                              task.title ||
                              task.prompt ||
                              "会话",
                          )
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
                      {task.access === "analysis" ? "分析" : "开发"}
                    </Badge>
                  )}
                  {task && !newConversation && (
                    <Badge variant="outline">
                      {task.mode === "automatic" ? "全自动" : "审批"}
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
                    项目数据库配置已更改，本会话仍使用旧配置。
                  </div>
                )}
                {task && !newConversation && !task.session_id && (
                  <div className="archive-banner">
                    旧版记录，无法续聊
                    <Button
                      variant="outline"
                      onClick={() => startDraft(task.workspace)}
                    >
                      新建任务
                    </Button>
                  </div>
                )}
                {selectedSession?.archived && !newConversation && (
                  <div className="archive-banner">
                    已归档
                    <Button
                      variant="outline"
                      onClick={() =>
                        safe(
                          sessionManager.edit(selectedSession, {
                            archived: false,
                          }),
                        )
                      }
                    >
                      恢复会话
                    </Button>
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
                      {timeline[0]?.reply_to &&
                        !details[timeline[0].reply_to] && (
                          <Button
                            variant="ghost"
                            onClick={() =>
                              safe(fetchTask(timeline[0].reply_to))
                            }
                          >
                            加载更早轮次
                          </Button>
                        )}
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
                                {Array.isArray(turn.events)
                                  ? displayModel(turn).actual
                                  : "读取中…"}
                              </Badge>
                              <span>
                                {statusLabels[turn.state] || turn.state}
                              </span>
                            </div>
                            {turn.state === "queued" ||
                            turn.ownership_unknown ? (
                              <TaskQueue
                                task={turn}
                                onOpen={(id) => switchConversation(id)}
                                onCancel={
                                  turn.ownership_unknown
                                    ? undefined
                                    : () =>
                                        safe(
                                          request("task_cancel", {
                                            task_id: turn.task_id,
                                          }).then(() =>
                                            fetchTask(turn.task_id),
                                          ),
                                        )
                                }
                                onCheck={() => safe(fetchTask(turn.task_id))}
                                onRelease={async () => {
                                  await invoke("task_release", {
                                    request: {
                                      task_id: turn.task_id,
                                      confirmed: true,
                                    },
                                  });
                                  await fetchTask(turn.task_id);
                                }}
                              />
                            ) : !Array.isArray(turn.events) ? (
                              <p role="status" className="muted">
                                读取会话…
                              </p>
                            ) : taskFailure(turn) ? (
                              <InlineAlert>{taskFailure(turn)}</InlineAlert>
                            ) : turn.result ? (
                              <Markdown
                                text={
                                  typeof turn.result === "string"
                                    ? turn.result
                                    : turn.result?.result || "无文本输出"
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
                                {active(turn)
                                  ? `执行中${turn.started_at || turn.created_at ? " · " + elapsed(tick - (turn.started_at || turn.created_at) * 1000) : ""}`
                                  : "无文本输出"}
                              </p>
                            )}
                            {turn.error &&
                              !turn.ownership_unknown &&
                              !taskFailure(turn) && (
                                <div className="inline-error">
                                  <p>
                                    {/^[\u4e00-\u9fff]/.test(turn.error)
                                      ? turn.error.split("\n")[0]
                                      : "执行失败"}
                                  </p>
                                  <details>
                                    <summary>原始错误</summary>
                                    <pre>{turn.error}</pre>
                                  </details>
                                </div>
                              )}
                            {(turn.events || []).length > 0 && (
                              <details>
                                <summary>{turn.events.length} 个步骤</summary>
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
                    <div className="chat-empty project-chat-empty">
                      <FolderOpen size={28} />
                      <h2>
                        {projects.find(
                          (p) => p.path === (draftProject || currentProject),
                        )?.name
                          ? `从${projects.find((p) => p.path === (draftProject || currentProject))?.name}开始`
                          : "开始新会话"}
                      </h2>
                      <p>
                        {draftProject || currentProject
                          ? "新会话将在此目录运行"
                          : "无需项目也可以开始分析"}
                      </p>
                      <Button
                        disabled={!connected}
                        onClick={() =>
                          projects.some(
                            (p) => p.path === (draftProject || currentProject),
                          )
                            ? document
                                .querySelector<HTMLTextAreaElement>(
                                  '[aria-label="任务指令"]',
                                )
                                ?.focus()
                            : openProject()
                        }
                      >
                        <Plus size={15} />
                        {projects.some(
                          (p) => p.path === (draftProject || currentProject),
                        )
                          ? "新建会话"
                          : "添加项目"}
                      </Button>
                    </div>
                  )}
                  {task?.state === "waiting_approval" &&
                    (task.approvals || []).map((a: Data) => (
                      <div className="approval" key={a.request_id}>
                        <ShieldCheck />
                        <div>
                          <strong>需要确认 · {a.tool_name}</strong>
                          <pre>
                            {a.input?.command ||
                              a.input?.file_path ||
                              a.input?.path ||
                              JSON.stringify(a.input, null, 2)}
                          </pre>
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
                              允许
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
                    <Select
                      value={
                        !runner.capabilities?.includes("analysis_v1")
                          ? "write"
                          : connection.policy?.read_only
                            ? "analysis"
                            : accessChoice === "inherit"
                              ? !newConversation && task
                                ? task.access || "write"
                                : "analysis"
                              : accessChoice
                      }
                      onValueChange={setAccessChoice}
                    >
                      <SelectTrigger
                        className="scope-select"
                        aria-label="任务类型"
                      >
                        <SelectValue />
                      </SelectTrigger>
                      <SelectContent>
                        <SelectItem
                          value="analysis"
                          disabled={
                            !runner.capabilities?.includes("analysis_v1")
                          }
                        >
                          分析
                        </SelectItem>
                        <SelectItem
                          value="write"
                          disabled={connection.policy?.read_only === true}
                        >
                          开发
                        </SelectItem>
                      </SelectContent>
                    </Select>
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
                        <SelectItem value="approval">审批</SelectItem>
                      </SelectContent>
                    </Select>
                    <RemoteModelSelect
                      label="下一轮模型"
                      value={
                        modelPolicy === "inherit"
                          ? !newConversation && task
                            ? "__inherit"
                            : profile.model || "default"
                          : modelPolicy === "remote_default"
                            ? "default"
                            : model
                      }
                      inheritLabel={
                        !newConversation && task
                          ? nextModelLabel(task, profile.model)
                          : undefined
                      }
                      models={claudeInfo.catalog.models}
                      loading={claudeInfo.catalog.loading}
                      disabled={!connected}
                      onChange={(v) => {
                        if (v === "__inherit") {
                          setModelPolicy("inherit");
                          setModel("");
                        } else if (v === "default") {
                          setModelPolicy("remote_default");
                          setModel("");
                        } else {
                          setModelPolicy("explicit");
                          setModel(v);
                        }
                      }}
                    />
                    <Button
                      variant="ghost"
                      size="icon"
                      aria-label="刷新模型列表"
                      disabled={
                        !claudeInfoSupported || claudeInfo.catalog.loading
                      }
                      onClick={() => void claudeInfo.refreshModels()}
                    >
                      <RefreshCw
                        size={14}
                        className={claudeInfo.catalog.loading ? "spin" : ""}
                      />
                    </Button>
                  </div>
                  {invalidModel && (
                    <InlineAlert>模型不在远端列表，请重新选择</InlineAlert>
                  )}
                  {claudeInfo.catalog.error && (
                    <InlineAlert>{claudeInfo.catalog.error}</InlineAlert>
                  )}
                  <Textarea
                    disabled={
                      !!task &&
                      !newConversation &&
                      (!task.session_id || !!selectedSession?.archived)
                    }
                    aria-label="任务指令"
                    value={draft}
                    onChange={(e) => setDraft(e.target.value)}
                    placeholder={
                      selectedSession?.archived && !newConversation
                        ? "恢复会话后继续追问"
                        : task?.state === "queued" && !newConversation
                          ? "本轮排队中，结束后可续聊…"
                          : task && !newConversation
                            ? "继续追问…"
                            : "描述任务…"
                    }
                    onKeyDown={(e) => {
                      if ((e.metaKey || e.ctrlKey) && e.key === "Enter") {
                        e.preventDefault();
                        safe(send());
                      }
                    }}
                  />
                  <div className="composer-footer">
                    <span></span>
                    <div>
                      {task && active(task) && task.state !== "queued" && (
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
                          {task.state === "queued" ? "取消排队" : "停止"}
                        </Button>
                      )}
                      <Button
                        disabled={
                          (!!task && !newConversation && !task.session_id) ||
                          (!!selectedSession?.archived && !newConversation) ||
                          !connected ||
                          !draft.trim() ||
                          busy.has("task_start") ||
                          busy.has("task_reply") ||
                          (!!task && active(task)) ||
                          (modelPolicy === "explicit" && !model.trim()) ||
                          invalidModel ||
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
              key={connection.connection_id || "offline"}
              connected={connected}
              projects={projects}
              currentProject={currentProject}
              request={request}
              refreshProjects={refreshProjects}
            />
          )}
          {page === "codex" && (
            <div className="integration-panel">
              <Panel title="Codex 接入">
                <StatusRow label="Codex 配置">
                  <span
                    role={integrationState === "error" ? "alert" : "status"}
                    className={
                      integrationState === "error"
                        ? "text-destructive"
                        : "muted"
                    }
                  >
                    {integration || mcpStatus.message || "正在检查配置…"}
                  </span>
                </StatusRow>
                <div className="toolbar integration-actions">
                  <Button
                    disabled={
                      !installLocation.installed ||
                      integrationState === "saving"
                    }
                    onClick={() => void configureCodex()}
                  >
                    <Cable size={16} />
                    {integrationState === "saving" ? "正在配置…" : "写入配置"}
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
              </Panel>
              <Panel title="提供的工具">
                <div className="tool-tags">
                  {[
                    "connection_status",
                    "project_list",
                    "cc_start",
                    "cc_reply",
                    "cc_result",
                    "git_update",
                  ].map((name) => (
                    <code key={name}>{name}</code>
                  ))}
                </div>
              </Panel>
            </div>
          )}
          {page === "updates" && (
            <div className="update-panel">
              <Panel title="">
                <div className="update-hero">
                  <AppIcon size={56} />
                  <div>
                    <h2>CCBridge {version}</h2>
                    <p className="muted">{architecture}</p>
                  </div>
                </div>
                <div className="update-status">
                  {
                    {
                      idle: "未检查",
                      checking: "正在检查 GitHub…",
                      current: "未发现新版本",
                      available: `发现新版本 ${update.version || ""}`,
                      downloading: "下载中…",
                      verified: "签名验证通过",
                      installing: "正在准备安装…",
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
                {updatePhase === "downloading" && (
                  <div role="status">
                    {updateTransfer.total > 0 && <Progress value={progress} />}
                    <p className="hint">
                      已下载 {bytes(updateTransfer.downloaded || 0)}
                      {updateTransfer.total
                        ? ` / ${bytes(updateTransfer.total)} · ${progress}%`
                        : ""}
                      <span className="transfer-time">
                        {elapsed(tick - updateStarted)}
                      </span>
                    </p>
                  </div>
                )}
                <div className="toolbar integration-actions">
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
                        setErrors((old) => {
                          const next = { ...old };
                          delete next.update;
                          return next;
                        });
                        setUpdatePhase("installing");
                        safe(
                          invoke("apply_update").catch((e) => {
                            setUpdatePhase("verified");
                            setErrors((x) => ({
                              ...x,
                              update: { page: "updates", message: String(e) },
                            }));
                          }),
                        );
                      }}
                    >
                      安装并重启
                    </Button>
                  )}
                </div>
                {errors.update && (
                  <InlineAlert>{errors.update.message}</InlineAlert>
                )}
              </Panel>
            </div>
          )}
          {page === "settings" && (
            <>
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
                    管理 SSH 密码
                  </Button>
                  <Button
                    variant="outline"
                    onClick={() => {
                      setAuthScope("local");
                      setAuthOpen(true);
                    }}
                  >
                    本机 Codeup
                  </Button>
                  <Button
                    variant="outline"
                    disabled={!connected}
                    onClick={() => {
                      setAuthScope("remote");
                      setAuthOpen(true);
                    }}
                  >
                    服务器 Codeup
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
              </Panel>
            </>
          )}
        </div>
      </main>
      {projectEntry &&
        projectEntry.connectionId === connection.connection_id && (
          <ProjectDialog
            key={`${projectEntry.connectionId}:${projectEntry.path}:${projectEntry.currentOnly}`}
            open
            onClose={() => setProjectEntry(null)}
            connected={connected}
            connectionId={connection.connection_id || ""}
            home={connection.environment?.home || ""}
            account={`${connection.profile?.user || ""}@${connection.profile?.host || ""}`}
            projects={projects}
            initialPath={projectEntry.path}
            currentOnly={projectEntry.currentOnly}
            onAdded={async (p) => {
              await refreshProjects();
              toast.success("已添加项目");
              if (page === "chat") startDraft(p.path);
            }}
          />
        )}
      <Dialog
        open={!!utilityDialog}
        onOpenChange={(v) => !v && setUtilityDialog("")}
      >
        <DialogContent>
          <DialogHeader>
            <DialogTitle>
              {utilityDialog === "draft-project" ? "新任务项目" : "新建文件夹"}
            </DialogTitle>
            <DialogDescription>
              {utilityDialog === "draft-project"
                ? "仅本次新任务使用"
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
                value={folderName}
                onChange={(e) => setFolderName(e.target.value)}
              />
              <DialogFooter>
                <Button
                  disabled={!locationReady || !folderName.trim()}
                  onClick={() =>
                    safe(
                      request("create_directory", {
                        path: listedPath,
                        name: folderName,
                      }).then(async () => {
                        await browse(listedPath);
                        setUtilityDialog("");
                        setFolderName("");
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
      <GitUpdate
        onOpenTask={(id) => {
          setGitUpdateOpen(false);
          setPage("chat");
          switchConversation(id);
        }}
        open={gitUpdateOpen}
        path={gitDialogPath || listedPath}
        connectionId={connection.connection_id || ""}
        operationId={gitCenterId}
        onOperation={setGitOperation}
        onClose={() => setGitUpdateOpen(false)}
        onUpdated={() => {
          if (
            locationRef.current.path === gitDialogPath &&
            locationRef.current.connection === connection.connection_id
          )
            void browse(gitDialogPath);
        }}
        onAuth={(url) => {
          setGitUpdateOpen(false);
          setAuthUrl(url);
          setAuthScope("remote");
          setAuthOpen(true);
        }}
      />
      <CodeupAuth
        key={`codeup:${connection.connection_id || "offline"}`}
        open={authOpen}
        onOpenChange={(v) => {
          setAuthOpen(v);
        }}
        requestVersion={authRequestVersion}
        onActivity={(activity, id) => {
          setAuthActivity(activity);
          if (id) setAuthOperationId(id);
        }}
        connected={connected}
        account={
          connected
            ? `${connection.profile?.user}@${connection.profile?.host}:${connection.profile?.port}`
            : ""
        }
        url={authUrl || cloneUrls.trim().split(/\s+/)[0] || ""}
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
      {fileOperation &&
        connected &&
        fileOperation.connectionId === connection.connection_id && (
          <FileOperation
            key={`${fileOperation.connectionId}:${fileOperation.entry.path}:${fileOperation.action}`}
            connectionId={fileOperation.connectionId}
            operation={fileOperation}
            onClose={() => setFileOperation(null)}
            onDone={async () => {
              await browse(listedPath);
            }}
            home={connection.environment?.home || ""}
          />
        )}
      <TrashDialog
        key={`trash:${connected ? connection.connection_id : "offline"}`}
        connectionId={connection.connection_id || ""}
        open={connected && trashOpen}
        onClose={() => setTrashOpen(false)}
        onDone={async () => {
          await browse(listedPath);
        }}
      />
      <BranchDialog
        key={`${connection.connection_id || "offline"}:${branchDialogPath || listedPath}`}
        open={branchesOpen}
        onClose={() => setBranchesOpen(false)}
        path={branchDialogPath || listedPath}
        connectionId={connection.connection_id || ""}
        operationId={branchCenterId}
        onOperation={setBranchOperation}
        onChanged={async () => {
          await browse(listedPath);
        }}
      />
      <Confirm
        open={!!localConfirm}
        title="本机保存密码"
        action={localConfirm?.action === "connect" ? "保存并连接" : "保存"}
        onCancel={() => setLocalConfirm(null)}
        onConfirm={() => {
          const a = localConfirm?.action;
          setLocalConfirm(null);
          setStorageAck(true);
          safe(
            a === "connect"
              ? connect(profile, true)
              : a === "migrate"
                ? migrateServer(true)
                : saveServer(true),
          );
        }}
      >
        <p>
          密码将明文保存在本机当前用户目录，同一用户运行的程序可读取。此提示只确认一次。
        </p>
      </Confirm>
      <Toaster
        theme={theme === "system" ? "system" : (theme as "light" | "dark")}
        richColors
        position="bottom-right"
      />
      <Dialog open={modelEdit} onOpenChange={setModelEdit}>
        <DialogContent>
          <DialogHeader>
            <DialogTitle>新会话默认模型</DialogTitle>
            <DialogDescription className="sr-only">
              选择远端 Claude 提供的模型
            </DialogDescription>
          </DialogHeader>
          <Field label="模型">
            <RemoteModelSelect
              value={modelDraft || "default"}
              onChange={(v) => setModelDraft(v === "default" ? "" : v)}
              models={claudeInfo.catalog.models}
              loading={claudeInfo.catalog.loading}
            />
          </Field>
          <Button
            variant="ghost"
            disabled={!claudeInfoSupported || claudeInfo.catalog.loading}
            onClick={() => void claudeInfo.refreshModels()}
          >
            {claudeInfo.catalog.loading ? "获取模型…" : "刷新列表"}
          </Button>
          {!claudeInfoSupported && (
            <InlineAlert kind="warning">
              更新服务端程序后获取模型列表
            </InlineAlert>
          )}
          {claudeInfo.catalog.error && (
            <InlineAlert>{claudeInfo.catalog.error}</InlineAlert>
          )}
          {errors.save_profile && (
            <InlineAlert>
              {String(errors.save_profile.message || errors.save_profile)}
            </InlineAlert>
          )}
          <DialogFooter>
            <Button variant="outline" onClick={() => setModelEdit(false)}>
              关闭
            </Button>
            <Button
              disabled={
                connecting ||
                busy.has("save_profile") ||
                claudeInfo.catalog.loading ||
                !claudeInfo.catalog.models.some(
                  (m) => m.value === (modelDraft || "default"),
                )
              }
              onClick={() =>
                safe(
                  request("save_profile", {
                    profile: { ...profile, model: modelDraft.trim() },
                    password,
                    storage_ack: storageAck,
                  }).then(() => {
                    setProfile({ ...profile, model: modelDraft.trim() });
                    setModelEdit(false);
                    toast.success("已保存");
                  }),
                )
              }
            >
              {busy.has("save_profile") ? "保存中…" : "保存"}
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>
      <Dialog open={serverEdit} onOpenChange={setServerEdit}>
        <DialogContent className="server-dialog">
          <DialogHeader>
            <DialogTitle>SSH 服务器</DialogTitle>
            <DialogDescription></DialogDescription>
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
          <Field label="密码">
            <Input
              type="password"
              value={password}
              autoComplete="off"
              placeholder={profile.remember_password ? "已保存，留空不变" : ""}
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
          <details>
            <summary>高级</summary>
            <Field label="服务端程序路径">{input("runner")}</Field>
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
                  disabled={busy.has("migrate_password")}
                  variant="outline"
                  onClick={() => safe(migrateServer())}
                >
                  {busy.has("migrate_password")
                    ? "正在迁移…"
                    : "迁移此账号已保存的密码"}
                </Button>
                <p className="hint">可能弹出钥匙串授权</p>
              </div>
            )}
          <DialogFooter>
            <Button
              variant="outline"
              disabled={connecting || busy.has("save_profile")}
              onClick={() => safe(saveServer())}
            >
              保存配置
            </Button>
            <Button
              disabled={
                connecting || !profile.host || !profile.user || !profile.port
              }
              onClick={() => safe(connect())}
            >
              {connecting ? "连接中…" : "连接 SSH"}
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
              允许全自动
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>
      <Dialog open={!!hostKey} onOpenChange={(v) => !v && setHostKey(null)}>
        <DialogContent>
          <DialogHeader>
            <DialogTitle>
              {hostKey?.profile.fingerprint
                ? "服务器身份已变化"
                : "首次连接此服务器"}
            </DialogTitle>
            <DialogDescription>
              {hostKey?.profile.fingerprint
                ? "与已保存指纹不一致。请先向管理员核实，更换主机或密钥也可能导致变化。"
                : "确认服务器指纹后，保存并继续连接。"}
            </DialogDescription>
          </DialogHeader>
          <StatusRow label="服务器">
            {hostKey?.profile.user}@{hostKey?.profile.host}:
            {hostKey?.profile.port}
          </StatusRow>
          {hostKey?.profile.fingerprint && (
            <StatusRow label="原指纹">
              <code className="break-all">{hostKey.profile.fingerprint}</code>
            </StatusRow>
          )}
          <StatusRow label="SHA256 指纹">
            <code className="break-all">{hostKey?.fingerprint}</code>
          </StatusRow>
          <DialogFooter>
            <Button variant="outline" onClick={() => setHostKey(null)}>
              取消
            </Button>
            <Button
              onClick={() => {
                if (!hostKey) return;
                const candidate = hostKey;
                setHostKey(null);
                safe(
                  connect(
                    {
                      ...candidate.profile,
                      fingerprint: candidate.fingerprint,
                    },
                    candidate.ack,
                    candidate.secret,
                  ),
                );
              }}
            >
              {hostKey?.profile.fingerprint
                ? "已核实，更新并连接"
                : "信任并连接"}
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>
      <Dialog
        open={cloneOpen}
        onOpenChange={(open) => {
          setCloneOpen(open);
        }}
      >
        <DialogContent>
          <DialogHeader>
            <DialogTitle>克隆仓库</DialogTitle>
            <DialogDescription>
              克隆到 {(cloneProgress.path || listedPath).split("/").pop()}
            </DialogDescription>
          </DialogHeader>
          <Textarea
            value={cloneUrls}
            disabled={busy.has("clone_repository")}
            onChange={(e) => setCloneUrls(e.target.value)}
            placeholder="每行一个 HTTPS 地址"
          />
          <div className="clone-source">
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
                <SelectItem value="local">本机获取</SelectItem>
                <SelectItem value="remote">服务器获取</SelectItem>
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
              认证
            </Button>
          </div>
          <OperationBar operation={cloneOperation.snap} />
          {cloneError && <InlineAlert>{cloneError}</InlineAlert>}
          {cloneResults.map((r) => (
            <p key={r.path} className="hint">
              {r.name} ·{" "}
              {r.state === "already_exists" ? "已存在，未改动" : "已克隆"}
            </p>
          ))}
          <DialogFooter>
            <Button variant="outline" onClick={() => setCloneOpen(false)}>
              {busy.has("clone_repository") ? "在后台继续" : "关闭"}
            </Button>
            <Button
              disabled={!cloneUrls.trim() || busy.has("clone_repository")}
              onClick={() => void cloneRepositories()}
            >
              {busy.has("clone_repository") ? "进行中…" : "克隆"}
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
