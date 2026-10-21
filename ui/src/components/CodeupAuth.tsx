import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { Button } from "./ui/button";
import { Input } from "./ui/input";
import { Label } from "./ui/label";
import { Badge } from "./ui/badge";
import {
  Dialog,
  DialogContent,
  DialogHeader,
  DialogTitle,
  DialogDescription,
  DialogFooter,
} from "./ui/dialog";
import {
  Select,
  SelectTrigger,
  SelectValue,
  SelectContent,
  SelectItem,
} from "./ui/select";
import { Loader2, ShieldCheck } from "lucide-react";
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
  const [storage, setStorage] = useState("file"),
    [storageAck, setStorageAck] = useState(false);
  const [scope, setScope] = useState(initialScope),
    [url, setUrl] = useState(initialUrl),
    [username, setUsername] = useState(""),
    [secret, setSecret] = useState(""),
    [status, setStatus] = useState<Data>({}),
    [busy, setBusy] = useState(""),
    [message, setMessage] = useState(""),
    [error, setError] = useState(""),
    [forget, setForget] = useState(false);
  const [vpsStatus, setVpsStatus] = useState<Data>({}),
    [vpsAck, setVpsAck] = useState(false),
    [vpsRemove, setVpsRemove] = useState(false);
  useEffect(() => {
    if (!open) return;
    let active = true;
    setVpsAck(false);
    setVpsRemove(false);
    setVpsStatus(connected ? { loading: true } : {});
    if (connected)
      invoke<Data>("git_vps_auth_status", { request: {} })
        .then((r) => {
          if (active) setVpsStatus(r);
        })
        .catch((e) => {
          if (active) setVpsStatus({ error: true, message: errorText(e) });
        });
    return () => {
      active = false;
    };
  }, [open, account, connected]);
  useEffect(() => {
    if (open) {
      setUrl(initialUrl);
      setScope(initialScope);
      setSecret("");
      setMessage("");
      setError("");
    }
  }, [open, initialUrl, initialScope]);
  useEffect(() => {
    if (!open) return;
    let active = true;
    setStatus({});
    setUsername("");
    setSecret("");
    setMessage("");
    setError("");
    if (scope === "remote" && !connected) return;
    invoke<Data>("git_auth_status", { request: { scope } })
      .then((r) => {
        if (active) {
          setStatus(r);
          setStorage("file");
          setStorageAck(false);
          setUsername(r.username || "");
        }
      })
      .catch((e) => {
        if (active) setError(errorText(e));
      });
    return () => {
      active = false;
    };
  }, [open, scope, account, connected]);
  async function run(op: string) {
    if (busy) return;
    setBusy(op);
    setError("");
    setMessage(
      op === "git_access"
        ? "正在用 Git 检测仓库访问…"
        : op === "git_vps_auth_test"
          ? "正在检测 VPS 自身认证…"
          : op === "git_vps_auth_install"
            ? "正在同步认证到当前 VPS…"
            : op === "git_vps_auth_remove"
              ? "正在移除 VPS 认证…"
              : "正在保存认证…",
    );
    try {
      const r = await invoke<Data>(op, {
        request: {
          scope,
          url,
          username,
          secret,
          storage,
          storage_ack: storageAck,
          server_storage_ack: vpsAck,
          vps_account: account,
          confirm: vpsRemove,
        },
      });
      if (
        ["git_auth_save", "git_auth_migrate"].includes(op) &&
        storage === "file"
      )
        onConsent?.();
      if (op.startsWith("git_vps_auth_")) {
        if (op !== "git_vps_auth_test") setVpsStatus(r);
        if (op === "git_vps_auth_install") setSecret("");
        setVpsAck(false);
        setVpsRemove(false);
        setMessage(r.message || "VPS 认证操作完成");
      } else if (op === "git_access") {
        setMessage(r.message || "仓库可以读取");
      } else {
        setStatus(r);
        setSecret("");
        setMessage(
          op === "git_auth_remove"
            ? "已忘记 App 保存的认证；系统原有 Git 凭据助手不受影响"
            : "认证已保存，可检测访问或直接克隆",
        );
      }
      setForget(false);
      if (["git_auth_save", "git_auth_migrate"].includes(op) && onSaved) {
        onOpenChange(false);
        onSaved();
      }
    } catch (e) {
      setMessage("");
      setError(errorText(e));
    } finally {
      setBusy("");
    }
  }
  const disabled = !!busy || (scope === "remote" && !connected);
  return (
    <Dialog
      open={open}
      onOpenChange={(v) => {
        if (!busy) onOpenChange(v);
      }}
    >
      <DialogContent className="max-h-[85vh] max-w-xl overflow-y-auto">
        <DialogHeader>
          <DialogTitle>Codeup 认证与访问检测</DialogTitle>
          <DialogDescription>
            HTTPS 用户名与密码 / 访问令牌。无需到终端配置，不使用 SSH 登录密码。
          </DialogDescription>
        </DialogHeader>
        <Label>在哪里执行 Git</Label>
        <Select value={scope} onValueChange={setScope} disabled={!!busy}>
          <SelectTrigger className="w-full">
            <SelectValue />
          </SelectTrigger>
          <SelectContent>
            <SelectItem value="local">本机获取，再通过 SSH 传仓库</SelectItem>
            <SelectItem value="remote">服务器当前 SSH 账号直接获取</SelectItem>
          </SelectContent>
        </Select>
        <p className="hint">
          {scope === "remote"
            ? `当前账号：${account || "尚未连接"}。App 档认证保存在 Mac，仅供 App 内 Git 操作；VPS 终端 / Claude 的认证请在下方单独同步。`
            : "认证保存在 Mac，仅用于 codeup.aliyun.com；未保存时使用本机 Git 凭据助手。可在下方单独授权同步到 VPS。"}
        </p>
        <Badge variant="outline">
          {status.configured
            ? `App 已保存：${status.username}`
            : "App 尚未保存认证 · 可先检测现有 Git 凭据"}
        </Badge>
        <Label htmlFor="codeup-url">Codeup 仓库 HTTPS 地址</Label>
        <Input
          id="codeup-url"
          value={url}
          onChange={(e) => setUrl(e.target.value)}
          disabled={disabled}
          placeholder="https://codeup.aliyun.com/组织/仓库.git"
        />
        <Label htmlFor="codeup-user">Codeup HTTPS 克隆用户名</Label>
        <Input
          id="codeup-user"
          value={username}
          onChange={(e) => setUsername(e.target.value)}
          disabled={disabled}
          autoComplete="off"
        />
        <Label htmlFor="codeup-secret">HTTPS 密码 / 访问令牌</Label>
        <Input
          id="codeup-secret"
          type="password"
          value={secret}
          onChange={(e) => setSecret(e.target.value)}
          disabled={disabled}
          autoComplete="new-password"
          placeholder={
            status.configured
              ? "留空使用已保存认证；填写则更新"
              : "请输入 Codeup 的 HTTPS 密码或访问令牌"
          }
        />
        <p className="hint">
          检测只读取远端引用，不克隆、不推送。填写但未保存的凭据也可直接检测；不会显示或回填已保存的令牌。
        </p>
        {status.configured && status.storage !== "file" && (
          <Button
            disabled={disabled || (!consented && !storageAck)}
            onClick={() => void run("git_auth_migrate")}
          >
            迁移此作用域已保存的认证
          </Button>
        )}
        {storage === "file" && !consented && (
          <label className="hint">
            <input
              type="checkbox"
              checked={storageAck}
              onChange={(e) => setStorageAck(e.target.checked)}
            />{" "}
            我理解密码以明文保存在当前用户私有目录，同一用户程序仍可读取。
          </label>
        )}
        <section className="rounded border p-3 space-y-3">
          <div className="flex items-center justify-between">
            <Label>VPS 持久认证</Label>
            <Badge variant="outline">
              {!connected
                ? "未连接"
                : vpsStatus.loading
                  ? "检测中…"
                  : vpsStatus.error
                    ? "检测失败"
                    : vpsStatus.configured
                      ? "VPS 已配置"
                      : "VPS 未配置"}
            </Badge>
          </div>
          <p className="hint">
            {account || "请先连接 SSH"} ·{" "}
            {vpsStatus.message || "可在克隆之后随时补同步，不需要重新克隆。"}
          </p>
          <p className="hint">
            同步使用上方输入的令牌；留空则使用所选档已保存认证，本机档还可复用
            Git 凭据助手。只接入当前 VPS 账号的 Codeup HTTPS，不修改 SSH 登录。
          </p>
          <label className="hint flex gap-2">
            <input
              type="checkbox"
              checked={vpsAck}
              disabled={!!busy || !connected}
              onChange={(e) => setVpsAck(e.target.checked)}
            />
            我同意将该认证以明文保存到此 VPS 账号的私有文件，供普通 Git / Claude
            使用；同账号程序及服务器管理员可读取。推荐专用最小权限令牌。
          </label>
          <div className="flex flex-wrap gap-2">
            <Button
              variant="outline"
              disabled={!!busy || !connected || !vpsAck || !url.trim()}
              onClick={() => void run("git_vps_auth_install")}
            >
              同步到当前 VPS
            </Button>
            <Button
              variant="outline"
              disabled={!!busy || !connected || !url.trim()}
              onClick={() => void run("git_vps_auth_test")}
            >
              检测 VPS 自身认证
            </Button>
            <Button
              variant="ghost"
              disabled={!!busy || !connected || !vpsStatus.configured}
              onClick={() => setVpsRemove(true)}
            >
              移除 VPS 认证
            </Button>
          </div>
          {vpsRemove && (
            <div>
              <p>
                只移除此 App 写入的 VPS Codeup 认证，Mac 认证及服务器其他 Git
                配置保留。
              </p>
              <Button
                variant="destructive"
                disabled={!!busy}
                onClick={() => void run("git_vps_auth_remove")}
              >
                确认移除 VPS 认证
              </Button>
              <Button variant="ghost" onClick={() => setVpsRemove(false)}>
                取消
              </Button>
            </div>
          )}
        </section>
        {message && (
          <p role="status" className="flex items-center gap-2">
            {busy ? (
              <Loader2 size={16} className="animate-spin" />
            ) : (
              <ShieldCheck size={16} />
            )}{" "}
            {message}
          </p>
        )}
        {error && (
          <p role="alert" className="text-destructive">
            {error}
          </p>
        )}
        {forget && (
          <div className="rounded border p-3">
            <p>
              忘记 App 为{scope === "local" ? "本机" : account}保存的 Codeup
              认证？之后仍可重新输入保存。
            </p>
            <Button
              variant="destructive"
              disabled={disabled}
              onClick={() => void run("git_auth_remove")}
            >
              确认忘记认证
            </Button>
            <Button variant="ghost" onClick={() => setForget(false)}>
              取消
            </Button>
          </div>
        )}
        <DialogFooter>
          <Button
            variant="ghost"
            disabled={disabled || !status.configured}
            onClick={() => setForget(true)}
          >
            忘记认证
          </Button>
          <Button
            variant="outline"
            disabled={disabled || !url.trim()}
            onClick={() => void run("git_access")}
          >
            检测仓库访问
          </Button>
          <Button
            disabled={
              disabled ||
              !username.trim() ||
              !secret ||
              (storage === "file" && !consented && !storageAck)
            }
            onClick={() => void run("git_auth_save")}
          >
            {onSaved ? "保存并重试克隆" : "保存认证"}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
