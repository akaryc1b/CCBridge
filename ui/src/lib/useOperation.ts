import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
export type Op = Record<string, any>;
export function useOperation(
  kind: string,
  path: string,
  connectionId: string,
  enabled: boolean,
  operationId?: string,
) {
  const [snap, setSnap] = useState<Op | null>(null),
    [error, setError] = useState("");
  useEffect(() => {
    setSnap(null);
    setError("");
  }, [connectionId, path, operationId]);
  useEffect(() => {
    if (!enabled || !connectionId) return;
    let active = true,
      t: ReturnType<typeof setTimeout>;
    let misses = 0;
    const poll = async () => {
      try {
        const s = await invoke<Op>("operation_progress", {
          request: {
            kind,
            path,
            ...(operationId ? { operation_id: operationId } : {}),
          },
        });
        if (!active) return;
        if (s?.id && s.connection_id === connectionId && s.path === path) {
          setSnap(s);
          setError("");
          if (s.state === "running") t = setTimeout(poll, 700);
        } else if (operationId && ++misses < 12) {
          t = setTimeout(poll, 700);
        } else if (operationId) {
          setError("未找到此操作，请刷新确认结果");
        }
      } catch (e) {
        if (active)
          setError(
            typeof e === "object" && e && "message" in e
              ? String(e.message)
              : String(e),
          );
      }
    };
    void poll();
    return () => {
      active = false;
      clearTimeout(t);
    };
  }, [kind, path, connectionId, enabled, operationId]);
  return { snap, error };
}
