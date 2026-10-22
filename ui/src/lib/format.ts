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
  apply: "快进",
  verify: "校验",
  receiving: "接收对象",
  resolving: "解析对象",
};
