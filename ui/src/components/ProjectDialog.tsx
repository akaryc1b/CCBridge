import { useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import {
  ArrowUp,
  Check,
  ChevronRight,
  Folder,
  FolderOpen,
  RefreshCw,
} from "lucide-react";
import { Button } from "./ui/button";
import { Input } from "./ui/input";
import {
  Dialog,
  DialogContent,
  DialogHeader,
  DialogTitle,
  DialogDescription,
  DialogFooter,
} from "./ui/dialog";
import type { RecordValue as Data } from "../lib/conversation";
import { latestNavigation } from "../lib/navigation";
import {
  projectAtPath,
  parentDirectory,
  directoryName,
} from "../lib/projectEntries";

/** Uses the existing directory_list/project_save APIs; never selects the MCP workspace. */
export function ProjectDialog({
  open,
  onClose,
  connectionId,
  connected,
  home,
  account,
  projects,
  initialPath,
  currentOnly,
  onAdded,
}: {
  open: boolean;
  onClose: () => void;
  connectionId: string;
  connected: boolean;
  home: string;
  account: string;
  projects: Data[];
  initialPath: string;
  currentOnly: boolean;
  onAdded: (project: Data) => Promise<void>;
}) {
  const [path, setPath] = useState(initialPath || home),
    [listing, setListing] = useState<Data>({}),
    [name, setName] = useState(""),
    [loading, setLoading] = useState(false),
    [saving, setSaving] = useState(false),
    [error, setError] = useState("");
  const mounted = useRef(true),
    saveLock = useRef(false),
    readGeneration = useRef(0);
  const navigation = useRef<ReturnType<typeof latestNavigation<Data>> | null>(
    null,
  );
  if (!navigation.current)
    navigation.current = latestNavigation<Data>(
      (p) =>
        invoke("directory_list", {
          request: { path: p, connection_id: connectionId },
        }),
      (r) => {
        setListing(r);
        setPath(r.path);
        setName(directoryName(r.path));
      },
      setLoading,
    );
  useEffect(() => {
    mounted.current = true;
    return () => {
      mounted.current = false;
      navigation.current?.invalidate();
    };
  }, []);
  async function browse(p: string) {
    const generation = ++readGeneration.current;
    setPath(p);
    setError("");
    try {
      await navigation.current!(p);
    } catch (e: any) {
      if (mounted.current && generation === readGeneration.current)
        setError(e.message || String(e));
    }
  }
  useEffect(() => {
    if (open && connected) void browse(initialPath || home);
  }, [open]); // Component keyed by connection and entry in App.
  const existing = projectAtPath(listing.path || "", projects);
  const ready =
    connected && !loading && !!listing.path && path === listing.path;
  return (
    <Dialog open={open} onOpenChange={(v) => !v && !saving && onClose()}>
      <DialogContent
        onOpenAutoFocus={(e) => {
          if (!currentOnly) e.preventDefault();
        }}
        className={
          currentOnly ? "project-register-dialog" : "project-picker-dialog"
        }
      >
        <DialogHeader>
          <DialogTitle>{currentOnly ? "添加为项目" : "添加项目"}</DialogTitle>
          <DialogDescription>
            {currentOnly ? "使用当前远端文件夹" : "选择远端文件夹"}
          </DialogDescription>
        </DialogHeader>
        {!connected && (
          <div role="alert" className="notice">
            先连接服务器，再添加项目
          </div>
        )}
        {currentOnly ? (
          <div className="project-path-summary">
            <FolderOpen size={20} />
            <div>
              <strong>{directoryName(listing.path || initialPath)}</strong>
              <small>{listing.path || initialPath}</small>
            </div>
          </div>
        ) : (
          <>
            <div className="project-picker-account">
              <span>{account}</span>
              <span>远端文件夹</span>
            </div>
            <div className="project-picker-path">
              <Button
                aria-label="上一级目录"
                variant="outline"
                size="icon"
                disabled={!ready || listing.path === home || saving}
                onClick={() => void browse(parentDirectory(listing.path, home))}
              >
                <ArrowUp size={15} />
              </Button>
              <Input
                aria-label="项目远端目录"
                value={path}
                disabled={!connected || saving}
                onChange={(e) => setPath(e.target.value)}
                onKeyDown={(e) => e.key === "Enter" && void browse(path)}
              />
              <Button
                variant="ghost"
                aria-label="读取目录"
                disabled={!connected || loading || saving}
                onClick={() => void browse(path)}
              >
                <RefreshCw size={15} className={loading ? "spin" : ""} />
              </Button>
            </div>
            <div className="project-picker-folders" aria-busy={loading}>
              {loading ? (
                <p className="empty">读取目录…</p>
              ) : (
                <>
                  <div className="project-picker-current">
                    <FolderOpen size={16} />
                    {directoryName(listing.path || path)}
                  </div>
                  {(listing.entries || [])
                    .filter((e: Data) =>
                      ["directory", "repository"].includes(e.kind),
                    )
                    .map((e: Data) => (
                      <button
                        disabled={!ready || saving}
                        key={e.path}
                        onClick={() => void browse(e.path)}
                      >
                        <Folder size={17} />
                        <span>{e.name}</span>
                        {projectAtPath(e.path, projects) ? (
                          <small>已添加</small>
                        ) : (
                          <ChevronRight size={14} />
                        )}
                      </button>
                    ))}
                  {!(listing.entries || []).some((e: Data) =>
                    ["directory", "repository"].includes(e.kind),
                  ) &&
                    listing.path && <p className="empty">没有子文件夹</p>}
                  {listing.truncated && (
                    <div className="notice">
                      仅显示前 1000 项，可输入准确路径
                    </div>
                  )}
                </>
              )}
            </div>
          </>
        )}
        <label className="project-name-field">
          项目名称
          <Input
            autoFocus={currentOnly}
            aria-label="项目名称"
            value={name}
            disabled={!ready || saving || !!existing}
            maxLength={80}
            onChange={(e) => setName(e.target.value)}
          />
        </label>
        {existing && (
          <div className="project-already">
            <Check size={15} />
            已添加为项目 · {existing.name}
          </div>
        )}
        {error && (
          <div role="alert" className="notice">
            {error}
          </div>
        )}
        <DialogFooter>
          <Button variant="outline" disabled={saving} onClick={onClose}>
            取消
          </Button>
          <Button
            disabled={!ready || saving || !!existing || !name.trim()}
            onClick={async () => {
              if (saveLock.current) return;
              saveLock.current = true;
              setSaving(true);
              setError("");
              try {
                const r = await invoke<Data>("project_save", {
                  request: {
                    path: listing.path,
                    workspace: listing.path,
                    name: name.trim(),
                    create_only: true,
                    connection_id: connectionId,
                  },
                });
                if (!mounted.current) return;
                await onAdded(r);
                if (mounted.current) onClose();
              } catch (e: any) {
                if (mounted.current) setError(e.message || String(e));
              } finally {
                saveLock.current = false;
                if (mounted.current) setSaving(false);
              }
            }}
          >
            {saving ? "添加中…" : "添加项目"}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
