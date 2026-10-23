import { describe, it, expect, vi } from "vitest";
import {
  authLabel,
  syncRequest,
  accessOperation,
  sourceLabel,
} from "./codeupAuth";
import { renderToStaticMarkup } from "react-dom/server";
import { createElement } from "react";
import { CodeupAuth } from "../components/CodeupAuth";
vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));
describe("compact Codeup authentication contract", () => {
  it("configuration is not repository access and does not invent credential validity", () => {
    expect(authLabel("local", { helper_configured: true }, {})).toBe(
      "本机 Git 助手",
    );
    expect(authLabel("local", {}, {})).toBe("未配置");
    expect(
      authLabel("remote", {}, { configured: true, username: "alice" }),
    ).toBe("已同步 · alice");
    expect(
      authLabel("remote", {}, { files_present: true, entry_present: false }),
    ).toBe("凭据已失效");
    expect(
      authLabel("remote", {}, { entry_present: true, included: false }),
    ).toBe("Git 配置未接入");
    expect(
      authLabel(
        "remote",
        { configured: true, storage: "keychain", username: "old" },
        {},
      ),
    ).toContain("需迁移");
    expect(authLabel("local", { error: "read failed" }, {})).toBe("读取失败");
  });
  it("retains the explicit helper default and supports both saved scopes without forwarding hidden drafts", () => {
    expect(
      syncRequest("git_helper", "cyb@host:22", "", "hidden", "secret"),
    ).toEqual({
      scope: "local",
      credential_source: "git_helper",
      vps_account: "cyb@host:22",
      url: "",
      username: "",
      secret: "",
    });
    expect(syncRequest("local_app", "a", "").scope).toBe("local");
    expect(syncRequest("remote_app", "a", "").scope).toBe("remote");
    expect(syncRequest("remote_app", "a", "").credential_source).toBe("app");
    expect(syncRequest("input", "a", "", "alice", "token").secret).toBe(
      "token",
    );
    expect(sourceLabel("local_app")).toBe("App 本机认证");
  });
  it("tests VPS itself when installed, retaining App injection and draft testing only on their own routes", () => {
    expect(accessOperation("remote", { configured: true }, false)).toBe(
      "git_vps_auth_test",
    );
    expect(accessOperation("remote", { configured: true }, true)).toBe(
      "git_access",
    );
    expect(accessOperation("remote", {}, false)).toBe("git_access");
    expect(accessOperation("local", {}, false)).toBe("git_access");
  });
  it("does not show a credential form by default while reading config", () => {
    // Radix portals intentionally omit modal children on SSR. This only checks safe rendering;
    // interactive contents and effects are exercised in the real browser/App acceptance.
    expect(() =>
      renderToStaticMarkup(
        createElement(CodeupAuth, {
          open: true,
          connected: true,
          account: "cyb@host:22",
          url: "",
          onOpenChange: () => {},
        }),
      ),
    ).not.toThrow();
  });
});
