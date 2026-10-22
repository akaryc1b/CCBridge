export const elapsed = (ms: number) =>
  `${Math.floor(Math.max(0, ms) / 60000)}:${String(Math.floor(Math.max(0, ms) / 1000) % 60).padStart(2, "0")}`;
export const bytes = (n: number) =>
  n >= 1048576
    ? `${(n / 1048576).toFixed(1)} MB`
    : `${Math.round(n / 1024)} KB`;
export const time = (seconds: number) =>
  new Date(seconds * 1000).toLocaleString("zh-CN", {
    month: "numeric",
    day: "numeric",
    hour: "2-digit",
    minute: "2-digit",
  });
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
