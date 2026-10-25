import { Dialog as Sheet } from "radix-ui";
import { useState } from "react";
import {
  ArrowLeft,
  ArrowUpRight,
  Check,
  ChevronRight,
  ListTodo,
  Loader2,
  Search,
  X,
} from "lucide-react";
import { Button } from "./ui/button";
import { elapsed } from "../lib/format";
import { Input } from "./ui/input";
export type BackgroundTask = {
  id: string;
  label: string;
  summary: string;
  running: boolean;
  open: () => void;
  state?: string;
  local?: boolean;
  project?: string;
  kind?: string;
  model?: string;
  mode?: string;
  error?: string;
  processed?: number;
  total?: number;
  started?: number;
  finished?: number;
  workspace?: string;
  phase?: string;
  verification?: string;
  verifiedAt?: number;
  action?: string;
};
const labels: Record<string, string> = {
  queued: "排队中",
  running: "执行中",
  waiting_approval: "等待审批",
  completed: "已完成",
  done: "已完成",
  cancelled: "已取消",
  failed: "失败",
  partial: "部分完成",
  unknown: "结果待核实",
  interrupted: "已中断",
};
export function BackgroundTasks({
  tasks,
  account,
  connected = true,
  onConnect,
  warning,
}: {
  tasks: BackgroundTask[];
  account?: string;
  connected?: boolean;
  onConnect?: () => void;
  warning?: string;
}) {
  if (!connected)
    tasks = tasks.map((t) =>
      t.running && !t.local
        ? {
            ...t,
            state: "unknown",
            running: false,
            summary: "离线 · 上次状态待核实",
          }
        : t,
    );
  const [open, setOpen] = useState(false),
    [filter, setFilter] = useState("running"),
    [q, setQ] = useState(""),
    [selected, setSelected] = useState("");
  const state = (t: BackgroundTask) =>
    t.state || (t.running ? "running" : "done");
  const inProgress = (t: BackgroundTask) =>
    ["queued", "running"].includes(state(t));
  const needs = (t: BackgroundTask) =>
    [
      "failed",
      "partial",
      "unknown",
      "interrupted",
      "waiting_approval",
    ].includes(state(t));
  const [read, setRead] = useState<Set<string>>(new Set());
  const count = tasks.filter(
    (t) => inProgress(t) || (needs(t) && !read.has(t.id)),
  ).length;
  const visible = tasks.filter(
    (t) =>
      (filter === "all" ||
        (filter === "running" && inProgress(t)) ||
        (filter === "attention" && needs(t)) ||
        (filter === "ended" && !inProgress(t) && !needs(t))) &&
      `${t.label} ${t.project || ""} ${t.kind || ""}`
        .toLowerCase()
        .includes(q.toLowerCase()),
  );
  const detail = tasks.find((t) => t.id === selected);
  return (
    <Sheet.Root
      open={open}
      onOpenChange={(v) => {
        setOpen(v);
        if (!v) setSelected("");
      }}
    >
      <Sheet.Trigger asChild>
        <Button variant="outline" aria-label="任务中心">
          {tasks.some(inProgress) ? (
            <Loader2 size={15} className="spin" />
          ) : (
            <ListTodo size={15} />
          )}
          任务中心{count > 0 && <span className="task-count">{count}</span>}
        </Button>
      </Sheet.Trigger>
      <Sheet.Portal>
        <Sheet.Overlay className="task-center-overlay" />
        <Sheet.Content className={`task-center ${detail ? "has-detail" : ""}`}>
          <header>
            <div>
              <Sheet.Title>任务中心</Sheet.Title>
              <Sheet.Description>
                {account || "当前账号"}
                {!connected ? " · 离线，上次状态" : ""}
              </Sheet.Description>
            </div>
            <Sheet.Close asChild>
              <Button variant="ghost" size="icon" aria-label="关闭任务中心">
                <X size={18} />
              </Button>
            </Sheet.Close>
          </header>
          {warning && (
            <div className="notice" role="alert">
              {warning}
            </div>
          )}
          <div className="task-center-body">
            <section className="task-center-list">
              <div className="task-center-search">
                <Search size={15} />
                <Input
                  placeholder="搜索任务或项目"
                  aria-label="搜索任务"
                  value={q}
                  onChange={(e) => setQ(e.target.value)}
                />
              </div>
              <div className="task-filters">
                {[
                  ["running", "进行中"],
                  ["attention", "需处理"],
                  ["ended", "已结束"],
                  ["all", "全部"],
                ].map(([key, text]) => (
                  <Button
                    key={key}
                    variant={filter === key ? "secondary" : "ghost"}
                    onClick={() => setFilter(key)}
                  >
                    {text}
                  </Button>
                ))}
              </div>
              <div className="task-rows">
                {visible.map((t) => (
                  <button
                    className={`task-center-row ${selected === t.id ? "selected" : ""}`}
                    key={t.id}
                    onClick={() => {
                      setSelected(t.id);
                      setRead((old) => new Set(old).add(t.id));
                    }}
                  >
                    <span
                      className={`task-state-icon ${needs(t) ? "attention" : ""}`}
                    >
                      {inProgress(t) ? (
                        <Loader2 size={16} className="spin" />
                      ) : needs(t) ? (
                        "!"
                      ) : (
                        <Check size={16} />
                      )}
                    </span>
                    <span>
                      <strong>{t.label}</strong>
                      <small>{t.project || t.kind || "后台操作"}</small>
                      <span className="muted">{t.summary}</span>
                    </span>
                    <span className="task-row-end">
                      <small>{labels[state(t)] || state(t)}</small>
                      <ChevronRight size={14} />
                    </span>
                  </button>
                ))}
                {!visible.length && (
                  <div className="empty">
                    {q
                      ? "没有匹配任务"
                      : filter === "running"
                        ? "没有进行中的任务"
                        : "没有此类任务"}
                  </div>
                )}
              </div>
            </section>
            <section className="task-center-detail">
              {detail ? (
                <>
                  <Button
                    variant="ghost"
                    className="task-back"
                    onClick={() => setSelected("")}
                  >
                    <ArrowLeft size={15} />
                    任务列表
                  </Button>
                  <span className="task-type">{detail.kind || "后台任务"}</span>
                  <h2>{detail.label}</h2>
                  <span className="task-state-label">
                    {labels[state(detail)] || state(detail)}
                  </span>
                  <dl>
                    <dt>项目</dt>
                    <dd>{detail.project || "未记录项目"}</dd>
                    {detail.workspace && (
                      <>
                        <dt>目录</dt>
                        <dd>{detail.workspace}</dd>
                      </>
                    )}
                    {detail.started && (
                      <>
                        <dt>耗时</dt>
                        <dd>
                          {elapsed(
                            (detail.finished || Date.now()) - detail.started,
                          )}
                        </dd>
                      </>
                    )}
                    {detail.model && (
                      <>
                        <dt>实际模型</dt>
                        <dd>{detail.model}</dd>
                      </>
                    )}
                    {detail.mode && (
                      <>
                        <dt>执行权限</dt>
                        <dd>
                          {detail.mode === "automatic" ? "全自动" : "审批"}
                        </dd>
                      </>
                    )}
                    <dt>当前阶段</dt>
                    <dd>{detail.phase || detail.summary}</dd>
                    {!!detail.total && (
                      <>
                        <dt>已处理</dt>
                        <dd>
                          {detail.processed || 0} / {detail.total}
                        </dd>
                      </>
                    )}
                  </dl>
                  {detail.verification && (
                    <p className="muted">
                      {detail.verification} ·{" "}
                      {new Date(
                        (detail.verifiedAt || 0) * 1000,
                      ).toLocaleTimeString("zh-CN", {
                        hour: "2-digit",
                        minute: "2-digit",
                      })}{" "}
                      核实
                    </p>
                  )}
                  {detail.error && (
                    <div className="notice" role="alert">
                      {detail.error}
                    </div>
                  )}
                  <Button
                    className="task-detail-action"
                    onClick={() => {
                      setOpen(false);
                      if (!connected && !detail.local && onConnect) onConnect();
                      else detail.open();
                    }}
                  >
                    {!connected ? "连接并核实" : detail.action || "打开原操作"}
                    <ArrowUpRight size={14} />
                  </Button>
                </>
              ) : (
                <div className="empty">选择任务查看详情</div>
              )}
            </section>
          </div>
        </Sheet.Content>
      </Sheet.Portal>
    </Sheet.Root>
  );
}
