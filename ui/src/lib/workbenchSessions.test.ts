import { describe, it, expect } from "vitest";
import { createElement } from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { Workbench } from "../components/Workbench";
describe("workbench session metadata", () => {
  it("shows stable names and omits archived sessions without changing prompts", () => {
    const tasks = [
      {
        task_id: "1",
        session_id: "s1",
        prompt: "latest user question",
        created_at: 2,
        workspace: "/p",
        session_meta: { title: "稳定名称", archived: false },
      },
      {
        task_id: "2",
        session_id: "s2",
        prompt: "archived private result",
        created_at: 3,
        workspace: "/p",
        session_meta: { title: "已归档名称", archived: true },
      },
    ];
    const html = renderToStaticMarkup(
      createElement(Workbench, {
        projects: [],
        tasks,
        currentProject: "/p",
        connection: {},
        defaultModel: "",
        usage: null,
        onFiles: () => {},
        onNew: () => {},
        onSession: () => {},
        onPage: () => {},
      }),
    );
    expect(html).toContain("稳定名称");
    expect(html).not.toContain("已归档名称");
    expect(html).not.toContain("latest user question");
    expect(tasks[0].prompt).toBe("latest user question");
  });
});
