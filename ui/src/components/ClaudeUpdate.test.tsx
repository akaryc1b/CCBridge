// @vitest-environment jsdom
import { afterEach, expect, it, vi } from "vitest";
import {
  cleanup,
  fireEvent,
  render,
  screen,
  waitFor,
} from "@testing-library/react";
import { invoke } from "@tauri-apps/api/core";
import { ClaudeUpdate } from "./ClaudeUpdate";
vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));
vi.mock("../lib/useOperation", () => ({
  useOperation: () => ({ snap: null, error: "" }),
}));
afterEach(() => {
  cleanup();
  vi.clearAllMocks();
});
const connection = {
  connected: true,
  connection_id: "qa",
  probe: {
    claude_version: "2.1.308 (Claude Code)",
    capabilities: ["claude_update_v1"],
  },
};
it("only checks when requested and never upgrades just because a newer version is found", async () => {
  vi.mocked(invoke).mockResolvedValue({
    current_version: "2.1.308",
    available_version: "2.1.317",
    update_available: true,
    can_update: true,
    channel: "latest",
  });
  render(<ClaudeUpdate connection={connection} onRefresh={async () => {}} />);
  expect(invoke).not.toHaveBeenCalled();
  fireEvent.click(screen.getByRole("button", { name: "检查 Claude 更新" }));
  await waitFor(() =>
    expect(
      screen.getByRole("button", { name: "更新 Claude Code" }),
    ).toBeTruthy(),
  );
  expect(invoke).toHaveBeenCalledTimes(1);
  expect(invoke).toHaveBeenCalledWith("claude_update_check", {
    request: { connection_id: "qa" },
  });
  fireEvent.click(screen.getByRole("button", { name: "更新 Claude Code" }));
  expect(screen.getByRole("alertdialog")).toBeTruthy();
  fireEvent.click(screen.getByRole("button", { name: "取消" }));
  expect(invoke).toHaveBeenCalledTimes(1);
});
it("does not offer updates for a shared installation", async () => {
  vi.mocked(invoke).mockResolvedValue({
    current_version: "2.1.308",
    available_version: "2.1.317",
    update_available: true,
    can_update: false,
    install_kind: "shared",
  });
  render(<ClaudeUpdate connection={connection} onRefresh={async () => {}} />);
  fireEvent.click(screen.getByRole("button", { name: "检查 Claude 更新" }));
  await waitFor(() =>
    expect(screen.getByText("共享安装，由管理员更新")).toBeTruthy(),
  );
  expect(screen.queryByRole("button", { name: "更新 Claude Code" })).toBeNull();
});

it("reads back the updated version then refreshes runtime and model capabilities", async () => {
  const refreshed = vi.fn(async () => {});
  vi.mocked(invoke)
    .mockResolvedValueOnce({
      current_version: "2.1.308",
      available_version: "2.1.317",
      update_available: true,
      can_update: true,
      channel: "latest",
    })
    .mockResolvedValueOnce({
      current_version: "2.1.317",
      changed: true,
      message: "Claude Code 已更新",
    });
  render(<ClaudeUpdate connection={connection} onRefresh={refreshed} />);
  fireEvent.click(screen.getByRole("button", { name: "检查 Claude 更新" }));
  await waitFor(() =>
    expect(
      screen.getByRole("button", { name: "更新 Claude Code" }),
    ).toBeTruthy(),
  );
  fireEvent.click(screen.getByRole("button", { name: "更新 Claude Code" }));
  fireEvent.click(screen.getByRole("button", { name: "确定更新" }));
  await waitFor(() => expect(refreshed).toHaveBeenCalledTimes(1));
  expect(screen.getByText("2.1.317")).toBeTruthy();
  expect(vi.mocked(invoke).mock.calls[1][1]).toMatchObject({
    request: {
      confirmed: true,
      expected_version: "2.1.308",
      connection_id: "qa",
    },
  });
});
