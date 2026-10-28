// @vitest-environment jsdom
import { afterEach, describe, expect, it, vi } from "vitest";
import { createElement } from "react";
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { invoke } from "@tauri-apps/api/core";
import { RepositoryOverview } from "../components/RepositoryOverview";
vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(async () => ({
    repositories: [
      {
        path: "/fixture/project/a",
        name: "backend",
        branch: "main",
        head: "a",
        upstream: "origin/main",
        tracking_known: true,
        ahead: 0,
        behind: 3,
        remotes: [
          { name: "origin", url: "https://codeup.aliyun.com/org/a.git" },
        ],
      },
      {
        path: "/fixture/project/b",
        name: "mobile",
        branch: "master",
        head: "b",
        upstream: "origin/master",
        dirty: true,
        changed_files: [" M src/index.vue"],
        remotes: [],
      },
    ],
  })),
}));
afterEach(cleanup);
describe("repository overview routes", () => {
  it("keeps all update while single update and branch use the selected exact repository", async () => {
    const update = vi.fn(),
      branches = vi.fn(),
      fetch = vi.fn();
    render(
      createElement(RepositoryOverview, {
        path: "/fixture/project",
        connectionId: "test",
        refreshKey: 0,
        onUpdate: update,
        onBranches: branches,
        onFetch: fetch,
        onAuth: vi.fn(),
        onBrowse: vi.fn(),
        onOpenTask: vi.fn(),
      }),
    );
    await screen.findAllByText("backend");
    fireEvent.click(screen.getByText("全部更新"));
    expect(update).toHaveBeenLastCalledWith("/fixture/project");
    fireEvent.click(screen.getByText("更新仓库"));
    expect(update).toHaveBeenLastCalledWith("/fixture/project/a");
    fireEvent.click(screen.getAllByText("获取最新状态")[0]);
    expect(fetch).toHaveBeenLastCalledWith("/fixture/project");
    fireEvent.click(screen.getByText("mobile"));
    expect((screen.getByText("更新仓库") as HTMLButtonElement).disabled).toBe(
      true,
    );
    fireEvent.click(screen.getByText("查看文件"));
    expect(screen.getByText("src/index.vue")).toBeTruthy();
  });
});

it("does not report a normal project without Git as a failure", async () => {
  vi.mocked(invoke).mockRejectedValueOnce({
    message: "此目录及直接子目录没有 Git 仓库，请选择仓库或项目根目录",
  });
  render(
    createElement(RepositoryOverview, {
      path: "/fixture/empty",
      connectionId: "test",
      refreshKey: 0,
      onUpdate: vi.fn(),
      onBranches: vi.fn(),
      onFetch: vi.fn(),
      onAuth: vi.fn(),
      onBrowse: vi.fn(),
      onOpenTask: vi.fn(),
    }),
  );
  expect(await screen.findByText("暂无仓库")).toBeTruthy();
  expect(screen.queryByRole("alert")).toBeNull();
  expect(screen.queryByText("全部更新")).toBeNull();
});
