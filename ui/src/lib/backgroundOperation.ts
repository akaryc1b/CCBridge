export function operationSummary(op: {
  state?: string;
  processed?: number;
  total?: number;
}): string {
  if (op.state === "running") return `${op.processed || 0}/${op.total || "…"}`;
  return (
    (
      {
        done: "已完成",
        partial: "部分失败",
        failed: "失败",
        unknown: "结果待确认",
      } as Record<string, string>
    )[op.state || ""] || "查看结果"
  );
}
