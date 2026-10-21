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
  onFiles,
  onNew,
  onSession,
  onPage,
}: {
  projects: D[];
  tasks: D[];
  currentProject: string;
  connection: D;
  onFiles: (p: string) => void;
  onNew: (p: string) => void;
  onSession: (id: string) => void;
  onPage: (p: string) => void;
}) {
  const recent: D[] = groupSessions(tasks, projects)
    .flatMap((g) => g.sessions.map((t) => ({ ...t, projectName: g.name }) as D))
    .sort(
      (a, b) => (b.updated_at || b.created_at) - (a.updated_at || a.created_at),
    )
    .slice(0, 5);
  return (
    <div className="workgrid">
      <section>
        <div className="section-title">
          <h2>
            项目 <span className="muted">{projects.length}</span>
          </h2>
          <Button variant="ghost" size="sm" onClick={() => onPage("projects")}>
            <Plus />
            添加项目
          </Button>
        </div>
        <div className="projectlist">
          {projects.map((p) => (
            <div className="projectrow" key={p.path}>
              <div className="projecthead">
                <span className="projecticon">
                  <FolderOpen />
                </span>
                <div className="grow">
                  <h3>
                    {p.name}{" "}
                    {p.path === currentProject && (
                      <Badge variant="outline">MCP 默认</Badge>
                    )}
                  </h3>
                  <code>{p.path}</code>
                </div>
                <Button variant="ghost" onClick={() => onFiles(p.path)}>
                  <FolderOpen />
                  文件
                </Button>
                <Button variant="outline" onClick={() => onNew(p.path)}>
                  <Plus />
                  新任务
                </Button>
              </div>
            </div>
          ))}
          {!projects.length && (
            <div className="empty">
              {connection.connected ? "还没有项目" : "连接服务器后选择项目"}
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
                    {String(t.prompt || t.title || t.task_id)
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
                  <td>
                    {new Date(
                      (t.updated_at || t.created_at) * 1000,
                    ).toLocaleDateString()}
                  </td>
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
                  <Check size={13} />
                  已就绪
                </>
              ) : (
                "未检测"
              )}
            </span>
          </div>
          <div className="status-row">
            <span>执行权限</span>
            <Badge variant="outline">
              {connection.profile?.allow_auto ? "允许全自动" : "审批"}
            </Badge>
          </div>
          <Button variant="ghost" onClick={() => onPage("servers")}>
            <ShieldCheck />
            管理连接与权限
            <ArrowRight />
          </Button>
        </div>
        <div className="compact-panel">
          <h2>快捷操作</h2>
          <Button variant="ghost" onClick={() => onPage("projects")}>
            克隆仓库
            <ArrowRight />
          </Button>
          <Button variant="ghost" onClick={() => onPage("codex")}>
            配置 Codex MCP
            <ArrowRight />
          </Button>
        </div>
      </aside>
    </div>
  );
}
