import { useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import {
  MoreHorizontal,
  RefreshCw,
  Server,
  Check,
  AlertCircle,
} from "lucide-react";
import { DropdownMenu as Menu, ToggleGroup } from "radix-ui";
import { Button } from "./ui/button";
import { Input } from "./ui/input";
import { Label } from "./ui/label";
import {
  Dialog,
  DialogContent,
  DialogHeader,
  DialogTitle,
  DialogDescription,
  DialogFooter,
} from "./ui/dialog";
import { InlineAlert } from "./common/Feedback";
import { Confirm } from "./common/Confirm";
import {
  authLabel,
  syncRequest,
  sourceLabel,
  accessOperation,
  credentialRecovery,
  type AuthStatus,
} from "../lib/codeupAuth";
type Data = Record<string, any>;
export const errorText = (e: unknown) =>
  typeof e === "object" && e !== null && "message" in e
    ? String((e as Data).message)
    : String(e);
export function CodeupAuth({
  open,
  onOpenChange,
  connected,
  account,
  url: initialUrl,
  initialScope = "local",
  onSaved,
  consented = false,
  onConsent,
  onActivity,
  requestVersion = 0,
}: {
  open: boolean;
  onOpenChange: (v: boolean) => void;
  connected: boolean;
  account: string;
  url: string;
  initialScope?: string;
  onSaved?: () => void;
  consented?: boolean;
  onConsent?: () => void;
  onActivity?: (activity: string, id?: string) => void;
  requestVersion?: number;
}) {
  const [scope, setScope] = useState(initialScope),
    [url, setUrl] = useState(initialUrl);
  const [status, setStatus] = useState<AuthStatus>({ loading: true }),
    [local, setLocal] = useState<AuthStatus>({ loading: true }),
    [vps, setVps] = useState<AuthStatus>({});
  const [editor, setEditor] = useState<null | "current" | "remote-app">(null),
    [username, setUsername] = useState(""),
    [secret, setSecret] = useState("");
  const [access, setAccess] = useState<Data>({}),
    [busy, setBusy] = useState(""),
    [error, setError] = useState(""),
    [confirmation, setConfirmation] = useState<Data | null>(null);
  const configIdentity = useRef("");
  const generation = useRef(0),
    runLock = useRef(false);
  const remote = scope === "remote";
  useEffect(() => {
    if (open && !runLock.current) {
      setScope(initialScope);
      setUrl(initialUrl);
    }
  }, [initialScope, initialUrl, requestVersion]);
  async function inspect(seq: number, pane: string) {
    const current = () => seq === generation.current;
    const app = invoke<AuthStatus>("git_auth_status", {
      request: { scope: pane },
    }).catch((e): AuthStatus => ({ error: errorText(e) }));
    const localInfo =
      pane === "local"
        ? app
        : invoke<AuthStatus>("git_auth_status", {
            request: { scope: "local" },
          }).catch((e): AuthStatus => ({ error: errorText(e) }));
    const server =
      pane === "remote" && connected
        ? invoke<AuthStatus>("git_vps_auth_status", { request: {} }).catch(
            (e): AuthStatus => ({ error: errorText(e) }),
          )
        : Promise.resolve<AuthStatus>({});
    const [a, l, v] = await Promise.all([app, localInfo, server]);
    if (!current()) return;
    const identity = JSON.stringify([
      a.configured,
      a.storage,
      a.username,
      a.helper_configured,
      a.error,
      v.configured,
      v.username,
      v.entry_present,
      v.included,
      v.error,
    ]);
    if (configIdentity.current && configIdentity.current !== identity)
      setAccess({});
    configIdentity.current = identity;
    setStatus(a);
    setLocal(l);
    setVps(v);
  }
  useEffect(() => {
    ++generation.current;
    onActivity?.("");
    configIdentity.current = "";
    setEditor(null);
    setUsername("");
    setSecret("");
    setAccess({});
    setConfirmation(null);
    setError("");
    setBusy("");
    setStatus({ loading: true });
    setLocal({ loading: true });
    setVps({ loading: remote && connected });
    return () => {
      generation.current++;
    };
  }, [scope, account, connected]);
  useEffect(() => {
    if (open && !runLock.current && (!remote || connected))
      void inspect(generation.current, scope);
  }, [open, scope, account, connected]);
  useEffect(() => {
    setAccess({});
    setConfirmation(null);
  }, [url, username, secret, editor]);
  const offline = remote && !connected;
  const disabled = !!busy || status.loading === true || offline;
  const legacy = status.configured && status.storage !== "file";
  const missing =
    !status.loading &&
    !status.error &&
    !status.configured &&
    !status.helper_configured;
  const showForm = !!editor || (!remote && missing);
  const appScope = editor === "remote-app" ? "remote" : scope;
  function edit(kind: "current" | "remote-app" = "current") {
    setEditor(kind);
    setUsername(kind === "remote-app" || !remote ? status.username || "" : "");
    setSecret("");
    setError("");
  }
  const baseRequest = () => ({
    scope: appScope,
    url,
    username: showForm ? username : "",
    secret: showForm ? secret : "",
    storage: "file",
    vps_account: account,
  });
  const draft = showForm && !!secret;
  const testOp = accessOperation(scope, vps, draft);
  async function run(op: string, req: Data = baseRequest()) {
    if (runLock.current) {
      setError("认证操作正在进行");
      return;
    }
    runLock.current = true;
    const seq = generation.current;
    const operationId = crypto.randomUUID();
    setBusy(op);
    setConfirmation(null);
    onActivity?.("认证处理中…", operationId);
    setError("");
    if (op === "git_access" || op === "git_vps_auth_test")
      setAccess({ testing: true });
    try {
      if (!op.endsWith("status"))
        await invoke<Data>(op, {
          request: { ...req, operation_id: operationId },
        });
      if (seq !== generation.current) return;
      if (op === "git_access" || op === "git_vps_auth_test") {
        setAccess({
          ok: true,
          time: new Date().toLocaleTimeString("zh-CN", {
            hour: "2-digit",
            minute: "2-digit",
          }),
        });
      } else {
        await inspect(seq, scope);
        if (seq !== generation.current) return;
        if (!op.endsWith("status")) {
          setAccess({});
          setEditor(null);
          setUsername("");
          setSecret("");
          if (op === "git_auth_save" || op === "git_auth_migrate")
            onConsent?.();
          if (
            onSaved &&
            (op === "git_auth_save" ||
              op === "git_auth_migrate" ||
              (op === "git_vps_auth_install" && remote))
          ) {
            onOpenChange(false);
            onSaved();
          }
        }
      }
      setConfirmation(null);
      onActivity?.("认证操作完成", operationId);
    } catch (e) {
      if (seq !== generation.current) return;
      onActivity?.("认证操作失败", operationId);
      if (op === "git_access" || op === "git_vps_auth_test")
        setAccess({ error: errorText(e) });
      else setError(errorText(e));
      // A retry requires a fresh confirmation; acknowledgement is never persisted.
    } finally {
      runLock.current = false;
      if (seq === generation.current) setBusy("");
    }
  }
  function confirm(op: string, req: Data = baseRequest(), source?: string) {
    setError("");
    if ((op === "git_auth_save" || op === "git_auth_migrate") && consented) {
      void run(op, { ...req, storage_ack: true });
      return;
    }
    setConfirmation({ op, request: req, source });
  }
  function sync(source = "git_helper") {
    confirm(
      "git_vps_auth_install",
      syncRequest(source, account, url, username, secret),
      source,
    );
  }
  const isVps = confirmation?.op === "git_vps_auth_install",
    isRemove = confirmation?.op?.includes("remove");
  const refresh = () =>
    void run(
      remote ? "git_vps_auth_status" : "git_auth_status",
      remote ? {} : { scope: "local" },
    );
  const appMenu = (
    <Menu.Root>
      <Menu.Trigger asChild>
        <Button
          aria-label={remote ? "服务器认证操作" : "本机认证操作"}
          variant="ghost"
          size="icon-sm"
          disabled={disabled}
        >
          <MoreHorizontal size={17} />
        </Button>
      </Menu.Trigger>
      <Menu.Portal>
        <Menu.Content className="context-menu">
          <Menu.Item onSelect={() => edit()}>更换认证</Menu.Item>
          {remote && (
            <>
              <Menu.Item
                disabled={!!vps.loading || !!vps.error}
                onSelect={() => sync()}
              >
                重新同步本机 Git 认证…
              </Menu.Item>
              {local.configured && local.storage === "file" && (
                <Menu.Item onSelect={() => sync("local_app")}>
                  同步 App 本机认证…
                </Menu.Item>
              )}
              {status.configured && status.storage === "file" && (
                <Menu.Item onSelect={() => sync("remote_app")}>
                  同步 App 保存的服务器认证…
                </Menu.Item>
              )}
              <Menu.Item onSelect={() => edit("remote-app")}>
                仅保存到 App…
              </Menu.Item>
              {(vps.files_present || vps.configured) && (
                <Menu.Item onSelect={() => confirm("git_vps_auth_remove")}>
                  移除服务器认证…
                </Menu.Item>
              )}
            </>
          )}
          {status.configured && (
            <>
              {legacy && (
                <Menu.Item
                  onSelect={() => confirm("git_auth_migrate", { scope })}
                >
                  迁移 App {remote ? "服务器认证" : "本机认证"}…
                </Menu.Item>
              )}
              <Menu.Item onSelect={() => confirm("git_auth_remove", { scope })}>
                移除 App {remote ? "服务器认证" : "本机认证"}…
              </Menu.Item>
            </>
          )}
        </Menu.Content>
      </Menu.Portal>
    </Menu.Root>
  );
  return (
    <>
      <Dialog
        open={open}
        onOpenChange={(v) => {
          onOpenChange(v);
        }}
      >
        <DialogContent className="codeup-dialog codeup-compact">
          <DialogHeader>
            <DialogTitle>Codeup 认证</DialogTitle>
            <DialogDescription className="sr-only">
              本机与服务器 Codeup 认证
            </DialogDescription>
          </DialogHeader>
          <div className="dialog-body">
            <ToggleGroup.Root
              type="single"
              value={scope}
              onValueChange={(v) => {
                if (v) setScope(v);
              }}
              disabled={!!busy}
              className="codeup-tabs"
              aria-label="Git 执行位置"
            >
              <ToggleGroup.Item value="local">本机 Git</ToggleGroup.Item>
              <ToggleGroup.Item value="remote">服务器 Git</ToggleGroup.Item>
            </ToggleGroup.Root>
            {offline ? (
              <div className="muted">请先连接服务器</div>
            ) : (
              <>
                {remote && (
                  <div className="codeup-account">
                    <Server size={14} />
                    {account}
                  </div>
                )}
                <div className="codeup-status">
                  <div className="codeup-status-copy">
                    <span className="muted">
                      {remote ? "服务器认证" : "认证来源"}
                    </span>
                    <strong>{authLabel(scope, status, vps)}</strong>
                  </div>
                  <div className="codeup-actions">
                    <Button
                      variant="ghost"
                      size="icon-sm"
                      aria-label="重新检查配置"
                      disabled={disabled}
                      onClick={refresh}
                    >
                      <RefreshCw
                        size={15}
                        className={busy.endsWith("status") ? "spin" : ""}
                      />
                    </Button>
                    {!remote && !legacy && !showForm && (
                      <Button
                        variant="ghost"
                        size="sm"
                        disabled={disabled}
                        onClick={() => edit()}
                      >
                        更换认证
                      </Button>
                    )}
                    {!remote && legacy && (
                      <Button
                        variant="ghost"
                        size="sm"
                        disabled={disabled}
                        onClick={() =>
                          confirm("git_auth_migrate", { scope: "local" })
                        }
                      >
                        迁移认证
                      </Button>
                    )}
                    {(remote || status.configured) && appMenu}
                  </div>
                </div>
                {(status.error || vps.error) && (
                  <InlineAlert>{status.error || vps.error}</InlineAlert>
                )}
                {remote && status.configured && vps.configured && (
                  <div className="codeup-legacy muted">
                    App 保存的服务器认证 · {status.username} ·
                    当前使用服务器已同步的认证
                  </div>
                )}
                {showForm && (
                  <div className="codeup-form">
                    {editor === "remote-app" && (
                      <div className="codeup-form-target muted">
                        保存位置：App 保存的服务器认证 · {account}
                      </div>
                    )}
                    <div>
                      <Label htmlFor="codeup-user">用户名</Label>
                      <Input
                        id="codeup-user"
                        autoComplete="off"
                        disabled={disabled}
                        value={username}
                        onChange={(e) => setUsername(e.target.value)}
                        placeholder="Codeup 克隆用户名"
                      />
                    </div>
                    <div>
                      <Label htmlFor="codeup-secret">密码或令牌</Label>
                      <Input
                        id="codeup-secret"
                        type="password"
                        autoComplete="new-password"
                        disabled={disabled}
                        value={secret}
                        onChange={(e) => setSecret(e.target.value)}
                      />
                    </div>
                    {editor && (
                      <Button
                        className="codeup-cancel-edit"
                        variant="ghost"
                        size="sm"
                        disabled={!!busy}
                        onClick={() => {
                          setEditor(null);
                          setSecret("");
                        }}
                      >
                        取消更换
                      </Button>
                    )}
                  </div>
                )}
                <div className="codeup-access">
                  <Label htmlFor="codeup-url">
                    仓库访问{draft ? " · 本次输入" : ""}
                  </Label>
                  <div className="codeup-access-input">
                    <Input
                      id="codeup-url"
                      disabled={disabled}
                      value={url}
                      onChange={(e) => setUrl(e.target.value)}
                      placeholder="粘贴仓库地址"
                    />
                    <Button
                      variant="outline"
                      disabled={
                        disabled ||
                        !url.trim() ||
                        !!vps.loading ||
                        (showForm && (!username.trim() || !secret))
                      }
                      onClick={() => void run(testOp)}
                    >
                      {access.testing && (
                        <RefreshCw size={14} className="spin" />
                      )}
                      检测
                    </Button>
                  </div>
                  <div
                    className={`codeup-access-result ${access.error ? "codeup-failed" : access.ok ? "codeup-passed" : "muted"}`}
                    role="status"
                  >
                    {access.error ? (
                      <>
                        <AlertCircle size={14} />
                        <span>{access.error}</span>
                        {credentialRecovery(access.error) && (
                          <Button
                            variant="ghost"
                            size="sm"
                            disabled={disabled}
                            onClick={() => edit()}
                          >
                            更换认证
                          </Button>
                        )}
                      </>
                    ) : access.ok ? (
                      <>
                        <Check size={14} />
                        通过 · {access.time}
                      </>
                    ) : access.testing ? (
                      "检测中…"
                    ) : (
                      "未检测"
                    )}
                  </div>
                </div>
                {error && !confirmation && <InlineAlert>{error}</InlineAlert>}
              </>
            )}
          </div>
          <DialogFooter>
            <Button variant="outline" onClick={() => onOpenChange(false)}>
              {busy ? "在后台继续" : "关闭"}
            </Button>
            {!offline && showForm ? (
              <Button
                disabled={
                  disabled || !username.trim() || !secret || !!vps.loading
                }
                onClick={() =>
                  remote && editor !== "remote-app"
                    ? sync("input")
                    : confirm("git_auth_save")
                }
              >
                {busy === "git_auth_save" || busy === "git_vps_auth_install" ? (
                  <RefreshCw size={14} className="spin" />
                ) : null}
                {remote && editor !== "remote-app"
                  ? "同步到服务器…"
                  : onSaved
                    ? "保存并重试克隆"
                    : "保存到 App"}
              </Button>
            ) : remote && !offline && !vps.configured ? (
              <Button
                disabled={disabled || !!vps.loading || !!vps.error}
                onClick={() => sync()}
              >
                {busy === "git_vps_auth_install" && (
                  <RefreshCw size={14} className="spin" />
                )}
                同步本机认证…
              </Button>
            ) : null}
          </DialogFooter>
        </DialogContent>
      </Dialog>
      <Confirm
        open={!!confirmation}
        title={isVps ? "同步到服务器" : isRemove ? "移除认证" : "本机保存"}
        action={isVps ? "同步到服务器" : isRemove ? "移除" : "保存"}
        destructive={!!isRemove}
        busy={!!busy}
        onCancel={() => {
          setConfirmation(null);
          setError("");
        }}
        onConfirm={() => {
          const c = confirmation;
          if (c)
            void run(c.op, {
              ...c.request,
              storage_ack: true,
              server_storage_ack: !!isVps,
              confirm: !!isRemove,
            });
        }}
      >
        {isVps ? (
          <>
            <strong>{confirmation?.request.vps_account}</strong>
            <p>来源：{sourceLabel(confirmation?.source)}</p>
            <p>
              认证将明文保存在此账号的私有文件中，同账号程序和服务器管理员可读取。
            </p>
            <code>~/.local/share/ccbridge/codeup-auth</code>
          </>
        ) : isRemove ? (
          <p>
            移除
            {confirmation?.op === "git_vps_auth_remove"
              ? ` ${account} 上由 CCBridge 保存的服务器认证`
              : confirmation?.request.scope === "remote"
                ? ` App 为 ${account} 保存的服务器认证`
                : " App 本机认证"}
            ？
          </p>
        ) : (
          <p>
            认证将明文保存在本机当前用户目录，同一用户运行的程序可读取。此提示只确认一次。
          </p>
        )}
        {error && <InlineAlert>{error}</InlineAlert>}
      </Confirm>
    </>
  );
}
