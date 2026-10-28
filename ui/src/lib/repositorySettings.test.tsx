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
import { RepositorySettings } from "../components/RepositorySettings";
const calls = vi.hoisted(() => [] as any[]);
vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(async (op: string, v: any) => {
    calls.push([op, v.request]);
    if (op === "branches")
      return {
        remote_details: [
          { name: "origin/main", head: "m" },
          { name: "origin/mobile", head: "n" },
        ],
      };
    return { configured: true };
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
  Select: ({ children, value, onValueChange }: any) =>
    createElement(
      "select",
      {
        "aria-label": "choice",
        value,
        onChange: (e: any) => onValueChange(e.target.value),
      },
      children,
    ),
  SelectTrigger: () => null,
  SelectValue: () => null,
  SelectContent: ({ children }: any) => children,
  SelectItem: ({ children, value }: any) =>
    createElement("option", { value }, children),
}));
afterEach(() => {
  cleanup();
  calls.length = 0;
});
const repo = {
  path: "/fixture/repo",
  name: "fixture",
  branch: "main",
  head: "full-head",
  upstream: "",
  remotes: [
    { name: "origin", url: "https://codeup.aliyun.com/org/a.git" },
    { name: "backup", url: "https://codeup.aliyun.com/org/b.git" },
  ],
};
const props = {
  repo,
  connectionId: "session",
  onClose: vi.fn(),
  onChanged: vi.fn(),
  onAuth: vi.fn(),
  onFetch: vi.fn(),
};
describe("repository settings", () => {
  it("retains upstream selection across confirmation back and pins target commit", async () => {
    render(createElement(RepositorySettings, { ...props, kind: "upstream" }));
    await screen.findByText("origin/mobile");
    fireEvent.change(screen.getByLabelText("choice"), {
      target: { value: "origin/mobile" },
    });
    fireEvent.click(screen.getByText("下一步"));
    expect(screen.getByText("未设置")).toBeTruthy();
    fireEvent.click(screen.getByText("返回"));
    expect((screen.getByLabelText("choice") as HTMLSelectElement).value).toBe(
      "origin/mobile",
    );
    fireEvent.click(screen.getByText("下一步"));
    fireEvent.click(screen.getByText("确定设置"));
    await waitFor(() =>
      expect(
        calls.some(
          ([op, r]) =>
            op === "branch_change" &&
            r.target === "origin/mobile" &&
            r.target_head === "n" &&
            r.expected_head === "full-head" &&
            r.path === "/fixture/repo" &&
            r.connection_id === "session",
        ),
      ).toBe(true),
    );
  });
  it("tests selected remote and carries its exact address to authentication", async () => {
    render(createElement(RepositorySettings, { ...props, kind: "remote" }));
    fireEvent.change(screen.getByLabelText("choice"), {
      target: { value: "backup" },
    });
    fireEvent.click(screen.getByText("检测仓库访问"));
    await waitFor(() =>
      expect(
        calls.some(
          ([op, r]) =>
            op === "git_remote_access" &&
            r.remote === "backup" &&
            r.expected_url.endsWith("/b.git"),
        ),
      ).toBe(true),
    );
    fireEvent.click(screen.getByText("获取此远端"));
    expect(props.onFetch).toHaveBeenLastCalledWith("/fixture/repo", "backup");
    fireEvent.click(screen.getByText("管理 / 同步认证…"));
    expect(props.onAuth).toHaveBeenLastCalledWith(
      "https://codeup.aliyun.com/org/b.git",
    );
  });
});
