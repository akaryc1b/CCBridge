import { useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { RefreshCw } from "lucide-react";
import { Button } from "./ui/button";
import { Confirm } from "./common/Confirm";
import { InlineAlert, OperationBar } from "./common/Feedback";
import { useOperation } from "../lib/useOperation";
import { errorText } from "./CodeupAuth";
type Data = Record<string, any>;
export function ClaudeUpdate({
  connection,
  onRefresh,
}: {
  connection: Data;
  onRefresh: () => Promise<void>;
}) {
  const id = connection.connection_id || "";
  const currentId = useRef(id);
  currentId.current = id;
  const [info, setInfo] = useState<Data | null>(null),
    [checking, setChecking] = useState(false),
    [updating, setUpdating] = useState(false),
    [confirm, setConfirm] = useState(false),
    [error, setError] = useState(""),
    [message, setMessage] = useState(""),
    [operationId, setOperationId] = useState<string>();
  const supported =
    connection.connected &&
    connection.probe?.capabilities?.includes("claude_update_v1");
  const operation = useOperation(
    "claude_update",
    "",
    id,
    !!supported,
    operationId,
  );
  const busy = updating || operation.snap?.state === "running";
  useEffect(() => {
    setInfo(null);
    setError("");
    setMessage("");
    setChecking(false);
    setUpdating(false);
    setConfirm(false);
    setOperationId(undefined);
  }, [id]);
  async function check() {
    const account = id;
    setChecking(true);
    setError("");
    setMessage("");
    try {
      const r = await invoke<Data>("claude_update_check", {
        request: { connection_id: account },
      });
      if (currentId.current === account) {
        setInfo(r);
        setError(r.check_error || "");
      }
    } catch (e) {
      if (currentId.current === account) setError(errorText(e));
    } finally {
      if (currentId.current === account) setChecking(false);
    }
  }
  async function update() {
    const account = id;
    setConfirm(false);
    setUpdating(true);
    setError("");
    setMessage("");
    const operation_id = crypto.randomUUID();
    setOperationId(operation_id);
    try {
      const r = await invoke<Data>("claude_update", {
        request: {
          connection_id: account,
          confirmed: true,
          expected_version: info?.current_version,
          operation_id,
        },
      });
      if (currentId.current === account) {
        setInfo((old) => ({
          ...old,
          current_version: r.current_version,
          update_available: false,
        }));
        setMessage(r.message);
        await onRefresh();
      }
    } catch (e) {
      if (currentId.current === account) {
        setError(errorText(e));
        setInfo(null);
      }
    } finally {
      if (currentId.current === account) setUpdating(false);
    }
  }
  const version =
    info?.current_version ||
    connection.probe?.claude_version?.replace(" (Claude Code)", "") ||
    "未检测到";
  return (
    <>
      <div className="environment-row compact-environment">
        <span title={connection.resolved_paths?.claude || ""}>Claude Code</span>
        <span className="grow" />
        <span className="muted">{version}</span>
        {info?.update_available && !info.check_error && (
          <span className="muted">可更新 {info.available_version}</span>
        )}
        <Button
          variant="ghost"
          size="sm"
          aria-label="检查 Claude 更新"
          title={
            supported
              ? "检查版本，不执行更新"
              : "更新服务端程序后可检查 Claude 更新"
          }
          disabled={!supported || checking || busy}
          onClick={() => void check()}
        >
          <RefreshCw size={14} className={checking ? "spin" : ""} />
          {checking ? "检查中…" : "检查更新"}
        </Button>
        {info?.can_update && info.update_available && !info.check_error && (
          <Button
            size="sm"
            variant="outline"
            aria-label="更新 Claude Code"
            disabled={busy || checking}
            onClick={() => setConfirm(true)}
          >
            {busy
              ? operation.snap?.phase === "等待活动任务结束"
                ? "等待中…"
                : "更新中…"
              : "更新"}
          </Button>
        )}
      </div>
      {info && !info.can_update && (
        <p className="hint">
          {info.install_kind === "shared"
            ? "共享安装，由管理员更新"
            : info.install_kind === "homebrew"
              ? "Homebrew 安装，请由原包管理器更新"
              : "此安装需由原安装方式维护"}
        </p>
      )}
      {message && (
        <p role="status" className="hint">
          {message}
        </p>
      )}
      {info && !info.update_available && !info.check_error && !message && (
        <p className="hint">
          当前版本不低于{info.channel === "stable" ? "稳定" : "最新"}通道版本{" "}
          {info.available_version}
        </p>
      )}
      {busy && operation.snap && <OperationBar operation={operation.snap} />}
      {error && <InlineAlert>{error}</InlineAlert>}
      <Confirm
        open={confirm}
        title="更新 Claude Code？"
        action="确定更新"
        onCancel={() => setConfirm(false)}
        onConfirm={() => void update()}
      >
        当前账号的 Claude Code {version}{" "}
        将通过其原有更新通道升级。活动任务结束后执行；断开连接取消等待，不使用
        sudo。
      </Confirm>
    </>
  );
}
