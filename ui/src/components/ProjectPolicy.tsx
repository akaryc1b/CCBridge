import { useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { Clock, ArrowRight, RefreshCw } from "lucide-react";
import { Switch } from "./ui/switch";
import { Button } from "./ui/button";
import { Confirm } from "./common/Confirm";
type Data = Record<string, any>;
export function ProjectPolicy({
  connection,
  refresh,
  onTasks,
}: {
  connection: Data;
  refresh: () => Promise<any>;
  onTasks: () => void;
}) {
  const [confirm, C] = useState(false),
    [busy, B] = useState(false),
    [error, E] = useState("");
  const p = connection.policy,
    capable = connection.probe?.capabilities?.includes("policy_v1"),
    available =
      connection.connected && capable && typeof p?.read_only === "boolean";
  async function save(read_only: boolean) {
    B(true);
    E("");
    try {
      await invoke("policy_set", {
        request: {
          read_only,
          confirmed: true,
          expected_revision: p.revision,
          connection_id: connection.connection_id,
        },
      });
      const c = await refresh();
      if (c?.policy?.read_only !== read_only)
        throw new Error("设置已提交，状态尚未确认，请重新检查");
      C(false);
    } catch (e: any) {
      E(e.message || String(e));
    } finally {
      B(false);
    }
  }
  return (
    <>
      <div className="switch-row policy-switch-row">
        <div>
          <strong>禁止 AI 修改项目文件</strong>
          <p className="muted">Claude 仅分析；仍可通过 Codex 更新仓库。</p>
        </div>
        <Switch
          aria-label="禁止 AI 修改项目文件"
          checked={p?.read_only === true}
          disabled={!available || busy}
          onCheckedChange={(v) => (v ? void save(true) : C(true))}
        />
      </div>
      {!connection.connected ? (
        <p className="muted policy-note">连接后可设置项目保护</p>
      ) : !capable ? (
        <p className="muted policy-note">需要更新服务端</p>
      ) : p?.error ? (
        <div className="policy-feedback inline-error">
          {p.error}
          <Button variant="ghost" size="sm" onClick={() => void refresh()}>
            <RefreshCw size={14} />
            重新检查
          </Button>
        </div>
      ) : !available ? (
        <p className="muted policy-note">读取保护设置中…</p>
      ) : (
        <p className="muted policy-note">
          {busy
            ? "保存中…"
            : "对新任务和下一轮生效，App 手动文件操作不受影响。"}
        </p>
      )}
      {p?.read_only && p.running_writes + (p.unknown_writes || 0) > 0 && (
        <div className="policy-existing">
          <Clock size={16} />
          <div>
            <strong>
              {p.unknown_writes
                ? `${p.running_writes + p.unknown_writes} 个先前任务未受限制`
                : `${p.running_writes} 个已有开发任务仍在运行`}
            </strong>
            <p>开启前已启动，不受此设置限制。</p>
          </div>
          <Button variant="ghost" size="sm" onClick={onTasks}>
            查看任务
            <ArrowRight size={14} />
          </Button>
        </div>
      )}
      {available && (
        <p className="muted policy-note">不限制服务器管理员配置的维护脚本。</p>
      )}
      {error && !confirm && (
        <p className="inline-error" role="alert">
          {error}
        </p>
      )}
      <Confirm
        open={confirm}
        title="允许 AI 修改项目文件？"
        action="允许修改"
        onCancel={() => C(false)}
        onConfirm={() => void save(false)}
        busy={busy}
        error={error}
      >
        后续任务可以修改代码；全自动任务无需逐项审批。
        {p?.queued_writes > 0 && (
          <p>{p.queued_writes} 个暂停的开发任务将恢复排队执行。</p>
        )}
      </Confirm>
    </>
  );
}
