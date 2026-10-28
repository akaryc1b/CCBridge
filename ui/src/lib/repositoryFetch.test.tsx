// @vitest-environment jsdom
import { afterEach, expect, it, vi } from "vitest";
import { createElement } from "react";
import {
  cleanup,
  fireEvent,
  render,
  screen,
  waitFor,
} from "@testing-library/react";
import { invoke } from "@tauri-apps/api/core";
import { RepositoryFetch } from "../components/RepositoryFetch";
vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn(async () => ({})) }));
vi.mock("@/lib/useOperation", () => ({
  useOperation: () => ({
    snap: {
      id: "op",
      state: "unknown",
      phase: "fetch",
      started_at_ms: Date.now(),
      remote_lease: { path: "/fixture/a", token: "lease" },
    },
    error: "",
  }),
}));
afterEach(() => {
  cleanup();
  vi.clearAllMocks();
});
it("reopens an uncertain fetch without replaying it and verifies the original operation", async () => {
  render(
    createElement(RepositoryFetch, {
      request: { path: "/fixture/a", id: "op", launch: false },
      connectionId: "connection",
      open: true,
      onClose: vi.fn(),
      onChanged: vi.fn(),
      onAuth: vi.fn(),
      onRetry: vi.fn(),
    }),
  );
  expect(
    vi.mocked(invoke).mock.calls.some(([name]) => name === "git_fetch"),
  ).toBe(false);
  fireEvent.click(screen.getByText("核实原操作"));
  await waitFor(() =>
    expect(invoke).toHaveBeenCalledWith("operation_verify", {
      request: { operation_id: "op", connection_id: "connection" },
    }),
  );
  expect(
    await screen.findByText("已核实当前仓库状态，未重新获取。"),
  ).toBeTruthy();
  fireEvent.click(screen.getByText("释放占用…"));
  expect(screen.getByText("释放获取占用？")).toBeTruthy();
  expect(
    vi.mocked(invoke).mock.calls.some(([name]) => name === "operation_release"),
  ).toBe(false);
  fireEvent.click(screen.getByText("确认已停止并释放"));
  await waitFor(() =>
    expect(invoke).toHaveBeenCalledWith("operation_release", {
      request: {
        operation_id: "op",
        connection_id: "connection",
        confirmed: true,
      },
    }),
  );
  await waitFor(() => expect(screen.queryByText("释放占用…")).toBeNull());
});
