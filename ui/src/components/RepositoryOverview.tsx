import { useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { GitBranch, RefreshCw, MoreHorizontal, FolderOpen } from "lucide-react";
import { DropdownMenu } from "radix-ui";
import { Button } from "./ui/button";
import { InlineAlert } from "./common/Feedback";
import { repositoryStatus, remoteTime } from "@/lib/repositoryOverview";
import { RepositorySettings } from "./RepositorySettings";
import { errorText } from "./CodeupAuth";
type D = Record<string, any>;
export function RepositoryOverview({
  path,
  connectionId,
  refreshKey,
  onUpdate,
  onBranches,
  onFetch,
  onAuth,
  onBrowse,
  onOpenTask,
}: {
  path: string;
  connectionId: string;
  refreshKey: number;
  onUpdate: (path: string) => void;
  onBranches: (path: string) => void;
  onFetch: (path: string, remote?: string) => void;
  onAuth: (url: string) => void;
  onBrowse: (path: string) => void;
  onOpenTask: (id: string) => void;
}) {
  const [repos, setRepos] = useState<D[]>([]),
    [selected, setSelected] = useState(""),
    [busy, setBusy] = useState(false),
    [empty, setEmpty] = useState(false),
    [error, setError] = useState(""),
    [settings, setSettings] = useState<"remote" | "upstream" | null>(null),
    [showFiles, setShowFiles] = useState(false);
  const generation = useRef(0);
  async function load() {
    const seq = ++generation.current;
    setBusy(true);
    setEmpty(false);
    setError("");
    try {
      const r = await invoke<D>("git_update_plan", {
        request: { path, connection_id: connectionId },
      });
      if (seq !== generation.current) return;
      setRepos(r.repositories || []);
      setEmpty(!r.repositories?.length);
      setSelected((old) =>
        (r.repositories || []).some((r: D) => r.path === old)
          ? old
          : r.repositories?.[0]?.path || "",
      );
    } catch (e) {
      if (seq === generation.current) {
        setRepos([]);
        const message = errorText(e);
        if (
          message.replace(/\s/g, "").startsWith("此目录及直接子目录没有Git仓库")
        )
          setEmpty(true);
        else setError(message);
      }
    } finally {
      if (seq === generation.current) setBusy(false);
    }
  }
  useEffect(() => {
    setRepos([]);
    setSettings(null);
    setShowFiles(false);
    setSelected("");
    void load();
    return () => {
      generation.current++;
    };
  }, [path, connectionId]);
  const lastRefresh = useRef(refreshKey);
  useEffect(() => {
    if (lastRefresh.current !== refreshKey) {
      lastRefresh.current = refreshKey;
      void load();
    }
  }, [refreshKey]);
  const r = repos.find((r) => r.path === selected);
  const status = r ? repositoryStatus(r) : null;
  const blocked =
    r &&
    (r.dirty ||
      !r.upstream ||
      !r.tracking_known ||
      (r.ahead > 0 && r.behind > 0) ||
      r.blockers?.length > 0);
  if (empty && !busy)
    return (
      <section className="repo-overview" aria-label="仓库总览">
        <h3>仓库</h3>
        <p className="hint">暂无仓库</p>
      </section>
    );
  return (
    <section className="repo-overview" aria-label="仓库总览" aria-busy={busy}>
      <div className="repo-overview-head">
        <div>
          <h3>仓库</h3>
          <small>{repos.length ? `${repos.length} 个仓库` : ""}</small>
        </div>
        <div className="repo-inline-actions">
          <Button
            variant="ghost"
            size="icon"
            aria-label="刷新本地仓库状态"
            disabled={busy}
            onClick={() => void load()}
          >
            <RefreshCw size={15} />
          </Button>
          <Button
            variant="outline"
            size="sm"
            disabled={busy || !repos.length}
            onClick={() => onFetch(path)}
          >
            获取最新状态
          </Button>
          <Button
            size="sm"
            disabled={busy || !repos.length}
            onClick={() => onUpdate(path)}
          >
            全部更新
          </Button>
        </div>
      </div>
      {error ? (
        <InlineAlert>
          {error}
          <Button size="sm" variant="ghost" onClick={() => void load()}>
            重新检查
          </Button>
        </InlineAlert>
      ) : (
        <>
          <div className="repo-table" role="table" aria-label="项目仓库">
            <div className="repo-table-heading" role="row">
              <span>仓库</span>
              <span>当前分支</span>
              <span>更新状态</span>
            </div>
            {repos.map((repo) => {
              const s = repositoryStatus(repo);
              return (
                <button
                  key={repo.path}
                  className={`repo-table-row ${repo.path === selected ? "selected" : ""}`}
                  aria-pressed={repo.path === selected}
                  onClick={() => {
                    setSelected(repo.path);
                    setSettings(null);
                    setShowFiles(false);
                  }}
                >
                  <strong title={repo.path}>{repo.name}</strong>
                  <span className="repo-code" title={repo.branch}>
                    {repo.branch}
                  </span>
                  <span className={`repo-status-${s.tone}`}>{s.label}</span>
                </button>
              );
            })}
          </div>
          {r && (
            <div className="repo-selected">
              <div className="repo-selected-title">
                <strong>{r.name}</strong>
                <Button
                  variant="ghost"
                  size="sm"
                  onClick={() => onBrowse(r.path)}
                >
                  <FolderOpen size={14} />
                  浏览文件
                </Button>
              </div>
              <div className="repo-tracking">
                <span>{r.upstream || "未设置上游"}</span>
                <span>{remoteTime(r)}</span>
              </div>
              <div className="repo-inline-actions">
                <Button
                  variant="outline"
                  size="sm"
                  onClick={() => onBranches(r.path)}
                >
                  <GitBranch size={14} />
                  {r.branch}
                </Button>
                <Button
                  size="sm"
                  disabled={!!blocked || busy}
                  onClick={() => onUpdate(r.path)}
                >
                  更新仓库
                </Button>
                <Button
                  variant="outline"
                  size="sm"
                  onClick={() => setSettings("remote")}
                >
                  远端与认证
                </Button>
                <DropdownMenu.Root>
                  <DropdownMenu.Trigger asChild>
                    <Button variant="ghost" size="icon" aria-label="仓库操作">
                      <MoreHorizontal size={15} />
                    </Button>
                  </DropdownMenu.Trigger>
                  <DropdownMenu.Portal>
                    <DropdownMenu.Content
                      className="context-menu"
                      sideOffset={5}
                    >
                      <DropdownMenu.Item
                        onSelect={() => setSettings("upstream")}
                      >
                        设置上游…
                      </DropdownMenu.Item>
                      <DropdownMenu.Item onSelect={() => onBranches(r.path)}>
                        管理与清理分支…
                      </DropdownMenu.Item>
                    </DropdownMenu.Content>
                  </DropdownMenu.Portal>
                </DropdownMenu.Root>
              </div>
              {(blocked || !r.last_fetch) && (
                <div className="repo-context">
                  <strong>{status?.label}</strong>
                  {r.blockers?.length > 0 ? (
                    <>
                      {r.blockers.map((b: D) => (
                        <div className="repo-blocker" key={b.id || b.title}>
                          <span>
                            {b.title || "目录操作"} ·{" "}
                            {b.ownership_unknown
                              ? "状态未知"
                              : b.state === "running"
                                ? "执行中"
                                : b.state}
                          </span>
                          {b.kind === "task" && b.id && (
                            <Button
                              variant="ghost"
                              size="sm"
                              onClick={() => onOpenTask(b.id)}
                            >
                              查看任务
                            </Button>
                          )}
                        </div>
                      ))}
                      <p className="hint">任务按目录记录，未确认具体分支。</p>
                    </>
                  ) : r.dirty ? (
                    <>
                      <Button
                        variant="ghost"
                        size="sm"
                        onClick={() => setShowFiles((v) => !v)}
                      >
                        查看文件
                      </Button>
                      {showFiles && (
                        <ul className="repo-dirty-list">
                          {r.changed_files?.map((s: string) => (
                            <li key={s}>
                              {s.replace(/^\s*[MADRCU?! ]+\s/, "")}
                            </li>
                          ))}
                        </ul>
                      )}
                    </>
                  ) : !r.upstream ? (
                    <Button
                      variant="ghost"
                      size="sm"
                      onClick={() => setSettings("upstream")}
                    >
                      设置上游…
                    </Button>
                  ) : !r.tracking_known || !r.last_fetch ? (
                    <Button
                      variant="ghost"
                      size="sm"
                      onClick={() => onFetch(r.path, r.remote)}
                    >
                      获取最新状态
                    </Button>
                  ) : r.ahead > 0 && r.behind > 0 ? (
                    <p className="hint">
                      本地领先 {r.ahead} · 远端领先 {r.behind}，请先处理分叉。
                    </p>
                  ) : null}
                </div>
              )}
            </div>
          )}
        </>
      )}
      {settings && r && (
        <RepositorySettings
          key={`${connectionId}:${r.path}:${settings}`}
          repo={r}
          connectionId={connectionId}
          kind={settings}
          onClose={() => setSettings(null)}
          onChanged={() => void load()}
          onAuth={(url) => {
            setSettings(null);
            onAuth(url);
          }}
          onFetch={(p, remote) => {
            setSettings(null);
            onFetch(p, remote);
          }}
        />
      )}
    </section>
  );
}
