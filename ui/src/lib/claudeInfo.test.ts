import { describe, it, expect } from "vitest";
import { modelLabel } from "../components/ClaudeInfo";
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
