import { describe, expect, it } from "vitest";
import { isConnecting, clearConnectionErrors } from "./connection";
describe("SSH connection feedback", () => {
  it("shows one connecting state for startup and manual requests", () => {
    expect(isConnecting(new Set(["auto_connect"]))).toBe(true);
    expect(isConnecting(new Set(["connect_ssh"]))).toBe(true);
    expect(isConnecting(new Set(["task_start"]))).toBe(false);
  });
  it("successful connection clears stale startup errors without hiding other errors", () => {
    const old = {
      auto_connect: "offline",
      connect_ssh: "old",
      save_profile: "keep",
    };
    expect(clearConnectionErrors(old)).toEqual({ save_profile: "keep" });
    expect(old.auto_connect).toBe("offline");
  });
});
