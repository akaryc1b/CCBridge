import { DropdownMenu as Menu } from "radix-ui";
import { StatusRow } from "./common/Feedback";
import { MoreHorizontal, RefreshCw } from "lucide-react";
import { useEffect, useState } from "react";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import {
  Card,
  CardHeader,
  CardTitle,
  CardDescription,
  CardContent,
} from "@/components/ui/card";
import { Badge } from "@/components/ui/badge";
import { Switch } from "@/components/ui/switch";
import {
  Select,
  SelectTrigger,
  SelectValue,
  SelectContent,
  SelectItem,
} from "@/components/ui/select";
import {
  Dialog,
  DialogContent,
  DialogHeader,
  DialogTitle,
  DialogDescription,
  DialogFooter,
} from "@/components/ui/dialog";
type Data = Record<string, any>;
const empty = {
  id: "",
  name: "",
  kind: "mysql",
  host: "",
  port: 3306,
  user: "",
  database: "",
  password: "",
  enabled: false,
  node: "",
  package_root: "",
};
export function Databases({
  connected,
  projects,
  currentProject,
  request,
  refreshProjects,
}: {
  connected: boolean;
  projects: Data[];
  currentProject: string;
  request: (command: string, args?: Data) => Promise<any>;
  refreshProjects: () => Promise<void>;
}) {
  const [tab, setTab] = useState("connections");
  const [workspace, setWorkspace] = useState(currentProject),
    [data, setData] = useState<Data>({ connections: [], bindings: [] }),
    [form, setForm] = useState<Data>(empty),
    [editing, setEditing] = useState(false),
    [removing, setRemoving] = useState<Data | null>(null),
    [bindings, setBindings] = useState<Data[]>([]),
    [runtime, setRuntime] = useState<Data>({}),
    [busy, setBusy] = useState(false),
    [error, setError] = useState(""),
    [notice, setNotice] = useState(""),
    [checks, setChecks] = useState<Data>({}),
    [loaded, setLoaded] = useState(false);
  const run = async (fn: () => Promise<void>) => {
    if (busy) return;
    setBusy(true);
    setError("");
    try {
      await fn();
    } catch (e: any) {
      setError(e?.message || String(e));
    } finally {
      setBusy(false);
    }
  };
  async function load(w = workspace) {
    const r = await request("database_list", { workspace: w });
    if (r) {
      setData(r);
      setLoaded(true);
      setBindings(r.bindings || []);
    }
  }
  useEffect(() => {
    if (connected)
      void run(async () => {
        await load();
        if (!runtime.node) {
          const r = await request("database_detect", {});
          setRuntime(r);
        }
      });
  }, [connected, workspace]);
  const field = (key: string, label: string, type = "text") => (
    <div className="field">
      <Label htmlFor={`db-${key}`}>{label}</Label>
      <Input
        id={`db-${key}`}
        type={type}
        value={form[key] ?? ""}
        autoComplete="off"
        onChange={(e) =>
          setForm({
            ...form,
            [key]: type === "number" ? Number(e.target.value) : e.target.value,
          })
        }
      />
    </div>
  );
  return (
    <div className="space-y-5">
      <div className="segmented">
        <button
          className={tab === "connections" ? "chosen" : ""}
          aria-pressed={tab === "connections"}
          onClick={() => setTab("connections")}
        >
          连接
        </button>
        <button
          className={tab === "mapping" ? "chosen" : ""}
          aria-pressed={tab === "mapping"}
          onClick={() => setTab("mapping")}
        >
          项目映射
        </button>
      </div>
      {tab === "connections" && (
        <Card>
          <CardHeader>
            <CardTitle>数据库连接</CardTitle>
          </CardHeader>
          <CardContent className="space-y-4">
            <div className="toolbar db-toolbar">
              <Button
                disabled={!connected || busy}
                onClick={() => {
                  setForm({
                    ...empty,
                    node: runtime.node || "",
                    package_root: runtime.package_root || "",
                  });
                  setEditing(true);
                }}
              >
                新增连接
              </Button>
              <Button
                variant="outline"
                disabled={!connected || busy}
                onClick={() => void run(() => load())}
              >
                <RefreshCw size={14} />
                刷新
              </Button>
            </div>
            {notice && (
              <p role="status" className="hint">
                {notice}
              </p>
            )}
            {!loaded && busy && (
              <p role="status" className="hint">
                读取连接…
              </p>
            )}
            {loaded && data.connections.length === 0 && (
              <p className="hint">还没有数据库连接</p>
            )}
            {data.connections.map((d: Data) => (
              <div
                key={d.id}
                className="flex flex-wrap items-center justify-between gap-3 db-connection-row"
              >
                <div>
                  <strong>{d.name}</strong>{" "}
                  <Badge variant={d.enabled ? "secondary" : "outline"}>
                    {d.enabled ? "已启用" : "未启用"}
                  </Badge>
                  <p className="hint">
                    {d.kind} · {d.host}:{d.port} / {d.database}
                  </p>
                  <p className="hint">
                    只读 · 被 {(d.used_by || []).length} 个项目使用
                    {!d.password_saved ? " · 未保存密码" : ""}
                  </p>
                  {checks[d.id] && (
                    <p role="status" className="hint">
                      <span className="db-checks">
                        {Object.entries(checks[d.id]).map(([key, value]) => (
                          <span key={key}>
                            {key === "mcp" ? "工具连接" : "数据库连接"}：
                            {String(value)}
                          </span>
                        ))}
                      </span>
                    </p>
                  )}
                </div>
                <div className="flex flex-wrap gap-2">
                  <Button
                    variant="outline"
                    disabled={busy}
                    onClick={() =>
                      void run(async () => {
                        for (const [command, key] of [
                          ["database_protocol_test", "mcp"],
                          ["database_test", "database"],
                        ]) {
                          setChecks((p) => ({
                            ...p,
                            [d.id]: { ...p[d.id], [key]: "检测中…" },
                          }));
                          try {
                            const r = await request(command, { id: d.id });
                            setChecks((p) => ({
                              ...p,
                              [d.id]: {
                                ...p[d.id],
                                [key]: `${r.notice || "已完成"} · ${new Date().toLocaleTimeString("zh-CN", { hour: "2-digit", minute: "2-digit" })}`,
                              },
                            }));
                          } catch (e: any) {
                            setChecks((p) => ({
                              ...p,
                              [d.id]: {
                                ...p[d.id],
                                [key]: e?.message || String(e),
                              },
                            }));
                          }
                        }
                      })
                    }
                  >
                    测试
                  </Button>
                  <Menu.Root>
                    <Menu.Trigger asChild>
                      <Button
                        variant="ghost"
                        size="icon"
                        aria-label={`${d.name}的操作`}
                        disabled={busy}
                      >
                        <MoreHorizontal size={16} />
                      </Button>
                    </Menu.Trigger>
                    <Menu.Portal>
                      <Menu.Content className="context-menu">
                        <Menu.Item
                          onSelect={() => {
                            setForm({ ...d, password: "" });
                            setEditing(true);
                          }}
                        >
                          编辑
                        </Menu.Item>
                        <Menu.Item
                          disabled={(d.used_by || []).length > 0}
                          title={
                            (d.used_by || []).length ? "此连接仍被项目引用" : ""
                          }
                          onSelect={() => setRemoving(d)}
                        >
                          删除
                        </Menu.Item>
                      </Menu.Content>
                    </Menu.Portal>
                  </Menu.Root>
                </div>
              </div>
            ))}
            <StatusRow
              label="数据库 MCP"
              action={
                <Button
                  variant="ghost"
                  size="sm"
                  disabled={!connected || busy}
                  onClick={() =>
                    void run(async () => {
                      const r = await request("database_detect", {});
                      setRuntime(r);
                      setNotice(r.notice);
                    })
                  }
                >
                  {busy ? "检测中…" : "检测"}
                </Button>
              }
            >
              <span className="muted" title={runtime.package_root || ""}>
                {runtime.candidates?.some((c: Data) => c.compatible)
                  ? `已找到数据库工具 · ${runtime.candidates.find((c: Data) => c.compatible)?.version}`
                  : connected
                    ? !runtime.candidates
                      ? busy
                        ? "检测中…"
                        : "未检测"
                      : "未找到数据库工具"
                    : "未连接"}
              </span>
            </StatusRow>
            {(runtime.candidates?.length > 0 ||
              runtime.registered?.length > 0) && (
              <details className="runtime-diagnostics">
                <summary>检测记录</summary>
                {runtime.candidates?.map((c: Data) => (
                  <p key={c.package_root}>
                    <code>{c.package_root}</code> · {c.version} · {c.source}
                  </p>
                ))}
                {runtime.registered?.map((r: Data) => (
                  <p key={r.name}>
                    <code>{r.name}</code> · {r.command}
                  </p>
                ))}
              </details>
            )}
          </CardContent>
        </Card>
      )}
      {tab === "mapping" && (
        <Card>
          <CardHeader>
            <CardTitle>项目数据库映射</CardTitle>
            <CardDescription>仅对新会话生效</CardDescription>
          </CardHeader>
          <CardContent className="space-y-4">
            <Label>配置项目</Label>
            <Select
              value={workspace || undefined}
              onValueChange={setWorkspace}
              disabled={busy || !connected}
            >
              <SelectTrigger aria-label="数据库映射项目">
                <SelectValue placeholder="选择项目" />
              </SelectTrigger>
              <SelectContent>
                {projects.map((p) => (
                  <SelectItem key={p.path} value={p.path}>
                    {p.name} · {p.path}
                  </SelectItem>
                ))}
              </SelectContent>
            </Select>
            {bindings.map((b, i) => (
              <div key={i} className="flex flex-wrap gap-3 items-center">
                <Input
                  className="max-w-52"
                  aria-label={`第${i + 1}个数据库用途别名`}
                  placeholder="用途：business_dev"
                  value={b.alias}
                  disabled={busy}
                  onChange={(e) =>
                    setBindings(
                      bindings.map((x, j) =>
                        j === i ? { ...x, alias: e.target.value } : x,
                      ),
                    )
                  }
                />
                <code className="muted">db_{b.alias || "用途"}</code>
                <Select
                  value={b.connection_id || undefined}
                  disabled={busy}
                  onValueChange={(v) =>
                    setBindings(
                      bindings.map((x, j) =>
                        j === i ? { ...x, connection_id: v } : x,
                      ),
                    )
                  }
                >
                  <SelectTrigger
                    className="max-w-80"
                    aria-label={`第${i + 1}个映射连接`}
                  >
                    <SelectValue placeholder="选择数据库连接" />
                  </SelectTrigger>
                  <SelectContent>
                    {data.connections.map((d: Data) => (
                      <SelectItem key={d.id} value={d.id}>
                        {d.name}
                        {d.enabled ? "" : "（未启用）"}
                      </SelectItem>
                    ))}
                  </SelectContent>
                </Select>
                <Button
                  variant="ghost"
                  disabled={busy}
                  onClick={() =>
                    setBindings(bindings.filter((_, j) => j !== i))
                  }
                >
                  移除映射
                </Button>
              </div>
            ))}
            <p className="hint"></p>
            <div className="flex gap-3">
              <Button
                variant="outline"
                disabled={
                  !workspace ||
                  busy ||
                  bindings.length >= 8 ||
                  !data.connections.length
                }
                onClick={() =>
                  setBindings([...bindings, { alias: "", connection_id: "" }])
                }
              >
                添加映射
              </Button>
              <Button
                disabled={!workspace || busy}
                onClick={() =>
                  void run(async () => {
                    await request("database_bind", { workspace, bindings });
                    await load();
                    await refreshProjects();
                    setNotice("已保存，新会话生效");
                  })
                }
              >
                保存项目映射
              </Button>
            </div>
          </CardContent>
        </Card>
      )}
      {error && (
        <div role="alert" className="error-box">
          {error}
        </div>
      )}
      <Dialog
        open={!!removing}
        onOpenChange={(v) => {
          if (!busy && !v) setRemoving(null);
        }}
      >
        <DialogContent>
          <DialogHeader>
            <DialogTitle>删除连接配置</DialogTitle>
            <DialogDescription>
              删除连接配置「{removing?.name}」？数据库本身不受影响。
            </DialogDescription>
          </DialogHeader>
          <div className="flex justify-end gap-3">
            <Button
              variant="outline"
              disabled={busy}
              onClick={() => setRemoving(null)}
            >
              取消
            </Button>
            <Button
              variant="destructive"
              disabled={busy}
              onClick={() =>
                void run(async () => {
                  await request("database_remove", { id: removing?.id });
                  setRemoving(null);
                  await load();
                })
              }
            >
              删除
            </Button>
          </div>
          {error && (
            <p role="alert" className="error-box">
              {error}
            </p>
          )}
        </DialogContent>
      </Dialog>
      <Dialog
        open={editing}
        onOpenChange={(v) => {
          if (!busy) {
            setEditing(v);
            if (!v) setForm(empty);
          }
        }}
      >
        <DialogContent className="max-h-[85vh] overflow-y-auto sm:max-w-2xl">
          <DialogHeader>
            <DialogTitle>{form.id ? "编辑连接" : "新增数据库连接"}</DialogTitle>
            <DialogDescription>建议使用只读（SELECT）账号</DialogDescription>
          </DialogHeader>
          <div className="dialog-body">
            <div className="form-two">
              {field("name", "连接名称")}
              <div className="field">
                <Label>数据库类型</Label>
                <Select
                  value={form.kind}
                  onValueChange={(v) =>
                    setForm({
                      ...form,
                      kind: v,
                      port: v === "mysql" ? 3306 : 5432,
                    })
                  }
                >
                  <SelectTrigger aria-label="数据库类型">
                    <SelectValue />
                  </SelectTrigger>
                  <SelectContent>
                    <SelectItem value="mysql">MySQL</SelectItem>
                    <SelectItem value="postgres">PostgreSQL</SelectItem>
                  </SelectContent>
                </Select>
              </div>
              {field("host", "数据库地址")}
              {field("port", "端口", "number")}
              {field("user", "用户名")}
              {field("database", "数据库名")}
              {field(
                "password",
                form.password_saved ? "密码（留空保留已保存密码）" : "密码",
                "password",
              )}
              <div className="field">
                <Label htmlFor="db-enabled">在新任务中启用</Label>
                <Switch
                  id="db-enabled"
                  checked={form.enabled}
                  onCheckedChange={(v) => setForm({ ...form, enabled: v })}
                />
              </div>
            </div>
            <details>
              <summary>远端运行环境</summary>
              <div className="space-y-3 pt-3">
                {field("node", "Node 绝对路径")}
                {field("package_root", "universal-db-mcp 安装目录")}
                <p className="hint"></p>
              </div>
            </details>
            {error && (
              <p role="alert" className="error-box">
                {error}
              </p>
            )}
          </div>
          <DialogFooter>
            <Button
              variant="outline"
              disabled={busy}
              onClick={() => {
                setEditing(false);
                setForm(empty);
              }}
            >
              关闭
            </Button>
            <Button
              disabled={
                busy ||
                !form.name.trim() ||
                !form.host.trim() ||
                !form.user.trim() ||
                !form.database.trim()
              }
              onClick={() =>
                void run(async () => {
                  await request("database_save", { connection: form });
                  setChecks((previous) => {
                    const next = { ...previous };
                    delete next[form.id];
                    return next;
                  });
                  setForm(empty);
                  setEditing(false);
                  await load();
                  setNotice("已保存（未测试）");
                })
              }
            >
              {busy ? "保存中…" : "保存连接"}
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>
    </div>
  );
}
