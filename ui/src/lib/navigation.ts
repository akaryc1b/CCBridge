export function containingProject(path: string, projects: { path: string }[]) {
  const normalized = path.replace(/\/$/, "");
  return (
    projects
      .filter(
        (p) =>
          normalized === p.path ||
          normalized.startsWith(p.path.replace(/\/$/, "") + "/"),
      )
      .sort((a, b) => b.path.length - a.path.length)[0]?.path || ""
  );
}
/** Ignore stale completions; browsing must never commit an earlier click over a later one. */
export function latestNavigation<T>(
  load: (path: string) => Promise<T>,
  commit: (data: T) => void,
  pending: (value: boolean) => void,
) {
  let generation = 0;
  return async (path: string) => {
    const id = ++generation;
    pending(true);
    try {
      const data = await load(path);
      if (id === generation) commit(data);
    } finally {
      if (id === generation) pending(false);
    }
  };
}
