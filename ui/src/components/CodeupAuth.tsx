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
}: {
  open: boolean;
  onOpenChange: (v: boolean) => void;
  connected: boolean;
  account: string;
  url: string;
  initialScope?: string;
  onSaved?: () => void;
}) {
  const [storage, setStorage] = useState("keychain"),
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
          setStorage(r.storage === "file" ? "file" : "keychain");
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
      op === "git_access" ? "正在用 Git 检测仓库访问…" : "正在保存认证…",
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
        },
      });
      if (op === "git_access") {
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
      if (op === "git_auth_save" && onSaved) {
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
            ? `当前账号：${account || "尚未连接"}。认证按此 SSH 账号隔离保存于本机所选存储，仅在执行时经加密 SSH 提供；不写入远端 Git 配置。`
            : "认证保存于本机所选存储，仅用于 codeup.aliyun.com。未保存时继续使用原有 Git 凭据助手。"}
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
        <Label>密码保存方式</Label>
        <Select
          value={storage}
          onValueChange={(v) => {
            setStorage(v);
            setStorageAck(false);
          }}
          disabled={disabled}
        >
          <SelectTrigger>
            <SelectValue />
          </SelectTrigger>
          <SelectContent>
            <SelectItem value="keychain">系统钥匙串</SelectItem>
            <SelectItem value="file">本机私有文件</SelectItem>
          </SelectContent>
        </Select>
        {storage === "file" && (
          <label className="hint">
            <input
              type="checkbox"
              checked={storageAck}
              onChange={(e) => setStorageAck(e.target.checked)}
            />{" "}
            我理解密码以明文保存在当前用户私有目录，同一用户程序仍可读取；切换方式需要重新输入。
          </label>
        )}
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
              (storage === "file" && !storageAck)
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
