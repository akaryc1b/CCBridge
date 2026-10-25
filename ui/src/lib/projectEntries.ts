import { groupSessions, type RecordValue as Data } from "./conversation";
export function projectAtPath(path: string, projects: Data[]) {
  return projects.find(
    (p) => p.path.replace(/\/$/, "") === path.replace(/\/$/, ""),
  );
}
export function directoryName(path: string) {
  return path.replace(/\/$/, "").split("/").pop() || path;
}
export function parentDirectory(path: string, home: string) {
  return path === home ? home : path.slice(0, path.lastIndexOf("/")) || "/";
}
/** Empty groups belong to unfiltered recent view only, never dilute search/archive results. */
export function projectSessionGroups(
  items: Data[],
  projects: Data[],
  query: string,
  archive: boolean,
  project: string,
) {
  const groups = groupSessions(items, projects);
  if (query.trim() || archive) return groups;
  for (const p of projects) {
    if (
      (!project || p.path === project) &&
      !groups.some((g) => g.path === p.path)
    )
      groups.push({
        path: p.path,
        name: p.name,
        registered: true,
        sessions: [],
      });
  }
  return groups;
}
