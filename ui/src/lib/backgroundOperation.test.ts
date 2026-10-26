import { describe, expect, it } from "vitest";
import { operationSummary } from "./backgroundOperation";
describe("background operation result", () => {
  it("keeps terminal and unknown states distinct", () => {
    expect(operationSummary({ state: "running", processed: 1, total: 3 })).toBe(
      "1/3",
    );
    expect(operationSummary({ state: "done" })).toBe("已完成");
    expect(operationSummary({ state: "partial" })).toBe("部分失败");
    expect(operationSummary({ state: "failed" })).toBe("失败");
    expect(operationSummary({ state: "unknown" })).toBe("结果待确认");
  });
});
