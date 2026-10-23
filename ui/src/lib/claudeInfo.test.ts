import { describe, it, expect } from "vitest";
import {
  inheritedModelLabel,
  modelLabel,
  quotaReset,
} from "../components/ClaudeInfo";
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

describe("continued conversation model display", () => {
  const models = [
    { value: "opus", label: "Opus", resolved: "claude-opus-5-5" },
  ];
  it("resolves a saved alias exactly as the dashboard does", () => {
    expect(inheritedModelLabel("opus", models)).toBe("opus · claude-opus-5-5");
  });
  it("uses updated remote resolution without changing the saved alias", () => {
    expect(
      inheritedModelLabel("opus", [
        { ...models[0], resolved: "remote-new-opus" },
      ]),
    ).toBe("opus · remote-new-opus");
  });
  it("preserves concrete historical models and unknown defaults without inventing a version", () => {
    expect(inheritedModelLabel("claude-opus-5-5", models)).toBe(
      "claude-opus-5-5",
    );
    expect(inheritedModelLabel("opus", [])).toBe("opus");
    expect(inheritedModelLabel("远端默认", models)).toBe("远端默认");
    expect(inheritedModelLabel(undefined, models)).toBeUndefined();
  });
});
