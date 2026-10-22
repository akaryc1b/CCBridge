import { useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { errorText } from "@/components/CodeupAuth";
export type RemoteModel = { value: string; label: string; resolved?: string };
type State = { models: RemoteModel[]; loading: boolean; error: string };
export function useClaudeInfo(connectionId: string, supported: boolean) {
  const [catalog, setCatalog] = useState<State>({
    models: [],
    loading: false,
    error: "",
  });
  const [usage, setUsage] = useState<Record<string, any>>({});
  const generation = useRef(0);
  const pending = useRef(new Set<string>());
  async function load(op: "claude_models" | "claude_usage") {
    if (!connectionId || !supported || pending.current.has(op)) return;
    const seq = generation.current;
    pending.current.add(op);
    if (op === "claude_models")
      setCatalog((s) => ({ ...s, loading: true, error: "" }));
    else setUsage((s) => ({ ...s, loading: true, error: "" }));
    try {
      const data = await invoke<Record<string, any>>(op, {
        request: { connection_id: connectionId },
      });
      if (seq !== generation.current) return;
      if (op === "claude_models")
        setCatalog({ models: data.models, loading: false, error: "" });
      else setUsage({ ...data, loading: false });
    } catch (e) {
      if (seq !== generation.current) return;
      if (op === "claude_models")
        setCatalog((s) => ({ ...s, loading: false, error: errorText(e) }));
      else setUsage((s) => ({ ...s, loading: false, error: errorText(e) }));
    } finally {
      if (seq === generation.current) pending.current.delete(op);
    }
  }
  useEffect(() => {
    generation.current++;
    pending.current = new Set();
    setCatalog({ models: [], loading: false, error: "" });
    setUsage({});
    if (connectionId && supported) {
      void load("claude_models");
      void load("claude_usage");
    }
    return () => {
      generation.current++;
    };
  }, [connectionId, supported]);
  return {
    catalog,
    usage,
    refreshModels: () => load("claude_models"),
    refreshUsage: () => load("claude_usage"),
  };
}
