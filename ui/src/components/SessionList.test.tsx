// @vitest-environment jsdom
import { cleanup, render, screen } from "@testing-library/react";
import { afterEach, expect, it, vi } from "vitest";
import { SessionList } from "./SessionList";
afterEach(cleanup);
it("keeps fixed search/filter controls separate from the scrolling tree and avoids repeated analysis labels", () => {
  const sessions: any = {
    supported: true,
    offline: false,
    loading: false,
    query: "",
    archive: false,
    project: "",
    items: [
      {
        session_id: "s",
        task_id: "t",
        workspace: "/qa",
        title: "登录流程分析",
        access: "analysis",
        state: "completed",
        updated_at: 1,
      },
    ],
    diagnostics: [],
    setQuery: vi.fn(),
    setArchive: vi.fn(),
    setProject: vi.fn(),
    load: vi.fn(),
  };
  render(
    <SessionList
      sessions={sessions}
      projects={[{ name: "神牛 EHS", path: "/qa" }]}
      selected="t"
      selectedProject="/qa"
      onSelect={() => {}}
      onUpdateRunner={() => {}}
      onAddProject={() => {}}
      onNewSession={() => {}}
      onRemoveProject={() => {}}
      removingProject={false}
    />,
  );
  expect(screen.getByRole("combobox", { name: "会话范围" })).toBeTruthy();
  expect(
    document
      .querySelector(".session-list-header")
      ?.contains(screen.getByRole("textbox", { name: "搜索会话" })),
  ).toBe(true);
  expect(document.querySelector(".session-tree-scroll")?.textContent).toContain(
    "登录流程分析",
  );
  expect(
    document.querySelector(".session-tree-scroll")?.textContent,
  ).not.toContain("分析 ·");
  expect(screen.getByText("已完成")).toBeTruthy();
  expect(screen.getByText("未读 1")).toBeTruthy();
  expect(
    screen.getByRole("button", { name: "在神牛 EHS新建会话" }),
  ).toBeTruthy();
});
