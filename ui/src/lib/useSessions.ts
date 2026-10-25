import { useEffect, useRef, useState } from "react";
import type { RecordValue as Data } from "./conversation";
/** Connection and query generations reject late reads, including account switches. */
export function useSessions(
  connection: Data,
  invoke: (command: string, args?: Data) => Promise<any>,
  historyStamp = "",
) {
  const [items, setItems] = useState<Data[]>([]),
    [query, setQuery] = useState(""),
    [project, setProject] = useState(""),
    [archive, setArchive] = useState(false),
    [cursor, setCursor] = useState<string | null>(null),
    [error, setError] = useState(""),
    [loading, setLoading] = useState(false),
    [incomplete, setIncomplete] = useState(0),
    [metadata, setMetadata] = useState<Record<string, Data>>({});
  const lastConnection = useRef(connection);
  if (connection.connection_id) lastConnection.current = connection;
  const [hasUpdates, setHasUpdates] = useState(false);
  const initialized = useRef(false),
    lastStamp = useRef(historyStamp);
  useEffect(() => {
    if (initialized.current && lastStamp.current !== historyStamp)
      setHasUpdates(true);
    lastStamp.current = historyStamp;
  }, [historyStamp]);
  const generation = useRef(0),
    identity = lastConnection.current.connection_id || "";
  const supported =
    !!lastConnection.current.probe?.capabilities?.includes("sessions_v1");
  const requestRef = useRef(invoke);
  requestRef.current = invoke;
  const queryRef = useRef({ query, archive, identity, project });
  queryRef.current = { query, archive, identity, project };
  async function load(next?: string) {
    const current = ++generation.current;
    const context = queryRef.current;
    if (!connection.connected || !supported) return;
    setLoading(true);
    setError("");
    try {
      const r = await requestRef.current(
        context.query.trim() ? "session_search" : "session_list",
        {
          q: context.query.trim(),
          project: context.project,
          archive_filter: context.archive ? "archived" : "recent",
          cursor: next,
          connection_id: identity,
        },
      );
      if (
        current !== generation.current ||
        context.identity !== queryRef.current.identity ||
        context.query !== queryRef.current.query ||
        context.archive !== queryRef.current.archive ||
        context.project !== queryRef.current.project
      )
        return;
      setItems((old) =>
        next
          ? [...old, ...r.sessions].filter(
              (s, i, a) =>
                a.findIndex((x) => x.session_id === s.session_id) === i,
            )
          : r.sessions,
      );
      setMetadata((old) => ({
        ...old,
        ...Object.fromEntries(r.sessions.map((s: Data) => [s.session_id, s])),
      }));
      initialized.current = true;
      setHasUpdates(false);
      setCursor(r.next_cursor || null);
      setIncomplete(r.unreadable_count || 0);
    } catch (e: any) {
      if (current === generation.current) setError(e.message || "会话读取失败");
    } finally {
      if (current === generation.current) setLoading(false);
    }
  }
  useEffect(() => {
    generation.current++;
    setItems([]);
    setMetadata({});
    setCursor(null);
    setError("");
    setLoading(false);
    setHasUpdates(false);
    initialized.current = false;
  }, [identity]);
  useEffect(() => {
    generation.current++;
    if (!connection.connected) setLoading(false);
    const t = setTimeout(() => load(), 300);
    return () => {
      clearTimeout(t);
      generation.current++;
    };
  }, [query, project, archive, identity, supported, connection.connected]);
  async function edit(session: Data, patch: Data) {
    if (!connection.connected) throw new Error("连接服务器后再修改会话");
    const context = identity;
    const r = await requestRef.current("session_meta_set", {
      session_id: session.session_id,
      expected_revision: session.revision || 0,
      ...patch,
      connection_id: identity,
    });
    if (queryRef.current.identity !== context)
      throw new Error("连接账号已变化，请重新选择会话");
    setMetadata((old) => ({
      ...old,
      [session.session_id]: { ...session, ...r },
    }));
    await load();
    return r;
  }
  return {
    hasUpdates,
    offline: !connection.connected,
    metadata,
    items,
    query,
    setQuery,
    project,
    setProject,
    archive,
    setArchive,
    cursor,
    error,
    loading,
    incomplete,
    supported,
    load,
    edit,
    read: (session: Data) =>
      requestRef.current("session_meta_get", {
        session_id: session.session_id,
        connection_id: identity,
      }),
  };
}
