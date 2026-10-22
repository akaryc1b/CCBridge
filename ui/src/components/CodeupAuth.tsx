import { useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { KeyRound, MoreHorizontal, Loader2 } from "lucide-react";
import { DropdownMenu as Menu } from "radix-ui";
import { Button } from "./ui/button";
import { Input } from "./ui/input";
import { Label } from "./ui/label";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "./ui/select";
import {
  Dialog,
  DialogContent,
  DialogHeader,
  DialogTitle,
  DialogDescription,
  DialogFooter,
} from "./ui/dialog";
import { StatusRow, InlineAlert } from "./common/Feedback";
import { Confirm } from "./common/Confirm";
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
}) {
  const [scope, setScope] = useState(initialScope),
    [url, setUrl] = useState(initialUrl),
    [username, setUsername] = useState(""),
    [secret, setSecret] = useState(""),
    [status, setStatus] = useState<Data>({ loading: true }),
    [vps, setVps] = useState<Data>({}),
    [access, setAccess] = useState<Data>({}),
    [vpsAccess, setVpsAccess] = useState<Data>({}),
    [busy, setBusy] = useState(""),
    [error, setError] = useState(""),
    [confirmation, setConfirmation] = useState<Data | null>(null),
    [syncSource, setSyncSource] = useState("git_helper");
  const generation = useRef(0);
  const runLock = useRef(false);
  useEffect(() => {
    if (open) {
      setUrl(initialUrl);
      setScope(initialScope);
      setSecret("");
      setError("");
      setConfirmation(null);
    }
  }, [open, initialUrl, initialScope]);
  useEffect(() => {
    const seq = ++generation.current;
    setStatus({ loading: true });
    setUsername("");
    setSecret("");
    setAccess({});
    setVpsAccess({});
    setError("");
    setConfirmation(null);
    if (!open) return;
    if (scope === "remote" && !connected) {
      setStatus({});
      return;
    }
    invoke<Data>("git_auth_status", { request: { scope } })
      .then((r) => {
        if (seq === generation.current) {
          setStatus(r);
          setUsername(r.username || "");
        }
      })
      .catch((e) => {
        if (seq === generation.current) setStatus({ error: errorText(e) });
      });
    if (connected) {
      setVps({ loading: true });
      invoke<Data>("git_vps_auth_status", { request: {} })
        .then((r) => {
          if (seq === generation.current) setVps(r);
        })
        .catch((e) => {
          if (seq === generation.current) setVps({ error: errorText(e) });
        });
    } else setVps({});
    return () => {
      generation.current++;
    };
  }, [open, scope, account, connected]);
  useEffect(() => {
    setAccess({});
    setVpsAccess({});
    setConfirmation(null);
  }, [url, username, secret, syncSource]);
  const request = () => ({
    scope,
    url,
    username,
    secret,
    storage: "file",
    vps_account: account,
    credential_source: secret ? "input" : syncSource,
  });
  async function run(op: string, req: Data = request()) {
    if (runLock.current) return;
    runLock.current = true;
    const seq = generation.current;
    setBusy(op);
    setError("");
    if (op === "git_access") setAccess({ testing: true });
    if (op === "git_vps_auth_test") setVpsAccess({ testing: true });
    try {
      const r = await invoke<Data>(op, { request: req });
      if (seq !== generation.current) return;
      const checked = {
        ok: true,
        time: new Date().toLocaleTimeString("zh-CN", {
          hour: "2-digit",
          minute: "2-digit",
        }),
      };
      if (op === "git_access") setAccess(checked);
      else if (op === "git_auth_status")
        setStatus({ ...r, manually_checked: true });
      else if (op === "git_vps_auth_test") setVpsAccess(checked);
      else if (op === "git_vps_auth_status")
        setVps({ ...r, manually_checked: true });
      else if (op.startsWith("git_vps_auth_")) {
        setVps(r);
        setVpsAccess({});
        setSecret("");
      } else {
        setStatus(r);
        if (op === "git_auth_remove") setUsername("");
        setSecret("");
        setAccess({});
        if (["git_auth_save", "git_auth_migrate"].includes(op)) {
          onConsent?.();
          if (onSaved) {
            onOpenChange(false);
            onSaved();
          }
        }
      }
      setConfirmation(null);
    } catch (e) {
      if (seq !== generation.current) return;
      const text = errorText(e);
      if (op === "git_access") setAccess({ error: text });
      else if (op === "git_vps_auth_test") setVpsAccess({ error: text });
      else setError(text);
    } finally {
      runLock.current = false;
      if (seq === generation.current) setBusy("");
    }
  }
  function confirm(op: string) {
    setError("");
    if ((op === "git_auth_save" || op === "git_auth_migrate") && consented) {
      void run(op);
      return;
    }
    setConfirmation({ op, request: request() });
  }
  const disabled =
    !!busy || status.loading === true || (scope === "remote" && !connected);
  const testText = (a: Data) =>
    a.testing
      ? "检测中…"
      : a.error
        ? "访问失败"
        : a.ok
          ? `通过 · ${a.time}`
          : "未检测";
  const effective =
    scope === "remote" && vps.configured
      ? "VPS 凭据"
      : status.configured
        ? status.storage === "file"
          ? "App 已保存"
          : "App 旧版存档 · 需迁移"
        : scope === "local"
          ? "本机 Git 凭据助手"
          : "服务器 Git 凭据助手";
  const isVps = confirmation?.op === "git_vps_auth_install";
  const isRemove = confirmation?.op?.includes("remove");
  return (
    <>
      <Dialog
        open={open}
        onOpenChange={(v) => {
          if (!busy) onOpenChange(v);
        }}
      >
        <DialogContent className="codeup-dialog">
          <DialogHeader>
            <DialogTitle>Codeup 认证</DialogTitle>
            <DialogDescription className="sr-only">
              本机与服务器 Codeup 凭据
            </DialogDescription>
          </DialogHeader>
          <div className="dialog-body">
            <div className="segmented">
              {[
                ["local", "本机 Git"],
                ["remote", "服务器 Git"],
              ].map(([v, l]) => (
                <button
                  disabled={!!busy}
                  className={scope === v ? "chosen" : ""}
                  key={v}
                  onClick={() => setScope(v)}
                >
                  {l}
                </button>
              ))}
            </div>
            <div className="field">
              <Label htmlFor="codeup-url">仓库地址（访问检测时填写）</Label>
              <Input
                id="codeup-url"
                value={url}
                disabled={disabled}
                onChange={(e) => setUrl(e.target.value)}
                placeholder="https://codeup.aliyun.com/组织/仓库.git"
              />
            </div>
            <StatusRow
              label="认证配置"
              action={
                <Button
                  variant="outline"
                  size="sm"
                  disabled={disabled}
                  onClick={() => void run("git_auth_status", { scope })}
                >
                  {busy === "git_auth_status"
                    ? "检查中…"
                    : status.manually_checked
                      ? "已检查 · 再检查"
                      : "检查配置"}
                </Button>
              }
            >
              {status.loading ? (
                "读取中…"
              ) : status.error ? (
                <span className="text-destructive">{status.error}</span>
              ) : status.configured ||
                (scope === "remote" && vps.configured) ? (
                effective
              ) : status.helper_configured ? (
                "已配置本机 Git 助手"
              ) : scope === "local" ? (
                "未找到 App 凭据或 Git 助手"
              ) : (
                "App 未保存"
              )}
            </StatusRow>
            {!(scope === "remote" && vps.configured) && (
              <StatusRow
                label={scope === "local" ? "本机仓库访问" : "App 仓库访问"}
                action={
                  <Button
                    variant="outline"
                    size="sm"
                    disabled={disabled || !url.trim()}
                    onClick={() => void run("git_access")}
                  >
                    检测
                  </Button>
                }
              >
                <span className={access.error ? "text-destructive" : "muted"}>
                  {testText(access)}
                </span>
              </StatusRow>
            )}
            {access.error && <InlineAlert>{access.error}</InlineAlert>}
            <div className="form-two">
              <div className="field">
                <Label htmlFor="codeup-user">用户名</Label>
                <Input
                  id="codeup-user"
                  value={username}
                  disabled={disabled}
                  autoComplete="off"
                  onChange={(e) => setUsername(e.target.value)}
                  placeholder="留空复用现有认证"
                />
              </div>
              <div className="field">
                <Label htmlFor="codeup-secret">密码或令牌</Label>
                <Input
                  id="codeup-secret"
                  type="password"
                  value={secret}
                  disabled={disabled}
                  autoComplete="new-password"
                  onChange={(e) => setSecret(e.target.value)}
                  placeholder={
                    status.configured ? "已保存，留空不变" : "未填写"
                  }
                />
              </div>
            </div>
            <StatusRow
              label="App 凭据"
              action={
                <Menu.Root>
                  <Menu.Trigger asChild>
                    <Button
                      aria-label="App凭据操作"
                      variant="ghost"
                      size="icon"
                      disabled={disabled || !status.configured}
                    >
                      <MoreHorizontal size={16} />
                    </Button>
                  </Menu.Trigger>
                  <Menu.Portal>
                    <Menu.Content className="context-menu">
                      {status.storage !== "file" && (
                        <Menu.Item onSelect={() => confirm("git_auth_migrate")}>
                          迁移到本机文件
                        </Menu.Item>
                      )}
                      <Menu.Item onSelect={() => confirm("git_auth_remove")}>
                        移除 App 凭据
                      </Menu.Item>
                    </Menu.Content>
                  </Menu.Portal>
                </Menu.Root>
              }
            >
              {status.configured
                ? `已保存 · ${status.username || ""}`
                : "未保存"}
            </StatusRow>
            <div className="section-title auth-vps-title">
              <h3>服务器认证</h3>
              <span className="muted">{account || "未连接"}</span>
            </div>
            <StatusRow
              label="服务器配置"
              action={
                <Button
                  variant="ghost"
                  size="sm"
                  disabled={
                    !!busy || !connected || !vps.configured || vps.loading
                  }
                  onClick={() => confirm("git_vps_auth_remove")}
                >
                  移除…
                </Button>
              }
            >
              {!connected ? (
                "未连接"
              ) : vps.loading ? (
                "读取中…"
              ) : vps.error ? (
                <span className="text-destructive">读取失败</span>
              ) : vps.configured ? (
                `已同步${vps.username ? ` · ${vps.username}` : ""}`
              ) : vps.files_present && !vps.entry_present ? (
                "凭据已失效 · 请重新同步"
              ) : vps.entry_present && !vps.included ? (
                "Git 配置未接入 · 请重新同步"
              ) : (
                "未同步"
              )}
            </StatusRow>
            {vps.error && <InlineAlert>{vps.error}</InlineAlert>}
            <Button
              variant="ghost"
              size="sm"
              disabled={!!busy || !connected || !!vps.loading}
              onClick={() => void run("git_vps_auth_status", {})}
            >
              {vps.manually_checked
                ? "已检查 · 再检查服务器配置"
                : "检查服务器配置"}
            </Button>
            <StatusRow
              label="服务器仓库访问"
              action={
                <Button
                  variant="outline"
                  size="sm"
                  disabled={
                    !!busy || !connected || !!vps.loading || !url.trim()
                  }
                  onClick={() => void run("git_vps_auth_test")}
                >
                  检测
                </Button>
              }
            >
              <span className={vpsAccess.error ? "text-destructive" : "muted"}>
                {testText(vpsAccess)}
              </span>
            </StatusRow>
            {vpsAccess.error && <InlineAlert>{vpsAccess.error}</InlineAlert>}
            <div className="field">
              <Label>同步来源</Label>
              <Select
                value={secret ? "input" : syncSource}
                disabled={!!busy || !!secret}
                onValueChange={setSyncSource}
              >
                <SelectTrigger aria-label="同步来源">
                  <SelectValue />
                </SelectTrigger>
                <SelectContent>
                  <SelectItem value="git_helper">本机 Git 凭据助手</SelectItem>
                  <SelectItem
                    value="app"
                    disabled={!status.configured || status.storage !== "file"}
                  >
                    当前 App 保存的认证
                    {status.configured && status.storage !== "file"
                      ? "（需迁移）"
                      : ""}
                  </SelectItem>
                  {secret && (
                    <SelectItem value="input">
                      上方填写的用户名和令牌
                    </SelectItem>
                  )}
                </SelectContent>
              </Select>
            </div>
            <Button
              variant="ghost"
              className="auth-write"
              disabled={
                disabled ||
                !connected ||
                !!vps.loading ||
                (syncSource === "app" && !status.configured && !secret)
              }
              onClick={() => confirm("git_vps_auth_install")}
            >
              <KeyRound size={14} />
              {vps.configured ? "重新同步到服务器…" : "同步到服务器…"}
            </Button>
            {busy && (
              <p role="status" className="muted flex items-center gap-2">
                <Loader2 size={14} className="spin" />
                {busy.includes("test") || busy === "git_access"
                  ? "检测中…"
                  : busy.endsWith("status")
                    ? "检查配置中…"
                    : "保存中…"}
              </p>
            )}
            {error && <InlineAlert>{error}</InlineAlert>}
          </div>
          <DialogFooter>
            <Button
              variant="outline"
              disabled={!!busy}
              onClick={() => onOpenChange(false)}
            >
              关闭
            </Button>
            <Button
              disabled={disabled || !username.trim() || !secret}
              onClick={() => confirm("git_auth_save")}
            >
              {onSaved ? "保存并重试克隆" : "保存到 App"}
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>
      <Confirm
        open={!!confirmation}
        title={isVps ? "写入 VPS 凭据" : isRemove ? "移除凭据" : "本机保存"}
        action={isVps ? "写入 VPS" : isRemove ? "移除" : "保存"}
        destructive={!!isRemove}
        busy={!!busy}
        onCancel={() => setConfirmation(null)}
        onConfirm={() => {
          const c = confirmation;
          if (c)
            void run(c.op, {
              ...c.request,
              storage_ack: true,
              server_storage_ack: isVps,
              confirm: isRemove,
            });
        }}
      >
        {isVps ? (
          <>
            <strong>{confirmation?.request.vps_account}</strong>
            <p>
              凭据将明文保存在此 VPS
              账号的私有文件中，同账号程序和服务器管理员可读取。
            </p>
            <code>~/.local/share/ccbridge/codeup-auth</code>
            <p className="muted">
              来源：
              {confirmation?.request.credential_source === "input"
                ? "上方填写的用户名和令牌"
                : confirmation?.request.credential_source === "app"
                  ? "当前 App 保存的认证"
                  : "本机 Git 凭据助手"}
            </p>
          </>
        ) : isRemove ? (
          <p>
            移除
            {confirmation?.op === "git_vps_auth_remove"
              ? ` ${account} 上由 CCBridge 写入的 VPS 凭据`
              : confirmation?.request.scope === "remote"
                ? ` App 为 ${account} 保存的服务器档凭据`
                : " App 本机档凭据"}
            ？
          </p>
        ) : (
          <p>
            凭据将明文保存在本机当前用户目录，同一用户运行的程序可读取。此提示只确认一次。
          </p>
        )}
      </Confirm>
    </>
  );
}
