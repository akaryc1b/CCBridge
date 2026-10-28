// @vitest-environment jsdom
import { afterEach, describe, expect, it, vi } from "vitest";
import { createElement } from "react";
import {
  cleanup,
  fireEvent,
  render,
  screen,
  waitFor,
} from "@testing-library/react";
import { BranchDialog } from "../components/BranchDialog";
const state = vi.hoisted(() => ({
  calls: [] as any[],
  current: "main",
  failFetch: false,
  holdFetch: false,
  manualSnap: null as any,
  finishFetch: null as null | (() => void),
}));
vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(async (op: string, p: any) => {
    state.calls.push([op, p.request]);
    if (op === "branch_fetch" && state.holdFetch)
      await new Promise<void>((resolve) => {
        state.finishFetch = resolve;
      });
    if (op === "branch_fetch" && state.failFetch)
      throw Error("远端无法连接 Git 服务");
    if (op === "git_update_plan")
      return { repositories: [{ name: "repo", path: "/fixture/repo" }] };
    if (op === "branch_change") state.current = p.request.branch || "main";
    return {
      current: state.current,
      base: "main",
      base_oid: "base-oid",
      default_branch: "main",
      branches: ["main", "feature/merged"],
      remote_branches: ["origin/feature/remote"],
      details: [
        {
          name: "main",
          oid: "main-oid",
          merged: true,
          deletable: false,
          reason: "当前分支",
        },
        {
          name: "feature/merged",
          oid: "merged-oid",
          merged: true,
          deletable: true,
          reason: "",
        },
      ],
    };
  }),
}));
vi.mock("./useOperation", () => ({
  useOperation: (
    _kind: any,
    _path: any,
    _connection: any,
    _enabled: any,
    id: any,
  ) => ({
    snap:
      state.manualSnap ||
      (state.failFetch
        ? { id, state: "failed", error: "远端无法连接 Git 服务" }
        : null),
  }),
}));
vi.mock("../components/ui/dialog", () => {
  const Plain = ({ children }: any) => children;
  return Object.fromEntries(
    [
      "Dialog",
      "DialogContent",
      "DialogHeader",
      "DialogTitle",
      "DialogDescription",
      "DialogFooter",
    ].map((k) => [k, Plain]),
  );
});
vi.mock("../components/ui/select", () => ({
  Select: ({ children }: any) => children,
  SelectTrigger: ({ children }: any) => children,
  SelectValue: () => null,
  SelectContent: ({ children }: any) => children,
  SelectItem: ({ children }: any) => children,
}));
afterEach(() => {
  cleanup();
  state.calls = [];
  state.current = "main";
  state.failFetch = false;
  state.holdFetch = false;
  state.finishFetch = null;
  state.manualSnap = null;
});
const props = {
  open: true,
  path: "/fixture",
  connectionId: "connection-test",
  onClose: vi.fn(),
  onChanged: vi.fn(async () => {}),
};
describe("branch picker", () => {
  it("loads exact repository, separates search and creation, and binds changes to connection", async () => {
    render(createElement(BranchDialog, props));
    await screen.findByText("feature/merged");
    fireEvent.change(screen.getByLabelText("搜索分支"), {
      target: { value: "remote" },
    });
    expect(screen.queryByText("feature/merged")).toBeNull();
    expect(screen.getByText("origin/feature/remote")).toBeTruthy();
    fireEvent.click(screen.getByText("创建分支…"));
    const button = screen.getByText("创建并切换") as HTMLButtonElement;
    expect(button.disabled).toBe(true);
    fireEvent.change(screen.getByLabelText("分支名称"), {
      target: { value: "feature/new" },
    });
    fireEvent.click(button);
    await waitFor(() =>
      expect(
        state.calls.some(
          ([op, r]) =>
            op === "branch_change" &&
            r.action === "create" &&
            r.branch === "feature/new" &&
            r.start_point === "main" &&
            r.connection_id === "connection-test",
        ),
      ).toBe(true),
    );
  });
  it("switches remote tracking branch without mixing in ff-only update", async () => {
    render(createElement(BranchDialog, props));
    fireEvent.click(await screen.findByText("origin/feature/remote"));
    await waitFor(() =>
      expect(
        state.calls.some(
          ([op, r]) =>
            op === "branch_change" &&
            r.action === "track" &&
            r.branch === "feature/remote" &&
            r.remote === "origin/feature/remote",
        ),
      ).toBe(true),
    );
    expect(
      screen.queryByText("使用 origin/feature/remote 更新当前分支"),
    ).toBeNull();
  });
  it("shows one fetch failure and clears it on successful retry", async () => {
    render(createElement(BranchDialog, props));
    await screen.findByText("feature/merged");
    state.failFetch = true;
    fireEvent.click(screen.getByLabelText("刷新分支列表"));
    await waitFor(() =>
      expect(screen.getAllByText("远端无法连接 Git 服务")).toHaveLength(1),
    );
    state.failFetch = false;
    fireEvent.click(screen.getByText("重新检查"));
    await waitFor(() =>
      expect(screen.queryByText("远端无法连接 Git 服务")).toBeNull(),
    );
  });
  it("keeps branch fetching backgroundable without dispatching a second fetch", async () => {
    props.onClose.mockClear();
    render(createElement(BranchDialog, props));
    await screen.findByText("feature/merged");
    state.holdFetch = true;
    fireEvent.click(screen.getByLabelText("刷新分支列表"));
    const close = await screen.findByText("在后台继续");
    expect((close as HTMLButtonElement).disabled).toBe(false);
    fireEvent.click(close);
    expect(props.onClose).toHaveBeenCalledTimes(1);
    expect(state.calls.filter(([op]) => op === "branch_fetch")).toHaveLength(1);
    state.holdFetch = false;
    state.finishFetch?.();
    await waitFor(() => expect(screen.queryByText("在后台继续")).toBeNull());
  });
  it("rereads branches once when a background fetch finishes without fetching again", async () => {
    const ui = render(createElement(BranchDialog, props));
    await screen.findByText("feature/merged");
    const before = state.calls.filter(([op]) => op === "branches").length;
    state.manualSnap = {
      id: "background-done",
      state: "done",
      path: "/fixture/repo",
      connection_id: "connection-test",
    };
    ui.rerender(createElement(BranchDialog, props));
    await waitFor(() =>
      expect(state.calls.filter(([op]) => op === "branches")).toHaveLength(
        before + 1,
      ),
    );
    ui.rerender(createElement(BranchDialog, props));
    expect(state.calls.filter(([op]) => op === "branches")).toHaveLength(
      before + 1,
    );
    expect(state.calls.filter(([op]) => op === "branch_fetch")).toHaveLength(0);
  });
});
