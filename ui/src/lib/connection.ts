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

/** SSH trust is a human decision, not a generic connection failure. */
export function hostKeyCandidate(error: unknown): string | null {
  const message =
    typeof error === "object" && error !== null && "message" in error
      ? String((error as { message: unknown }).message)
      : String(error);
  return (
    /^HOST_KEY:(SHA256:[A-Za-z0-9+/]+={0,2})(?:\s|$)/.exec(message)?.[1] || null
  );
}
