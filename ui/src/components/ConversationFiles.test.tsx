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
import { ConversationFiles } from "./ConversationFiles";
vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));
afterEach(() => {
  cleanup();
  vi.clearAllMocks();
});
it("browses current workspace and references a file without sending or changing the task directory", async () => {
  vi.mocked(invoke).mockImplementation(async (op) =>
    op === "directory_list"
      ? {
          entries: [
            { name: "README.md", path: "/fixture/README.md", kind: "file" },
          ],
        }
      : { text: "# fixture" },
  );
  const ref = vi.fn();
  render(
    <ConversationFiles
      root="/fixture"
      connectionId="a"
      connected
      onReference={ref}
      onManage={vi.fn()}
      onAction={vi.fn()}
    />,
  );
  fireEvent.click(await screen.findByText("README.md"));
  expect(await screen.findByText("# fixture")).toBeTruthy();
  fireEvent.click(screen.getByText("引用到会话"));
  expect(ref).toHaveBeenCalledWith("/fixture/README.md");
  expect(
    vi
      .mocked(invoke)
      .mock.calls.every(([op]) =>
        ["directory_list", "file_preview"].includes(String(op)),
      ),
  ).toBe(true);
  expect(invoke).toHaveBeenCalledWith("file_preview", {
    request: { path: "/fixture/README.md", connection_id: "a" },
  });
});
it("ignores a file response arriving after switching accounts", async () => {
  let resolve: (v: any) => void = () => {};
  vi.mocked(invoke).mockImplementation(async (op) =>
    op === "directory_list"
      ? {
          entries: [
            { name: "old.txt", path: "/fixture/old.txt", kind: "file" },
          ],
        }
      : new Promise((r) => {
          resolve = r;
        }),
  );
  const props = {
    root: "/fixture",
    connectionId: "a",
    connected: true,
    onReference: vi.fn(),
    onManage: vi.fn(),
    onAction: vi.fn(),
  };
  const view = render(<ConversationFiles {...props} />);
  fireEvent.click(await screen.findByText("old.txt"));
  view.rerender(
    <ConversationFiles {...props} root="/other" connectionId="b" />,
  );
  resolve({ text: "old account secret" });
  await waitFor(() =>
    expect(screen.queryByText("old account secret")).toBeNull(),
  );
});
