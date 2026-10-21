/** Both startup and explicit connections share one visible operation state. */
export function isConnecting(busy: ReadonlySet<string>): boolean {
  return busy.has("auto_connect") || busy.has("connect_ssh");
}
export function clearConnectionErrors<T extends Record<string, unknown>>(
  errors: T,
): T {
  const next = { ...errors };
  delete next.auto_connect;
  delete next.connect_ssh;
  return next;
}
