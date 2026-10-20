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
  const [workspace, setWorkspace] = useState(currentProject),
    [data, setData] = useState<Data>({ connections: [], bindings: [] }),
    [form, setForm] = useState<Data>(empty),
    [editing, setEditing] = useState(false),
    [bindings, setBindings] = useState<Data[]>([]),
    [runtime, setRuntime] = useState<Data>({}),
    [busy, setBusy] = useState(false),
    [error, setError] = useState(""),
    [notice, setNotice] = useState(""),
    [checks, setChecks] = useState<Data>({});
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
      <Card>
        <CardHeader>
          <CardTitle>数据库连接</CardTitle>
          <CardDescription>
            连接保存在当前 SSH
            账号下。每个项目可以映射多个连接，同一连接也可供多个项目使用。
          </CardDescription>
        </CardHeader>
        <CardContent className="space-y-4">
          <div className="flex flex-wrap gap-3">
            <Button
              disabled={!connected || busy}
              onClick={() =>
                void run(async () => {
                  const r = await request("database_detect", {});
                  setRuntime(r);
                  setNotice(r.notice);
                })
              }
            >
              探测远端 MCP 环境
            </Button>
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
              刷新
            </Button>
          </div>
          {runtime.candidates?.map((c: Data) => (
            <div
              key={c.package_root}
              className="rounded-lg border p-3 space-y-1"
            >
              <Badge variant={c.compatible ? "secondary" : "outline"}>
                {c.compatible ? "运行文件已找到" : "版本暂不兼容"}
              </Badge>
              <p className="break-all">{c.package_root}</p>
              <p className="hint">
                universal-db-mcp {c.version} · {c.source}
              </p>
            </div>
          ))}
          {runtime.registered?.map((r: Data) => (
            <p key={r.name} className="hint">
              已发现 Claude MCP：{r.name} · {r.command}
              （仅识别运行环境，不继承共享凭据与权限）
            </p>
          ))}
          {notice && (
            <p role="status" className="hint">
              {notice}
            </p>
          )}
          {data.connections.length === 0 && (
            <p className="hint">
              还没有连接。探测只查找软件，不会自动读取共享数据库凭据或连接业务数据库。
            </p>
          )}
          {data.connections.map((d: Data) => (
            <div
              key={d.id}
              className="flex flex-wrap items-center justify-between gap-3 rounded-lg border p-4"
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
                  {d.password_saved ? "已保存密码" : "未保存密码"} · 只读 safe ·{" "}
                  {(d.used_by || []).length} 个项目引用
                </p>
                {checks[d.id] && (
                  <p role="status" className="hint">
                    {Object.values(checks[d.id]).join(" · ")}
                  </p>
                )}
              </div>
              <div className="flex flex-wrap gap-2">
                <Button
                  variant="outline"
                  disabled={busy}
                  onClick={() => {
                    setForm({ ...d, password: "" });
                    setEditing(true);
                  }}
                >
                  编辑
                </Button>
                <Button
                  variant="outline"
                  disabled={busy}
                  onClick={() =>
                    void run(async () => {
                      const r = await request("database_protocol_test", {
                        id: d.id,
                      });
                      setChecks((p) => ({
                        ...p,
                        [d.id]: {
                          ...p[d.id],
                          mcp: `MCP：${r.notice}（${new Date().toLocaleTimeString()}）`,
                        },
                      }));
                    })
                  }
                >
                  检查 MCP 握手
                </Button>
                <Button
                  variant="outline"
                  disabled={busy}
                  onClick={() =>
                    void run(async () => {
                      const r = await request("database_test", { id: d.id });
                      setChecks((p) => ({
                        ...p,
                        [d.id]: {
                          ...p[d.id],
                          database: `数据库：${r.notice}（${new Date().toLocaleTimeString()}）`,
                        },
                      }));
                    })
                  }
                >
                  测试数据库连接
                </Button>
                <Button
                  variant="outline"
                  disabled={busy || (d.used_by || []).length > 0}
                  onClick={() => {
                    if (
                      window.confirm(
                        `删除连接“${d.name}”？不会删除数据库或业务数据。`,
                      )
                    )
                      void run(async () => {
                        await request("database_remove", { id: d.id });
                        await load();
                      });
                  }}
                >
                  删除
                </Button>
              </div>
            </div>
          ))}
        </CardContent>
      </Card>
      <Card>
        <CardHeader>
          <CardTitle>项目数据库映射</CardTitle>
          <CardDescription>
            按准确项目目录匹配，不继承父目录或其他项目。变更仅对新会话生效；续聊保留原连接快照。
          </CardDescription>
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
                onClick={() => setBindings(bindings.filter((_, j) => j !== i))}
              >
                移除映射
              </Button>
            </div>
          ))}
          <p className="hint">
            用途别名使用英文字母、数字、下划线，例如
            business_dev、workflow、reporting。Claude 将看到 db_用途别名。
          </p>
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
                  setNotice(
                    "项目映射已保存；已有会话保持原配置，新会话使用新映射。",
                  );
                })
              }
            >
              保存项目映射
            </Button>
          </div>
        </CardContent>
      </Card>
      {error && (
        <div role="alert" className="error-box">
          {error}
        </div>
      )}
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
            <DialogDescription>
              固定只读 safe，建议使用数据库本身仅有 SELECT
              权限的账号。密码不显示在列表、模型指令或 MCP 启动参数中。
            </DialogDescription>
          </DialogHeader>
          <div className="grid grid-cols-2 gap-4">
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
              <p className="hint">
                可先在页面探测。当前适配 universal-db-mcp
                2.14.0；发现其他版本时明确提示，不自动改服务器依赖。
              </p>
            </div>
          </details>
          {error && (
            <p role="alert" className="error-box">
              {error}
            </p>
          )}
          <Button
            disabled={busy}
            onClick={() =>
              void run(async () => {
                await request("database_save", { connection: form });
                setForm(empty);
                setEditing(false);
                await load();
                setNotice(
                  "连接已保存，尚未测试数据库。请添加项目映射后开启新会话。",
                );
              })
            }
          >
            {busy ? "保存中…" : "保存连接"}
          </Button>
        </DialogContent>
      </Dialog>
    </div>
  );
}
