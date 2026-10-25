import type { ReactNode } from "react";
import { time } from "@/lib/format";
import { FolderOpen, Plus, ArrowRight, Check, ShieldCheck } from "lucide-react";
import { Button } from "./ui/button";
import { Badge } from "./ui/badge";
import { groupSessions } from "@/lib/conversation";
type D = Record<string, any>;
export function Workbench({
  projects,
  tasks,
  currentProject,
  connection,
  defaultModel,
  usage,
  onFiles,
  onSession,
  onPage,
}: {
  projects: D[];
  tasks: D[];
  currentProject: string;
  connection: D;
  defaultModel: string;
  usage: ReactNode;
  onFiles: (p: string) => void;
  onNew: (p: string) => void;
  onSession: (id: string) => void;
  onPage: (p: string) => void;
}) {
  const recent: D[] = groupSessions(
    tasks.filter((t) => !t.session_meta?.archived),
    projects,
  )
    .flatMap((g) => g.sessions.map((t) => ({ ...t, projectName: g.name }) as D))
    .sort(
      (a, b) => (b.updated_at || b.created_at) - (a.updated_at || a.created_at),
    )
    .slice(0, 5);
  return (
    <>
      <div className="workbench-hero">
        <div>
          <small>当前工作区</small>
          <h2>
            {projects.find((p) => p.path === currentProject)?.name || "工作台"}
          </h2>
          <p className="muted">{currentProject || "未选择目录"}</p>
        </div>
        <Button variant="outline" onClick={() => onFiles(currentProject)}>
          打开文件
          <ArrowRight size={15} />
        </Button>
      </div>
      <div className="workgrid">
        <section>
          <div className="section-title">
            <h2>
              项目 <span className="muted">{projects.length}</span>
            </h2>
            <Button
              variant="ghost"
              size="sm"
              onClick={() => onPage("projects")}
            >
              <Plus />
              添加项目
            </Button>
          </div>
          <div className="projectlist">
            {projects.map((p) => (
              <button
                className="projectrow"
                key={p.path}
                onClick={() => onFiles(p.path)}
              >
                <span className="projecthead">
                  <span className="projecticon">
                    <FolderOpen />
                  </span>
                  <span className="grow">
                    <h3>
                      {p.name}{" "}
                      {p.path === currentProject && (
                        <Badge variant="outline">MCP 默认</Badge>
                      )}
                    </h3>
                    <code>{p.path}</code>
                  </span>
                  <ArrowRight size={16} />
                </span>
              </button>
            ))}
            {!projects.length && (
              <div className="empty">
                {connection.connected ? "还没有项目" : "未连接"}
                <Button
                  variant="outline"
                  onClick={() =>
                    onPage(connection.connected ? "projects" : "servers")
                  }
                >
                  {connection.connected ? "添加项目" : "连接服务器"}
                </Button>
              </div>
            )}
          </div>
          <div className="recent-section">
            <div className="section-title">
              <h2>最近会话</h2>
              <Button variant="ghost" onClick={() => onPage("chat")}>
                全部会话
                <ArrowRight />
              </Button>
            </div>
            <table className="recent-table">
              <thead>
                <tr>
                  <th>主题</th>
                  <th>项目</th>
                  <th>最近活动</th>
                </tr>
              </thead>
              <tbody>
                {recent.map((t) => (
                  <tr key={t.task_id} onClick={() => onSession(t.task_id)}>
                    <td>
                      {String(
                        t.session_meta?.title ||
                          t.title ||
                          t.prompt ||
                          t.task_id,
                      )
                        .split("\n")
                        .find((s) => s.trim())
                        ?.slice(0, 32)}
                      {[
                        "running",
                        "queued",
                        "waiting_approval",
                        "failed",
                      ].includes(t.state) && (
                        <Badge>
                          {t.state === "failed"
                            ? "失败"
                            : t.state === "waiting_approval"
                              ? "待审批"
                              : "执行中"}
                        </Badge>
                      )}
                    </td>
                    <td>{t.projectName}</td>
                    <td>{time(t.updated_at || t.created_at)}</td>
                  </tr>
                ))}
              </tbody>
            </table>
            {!recent.length && <p className="empty">还没有会话</p>}
          </div>
        </section>
        <aside>
          <div className="compact-panel">
            <h2>运行环境</h2>
            <div className="status-row">
              <span>Runner</span>
              <span>{connection.probe?.runner_version || "未检测"}</span>
            </div>
            <div className="status-row">
              <span>Claude Code</span>
              <span>
                {connection.probe?.claude_version ? (
                  <>
                    {connection.probe.claude_version.replace(
                      " (Claude Code)",
                      "",
                    )}
                  </>
                ) : (
                  "未检测"
                )}
              </span>
            </div>
            <div className="workbench-model">
              <span>默认模型</span>
              <span title={defaultModel}>{defaultModel}</span>
            </div>
            <div className="status-row">
              <span>执行权限</span>
              <Badge variant="outline">
                {!connection.connected
                  ? "未连接"
                  : connection.profile?.allow_auto
                    ? "允许全自动"
                    : "审批"}
              </Badge>
            </div>
            <Button variant="ghost" onClick={() => onPage("servers")}>
              <ShieldCheck />
              服务器设置
              <ArrowRight />
            </Button>
          </div>
          {usage}
        </aside>
      </div>
    </>
  );
}
