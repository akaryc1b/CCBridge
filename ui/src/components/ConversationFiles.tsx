import { useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import {
  Folder,
  FileText,
  ChevronRight,
  ChevronDown,
  RefreshCw,
} from "lucide-react";
import { Button } from "./ui/button";
import { Input } from "./ui/input";
import { InlineAlert } from "./common/Feedback";
import { FileActions } from "./WorkspaceTools";
import { errorText } from "./CodeupAuth";
type D = Record<string, any>;
export function ConversationFiles({
  root,
  connectionId,
  connected,
  onReference,
  onManage,
  onAction,
  revision = 0,
}: {
  root: string;
  connectionId: string;
  connected: boolean;
  onReference: (path: string) => void;
  onManage: (path: string) => void;
  onAction: (action: string, entry: D) => void;
  revision?: number;
}) {
  const [cache, setCache] = useState<Record<string, D>>({}),
    [expanded, setExpanded] = useState<Set<string>>(new Set()),
    [file, setFile] = useState<D | null>(null),
    [text, setText] = useState(""),
    [error, setError] = useState(""),
    [filter, setFilter] = useState(""),
    [loading, setLoading] = useState(false);
  const generation = useRef(0),
    previewSeq = useRef(0);
  async function load(path: string) {
    const seq = generation.current;
    setError("");
    try {
      const r = await invoke<D>("directory_list", {
        request: { path, hidden: false, connection_id: connectionId },
      });
      if (seq === generation.current)
        setCache((old) => ({ ...old, [path]: r }));
    } catch (e) {
      if (seq === generation.current) setError(errorText(e));
    }
  }
  useEffect(() => {
    generation.current++;
    previewSeq.current++;
    setCache({});
    setFile(null);
    setText("");
    setFilter("");
    setExpanded(new Set([root]));
    if (connected && root) void load(root);
    return () => {
      generation.current++;
      previewSeq.current++;
    };
  }, [root, connectionId, connected, revision]);
  async function select(entry: D) {
    const seq = ++previewSeq.current,
      gen = generation.current;
    setFile(entry);
    setText("");
    setError("");
    setLoading(true);
    try {
      const r = await invoke<D>("file_preview", {
        request: { path: entry.path, connection_id: connectionId },
      });
      if (seq === previewSeq.current && gen === generation.current)
        setText(typeof r.text === "string" ? r.text : "此文件不支持文本预览");
    } catch (e) {
      if (seq === previewSeq.current && gen === generation.current)
        setError(errorText(e));
    } finally {
      if (seq === previewSeq.current && gen === generation.current)
        setLoading(false);
    }
  }
  function toggle(path: string) {
    setExpanded((old) => {
      const next = new Set(old);
      if (next.has(path)) next.delete(path);
      else next.add(path);
      return next;
    });
    if (!cache[path]) void load(path);
  }
  function tree(path: string, depth: number): any {
    if (depth > 24)
      return <p className="muted">目录层级过深，请打开完整文件管理</p>;
    const listing = cache[path];
    if (!listing) return <p className="muted">读取中…</p>;
    return (
      <>
        {(listing.entries || [])
          .filter(
            (e: D) =>
              !filter ||
              e.name.toLowerCase().includes(filter.toLowerCase()) ||
              expanded.has(e.path),
          )
          .map((e: D) => {
            const dir = ["directory", "repository"].includes(e.kind);
            return (
              <div key={e.path}>
                <div
                  className="conversation-file-row"
                  style={{ paddingLeft: depth * 12 }}
                >
                  <button
                    disabled={!connected}
                    aria-expanded={dir ? expanded.has(e.path) : undefined}
                    onClick={() => (dir ? toggle(e.path) : void select(e))}
                  >
                    {dir ? (
                      expanded.has(e.path) ? (
                        <ChevronDown size={12} />
                      ) : (
                        <ChevronRight size={12} />
                      )
                    ) : (
                      <FileText size={13} />
                    )}
                    {dir && <Folder size={13} />}
                    <span title={e.path}>{e.name}</span>
                  </button>
                  <FileActions
                    entry={e}
                    disabled={!connected}
                    onAction={onAction}
                  />
                </div>
                {dir && expanded.has(e.path) && tree(e.path, depth + 1)}
              </div>
            );
          })}
        {listing.truncated && (
          <p className="muted">目录项过多，请在完整文件管理中查看</p>
        )}
      </>
    );
  }
  return (
    <section className="conversation-files">
      <div className="row-heading">
        <span title={root}>{root}</span>
        <Button
          size="sm"
          variant="ghost"
          disabled={!connected}
          onClick={() => void load(root)}
          aria-label="刷新会话目录"
        >
          <RefreshCw size={14} />
        </Button>
        <Button
          size="sm"
          variant="outline"
          onClick={() =>
            onManage(
              file?.path
                ? file.path.slice(0, file.path.lastIndexOf("/"))
                : root,
            )
          }
        >
          完整文件管理
        </Button>
      </div>
      {error && (
        <InlineAlert>
          {error}
          <Button
            size="sm"
            variant="ghost"
            disabled={!connected}
            onClick={() => void load(root)}
          >
            重新检查
          </Button>
        </InlineAlert>
      )}
      <div className="conversation-files-grid">
        <div className="conversation-tree">
          <Input
            aria-label="筛选已展开目录"
            placeholder="筛选目录项…"
            value={filter}
            onChange={(e) => setFilter(e.target.value)}
          />
          {connected ? (
            tree(root, 0)
          ) : (
            <p className="muted">连接后浏览工作区</p>
          )}
        </div>
        <div className="conversation-file-preview">
          {file ? (
            <>
              <div className="row-heading">
                <strong>{file.name}</strong>
                <span className="muted">只读预览</span>
              </div>
              <pre>{loading ? "读取中…" : text}</pre>
              <Button
                size="sm"
                disabled={!connected || loading || !!error}
                onClick={() => onReference(file.path)}
              >
                引用到会话
              </Button>
            </>
          ) : (
            <p className="muted">选择文件预览</p>
          )}
        </div>
      </div>
    </section>
  );
}
