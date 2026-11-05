import { useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { Button } from "./ui/button";
import { Confirm } from "./common/Confirm";
import { errorText } from "./CodeupAuth";
export function workspaceProtected(
  policy: Record<string, any> | undefined,
  path: string,
) {
  return (
    policy?.read_only === true &&
    !(policy.writable_workspaces || []).some(
      (root: string) => path === root || path.startsWith(root + "/"),
    )
  );
}
export function WorkspacePolicy({
  connection,
  path,
  refresh,
}: {
  connection: Record<string, any>;
  path: string;
  refresh: () => Promise<any>;
}) {
  const [confirm, setConfirm] = useState(false),
    [busy, setBusy] = useState(false),
    [error, setError] = useState("");
  const p = connection.policy;
  const available = typeof p?.read_only === "boolean";
  const own = (p?.writable_workspaces || []).includes(path);
  const inherited = (p?.writable_workspaces || []).some(
    (root: string) => root !== path && path.startsWith(root + "/"),
  );
  const supported = connection.probe?.capabilities?.includes(
    "workspace_policy_v1",
  );
  async function save() {
    setBusy(true);
    setError("");
    try {
      await invoke("policy_set", {
        request: {
          workspace: path,
          write_enabled: !own,
          confirmed: true,
          expected_revision: p.revision,
          connection_id: connection.connection_id,
        },
      });
      await refresh();
      setConfirm(false);
    } catch (e) {
      setError(errorText(e));
    } finally {
      setBusy(false);
    }
  }
  return (
    <>
      <Button
        size="sm"
        variant="outline"
        disabled={
          !connection.connected ||
          !supported ||
          !path ||
          busy ||
          inherited ||
          !available
        }
        onClick={() => setConfirm(true)}
        title={
          inherited
            ? "此目录继承父工作区写入例外"
            : !supported
              ? "更新服务端后设置工作区权限"
              : path
        }
      >
        {!connection.connected
          ? "工作区权限"
          : !supported
            ? "工作区权限 · 需更新服务端"
            : !available
              ? "工作区权限 · 未读取"
              : workspaceProtected(p, path)
                ? "仅分析 · 工作区权限"
                : "允许写入 · 工作区权限"}
      </Button>
      <Confirm
        open={confirm}
        title={own ? "移除此工作区写入例外？" : "允许此工作区写入？"}
        action={own ? "移除例外" : "允许此工作区写入"}
        busy={busy}
        error={error}
        onCancel={() => setConfirm(false)}
        onConfirm={() => void save()}
      >
        <p>{path}</p>
        <p>
          {own
            ? "后续任务重新遵循全局保护。"
            : "后续任务及下一轮续聊可选择开发，此目录和其子目录中的项目会使用写入例外。"}{" "}
          当前运行任务不变。
        </p>
        <p>
          写入任务使用此 SSH
          账号的命令与文件权限；这不是操作系统沙箱，不限制命令对目录外资源的访问。
        </p>
      </Confirm>
    </>
  );
}
