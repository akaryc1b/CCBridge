// @vitest-environment jsdom
import { afterEach, expect, it, vi } from "vitest";
import { cleanup, render, waitFor } from "@testing-library/react";
import { invoke } from "@tauri-apps/api/core";
import App from "./App";
vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(),
  isTauri: () => true,
}));
vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn(async () => () => {}),
}));
afterEach(() => {
  cleanup();
  vi.restoreAllMocks();
  vi.unstubAllGlobals();
});
it("keeps connection-scoped sibling dialogs uniquely keyed during connection refresh", async () => {
  vi.stubGlobal("localStorage", { getItem: () => null, setItem: () => {} });
  vi.stubGlobal("matchMedia", () => ({
    matches: false,
    addEventListener() {},
    removeEventListener() {},
  }));
  vi.stubGlobal("requestAnimationFrame", (f: FrameRequestCallback) =>
    setTimeout(() => f(0), 0),
  );
  vi.stubGlobal("cancelAnimationFrame", clearTimeout);
  const errors = vi.spyOn(console, "error").mockImplementation(() => {});
  vi.mocked(invoke).mockImplementation(async (command) => {
    if (command === "initial_state")
      return { version: "fixture", profile: {}, preferences: {} };
    if (command === "connection_status")
      return {
        connected: true,
        connection_id: "fixture-connection",
        profile: { user: "fixture", host: "localhost", port: 22 },
        probe: { capabilities: [] },
      };
    if (command === "task_list") return { tasks: [] };
    if (command === "project_list") return { projects: [] };
    return {};
  });
  render(<App />);
  await waitFor(() =>
    expect(invoke).toHaveBeenCalledWith("connection_status", expect.anything()),
  );
  await waitFor(() =>
    expect(invoke).toHaveBeenCalledWith("project_list", expect.anything()),
  );
  expect(
    errors.mock.calls.filter((args) =>
      args.some((v) => String(v).includes("same key")),
    ),
  ).toHaveLength(0);
});
