export const elapsed = (ms: number) =>
  `${Math.floor(Math.max(0, ms) / 60000)}:${String(Math.floor(Math.max(0, ms) / 1000) % 60).padStart(2, "0")}`;
export const bytes = (n: number) =>
  n >= 1048576
    ? `${(n / 1048576).toFixed(1)} MiB`
    : n >= 1024
      ? `${Math.round(n / 1024)} KiB`
      : `${Math.max(0, Math.round(n))} B`;
export function time(seconds: number, now = new Date()) {
  if (!Number.isFinite(seconds) || seconds <= 0) return "未记录时间";
  const date = new Date(seconds * 1000);
  const clock = date.toLocaleTimeString("zh-CN", {
    hour: "2-digit",
    minute: "2-digit",
    hour12: false,
  });
  return date.toDateString() === now.toDateString()
    ? clock
    : `${date.getMonth() + 1}月${date.getDate()}日 ${clock}`;
}
export const phaseText: Record<string, string> = {
  checking: "检查",
  fetch: "获取",
  upload: "上传",
  import: "导入",
  apply: "更新代码",
  verify: "校验",
  receiving: "接收对象",
  resolving: "解析对象",
};

export function gitResultSummary(results: Record<string, any>[]) {
  const counts = {
    fetched: 0,
    updated: 0,
    unchanged: 0,
    ahead: 0,
    skipped: 0,
    failed: 0,
  };
  for (const row of results) {
    if (row.blocked || ["dirty", "no_upstream"].includes(row.code))
      counts.skipped++;
    else if (row.state in counts) counts[row.state as keyof typeof counts]++;
  }
  const labels = {
    fetched: "已获取",
    updated: "已更新",
    unchanged: "已是最新",
    ahead: "有未推送提交",
    skipped: "跳过",
    failed: "失败",
  };
  return (Object.keys(counts) as (keyof typeof counts)[])
    .filter((key) => counts[key] > 0)
    .map((key) => `${labels[key]} ${counts[key]}`)
    .join(" · ");
}
