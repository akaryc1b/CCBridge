import { describe, expect, it } from "vitest";
import { bytes, elapsed, time, gitResultSummary } from "./format";
describe("visible operation values", () => {
  it("labels binary transfer units without rounding zero into a percentage", () => {
    expect(bytes(0)).toBe("0 B");
    expect(bytes(2048)).toBe("2 KiB");
    expect(bytes(1048576)).toBe("1.0 MiB");
  });
  it("uses stable elapsed minutes and clamps pre-start clocks", () => {
    expect(elapsed(-10)).toBe("0:00");
    expect(elapsed(65000)).toBe("1:05");
  });
  it("shows a clock today and an unambiguous date for older activity", () => {
    const now = new Date(2026, 9, 22, 12, 0);
    expect(time(new Date(2026, 9, 22, 9, 5).getTime() / 1000, now)).toBe(
      "09:05",
    );
    expect(time(new Date(2026, 9, 21, 9, 5).getTime() / 1000, now)).toBe(
      "10月21日 09:05",
    );
    expect(time(0, now)).toBe("未记录时间");
  });
});

it("separates skipped guards from failed updates without changing protocol state", () => {
  expect(
    gitResultSummary([
      { state: "updated" },
      { state: "unchanged" },
      { state: "failed", code: "dirty" },
      { state: "failed", code: "auth" },
    ]),
  ).toBe("已快进 1 · 已是最新 1 · 跳过 1 · 失败 1");
});
