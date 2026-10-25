import { useEffect, useState, useRef } from "react";
import { invoke } from "@tauri-apps/api/core";
import type { RecordValue as Data } from "./conversation";
export function useTaskCenter(connection: Data) {
  const lastConnection = useRef(connection);
  if (connection.connection_id) lastConnection.current = connection;
  const accountConnectionId = lastConnection.current.connection_id;
  const [operations, setOperations] = useState<Data[]>([]),
    [tasks, setTasks] = useState<Data[]>([]),
    [error, setError] = useState("");
  useEffect(() => {
    setOperations([]);
    setTasks([]);
    setError("");
  }, [accountConnectionId]);
  useEffect(() => {
    let alive = true,
      t: ReturnType<typeof setTimeout>;
    async function poll() {
      try {
        const [ops, active] = await Promise.all([
          invoke<Data>("operation_list", {
            request: { connection_id: connection.connection_id },
          }),
          connection.connected &&
          connection.probe?.capabilities?.includes("sessions_v1")
            ? invoke<Data>("active_tasks", {
                request: { connection_id: connection.connection_id },
              })
            : Promise.resolve<Data>({ tasks: [] }),
        ]);
        if (!alive) return;
        setOperations((old) =>
          connection.connected
            ? ops.operations || []
            : [
                ...old.filter((o) => o.account !== "local"),
                ...(ops.operations || []),
              ],
        );
        if (connection.connected)
          setTasks(
            (active.tasks || []).map((t: Data) => ({
              ...t,
              task_id: t.task_id || t.id,
            })),
          );
        setError(
          ops.history_error ||
            (active.incomplete ? "部分活动任务无法读取" : ""),
        );
      } catch (e: any) {
        if (alive) setError(e.message || "任务状态读取失败");
      } finally {
        if (alive) t = setTimeout(poll, 3000);
      }
    }
    void poll();
    return () => {
      alive = false;
      clearTimeout(t);
    };
  }, [
    connection.connected,
    connection.connection_id,
    connection.probe?.runner_version,
  ]);
  return { operations, tasks, error };
}
