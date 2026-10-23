/** Remote paths are not necessarily paths on the Mac client. */
export function normalizeRemotePath(path: string) {
  const win = /^(?:\\\\\?\\)?[a-z]:[\\/]/i.test(path);
  const value = win
    ? path
        .replace(/^\\\\\?\\/, "")
        .replaceAll("\\", "/")
        .toLowerCase()
    : path;
  return value.replace(/\/+$/, "");
}
export function remoteContains(root: string, path: string) {
  const r = normalizeRemotePath(root),
    p = normalizeRemotePath(path);
  return p === r || p.startsWith(r + "/");
}
export function remoteBasename(path: string) {
  return (
    path
      .replace(/[\\/]+$/, "")
      .split(/[\\/]/)
      .pop() || path
  );
}
export function remoteParent(path: string, home: string) {
  const trimmed = path.replace(/[\\/]+$/, "");
  const index = Math.max(trimmed.lastIndexOf("/"), trimmed.lastIndexOf("\\"));
  const parent = index > 0 ? trimmed.slice(0, index) : home;
  return remoteContains(home, parent) ? parent : home;
}
