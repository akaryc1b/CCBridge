import { describe, it, expect } from "vitest";
import { modelLabel, quotaReset } from "../components/ClaudeInfo";
describe("remote catalog display", () => {
  const models = [
    { value: "default", label: "Default", resolved: "remote-model" },
    { value: "opus", label: "Custom", resolved: "gateway-model" },
  ];
  it("uses remote resolution and distinguishes aliases without an invented lineup", () => {
    expect(modelLabel("default", [])).toBe("跟随远端默认");
    expect(modelLabel("", models)).toBe("跟随远端默认 · remote-model");
    expect(modelLabel("opus", models)).toBe("opus · gateway-model");
    expect(modelLabel("not-returned", models)).toContain("不在远端列表");
  });
});

it("normalizes account ISO resets and legacy event seconds without inventing a time", () => {
  expect(quotaReset({ resets_at: 123 })).toBe(123);
  expect(quotaReset({ resets_at_iso: "2026-10-22T20:00:00Z" })).toBe(
    Date.parse("2026-10-22T20:00:00Z") / 1000,
  );
  expect(quotaReset({ resets_at_iso: null })).toBeNull();
  expect(quotaReset({ resets_at_iso: "invalid" })).toBeNull();
});
