// @vitest-environment jsdom
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import {
  act,
  cleanup,
  fireEvent,
  render,
  screen,
  waitFor,
} from "@testing-library/react";
import { invoke } from "@tauri-apps/api/core";
import { toast } from "sonner";
import { FileOperation, TrashDialog } from "./WorkspaceTools";
vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));
vi.mock("sonner", () => ({ toast: { success: vi.fn(), error: vi.fn() } }));
afterEach(cleanup);
beforeEach(() => vi.clearAllMocks());
const operation = {
  action: "rename",
  entry: { name: "old.txt", path: "/fixture/old.txt" },
};
describe("file operation connection and result boundaries", () => {
  it("pins the file write to the connection which opened it", async () => {
    vi.mocked(invoke).mockResolvedValue({ message: "renamed" });
    render(
      <FileOperation
        operation={operation}
        connectionId="conn-a"
        home="/fixture"
        onClose={() => {}}
        onDone={async () => {}}
      />,
    );
    fireEvent.change(screen.getByLabelText("名称"), {
      target: { value: "new.txt" },
    });
    fireEvent.click(screen.getByRole("button", { name: "重命名" }));
    await waitFor(() =>
      expect(invoke).toHaveBeenCalledWith("file_mutate", {
        request: {
          path: "/fixture/old.txt",
          action: "rename",
          name: "new.txt",
          destination: "",
          connection_id: "conn-a",
        },
      }),
    );
  });
  it("does not refresh or close a newer context after an unmounted write completes", async () => {
    let finish!: (v: unknown) => void;
    vi.mocked(invoke).mockImplementation(
      () =>
        new Promise((r) => {
          finish = r;
        }),
    );
    const done = vi.fn(async () => {}),
      close = vi.fn();
    const view = render(
      <FileOperation
        operation={operation}
        connectionId="conn-a"
        home="/fixture"
        onClose={close}
        onDone={done}
      />,
    );
    fireEvent.click(screen.getByRole("button", { name: "重命名" }));
    view.unmount();
    await act(async () => {
      finish({ message: "done" });
    });
    expect(done).not.toHaveBeenCalled();
    expect(close).not.toHaveBeenCalled();
    expect(toast.success).not.toHaveBeenCalled();
  });
  it("keeps an acknowledged restore successful even when the directory refresh fails", async () => {
    let lists = 0;
    vi.mocked(invoke).mockImplementation(async (command) => {
      if (command === "trash_list")
        return {
          items:
            lists++ === 0
              ? [
                  {
                    id: "trash-1",
                    name: "old.txt",
                    original: "/fixture/old.txt",
                  },
                ]
              : [],
        };
      return {};
    });
    render(
      <TrashDialog
        open
        connectionId="conn-a"
        onClose={() => {}}
        onDone={async () => {
          throw new Error("directory refresh failed");
        }}
      />,
    );
    await screen.findByText("old.txt");
    fireEvent.click(screen.getByRole("button", { name: "恢复" }));
    await waitFor(() => expect(screen.queryByText("old.txt")).toBeNull());
    expect(invoke).toHaveBeenCalledWith("trash_restore", {
      request: { id: "trash-1", connection_id: "conn-a" },
    });
    expect(lists).toBe(2);
    expect(screen.queryByText("directory refresh failed")).toBeNull();
  });
});
