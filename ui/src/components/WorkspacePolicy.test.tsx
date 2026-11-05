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
import { WorkspacePolicy } from "./WorkspacePolicy";
vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn(async () => ({})) }));
afterEach(() => {
  cleanup();
  vi.clearAllMocks();
});
it("requires human confirmation and pins workspace, revision and connection", async () => {
  const refresh = vi.fn(async () => ({}));
  render(
    <WorkspacePolicy
      path="/fixture"
      refresh={refresh}
      connection={{
        connected: true,
        connection_id: "conn",
        policy: { read_only: true, revision: 7 },
        probe: { capabilities: ["workspace_policy_v1"] },
      }}
    />,
  );
  fireEvent.click(screen.getByText("仅分析 · 工作区权限"));
  expect(invoke).not.toHaveBeenCalled();
  fireEvent.click(screen.getByRole("button", { name: "允许此工作区写入" }));
  await waitFor(() =>
    expect(invoke).toHaveBeenCalledWith("policy_set", {
      request: {
        workspace: "/fixture",
        write_enabled: true,
        confirmed: true,
        expected_revision: 7,
        connection_id: "conn",
      },
    }),
  );
  expect(refresh).toHaveBeenCalled();
});
it("does not show permission granted while policy could not be read", () => {
  render(
    <WorkspacePolicy
      path="/fixture"
      refresh={vi.fn()}
      connection={{
        connected: true,
        connection_id: "conn",
        probe: { capabilities: ["workspace_policy_v1"] },
      }}
    />,
  );
  expect(
    (screen.getByText("工作区权限 · 未读取") as HTMLButtonElement).disabled,
  ).toBe(true);
});
