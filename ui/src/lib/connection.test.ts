import { describe, expect, it } from "vitest";
import {
  isConnecting,
  clearConnectionErrors,
  hostKeyCandidate,
} from "./connection";
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

describe("SSH host identity confirmation", () => {
  it("recognizes only a complete SSH SHA256 candidate", () => {
    expect(
      hostKeyCandidate({ message: "HOST_KEY:SHA256:abc/DEF+123\n核对指纹" }),
    ).toBe("SHA256:abc/DEF+123");
    expect(hostKeyCandidate("HOST_KEY:bad")).toBeNull();
    expect(hostKeyCandidate("连接失败 HOST_KEY:SHA256:abc")).toBeNull();
    expect(hostKeyCandidate({ message: "connection refused" })).toBeNull();
  });
});
