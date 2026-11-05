import { useCallback, useEffect, useState } from "react";
import { markTasksRead, type ReadTasks } from "./taskRead";
const event = "ccbridge-task-read-change";
function load(key: string): ReadTasks {
  try {
    const v = JSON.parse(localStorage.getItem(key) || "{}");
    return v && typeof v === "object" && !Array.isArray(v)
      ? (Object.fromEntries(
          Object.entries(v).filter(([, value]) => typeof value === "string"),
        ) as ReadTasks)
      : {};
  } catch {
    return {};
  }
}
/** One local-account read store for both session list and task center. */
export function useTaskRead(account: string) {
  const key = `ccbridge.task-read.v1:${account || "offline"}`;
  const [state, setState] = useState<{ key: string; values: ReadTasks }>({
    key: "",
    values: {},
  });
  useEffect(() => {
    const update = (e?: Event) => {
      const d = (e as CustomEvent)?.detail;
      if (d?.key === key) setState({ key, values: d.values });
      else if (!d) setState({ key, values: load(key) });
    };
    update();
    window.addEventListener(event, update);
    window.addEventListener("storage", update);
    return () => {
      window.removeEventListener(event, update);
      window.removeEventListener("storage", update);
    };
  }, [key]);
  const markRead = useCallback(
    (tasks: Parameters<typeof markTasksRead>[0]) => {
      const values = markTasksRead(tasks, {
        ...load(key),
        ...(state.key === key ? state.values : {}),
      });
      setState({ key, values });
      try {
        localStorage.setItem(key, JSON.stringify(values));
      } catch {
        /* In-memory read state still works. */
      }
      window.dispatchEvent(new CustomEvent(event, { detail: { key, values } }));
    },
    [key, state],
  );
  return { read: state.key === key ? state.values : {}, markRead };
}
