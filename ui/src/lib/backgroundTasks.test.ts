import { describe, expect, it } from "vitest";
import { renderToStaticMarkup } from "react-dom/server";
import { createElement } from "react";
import { BackgroundTasks } from "../components/BackgroundTasks";
describe("background tasks entry", () => {
  it("keeps one task center entry without a false active count", () => {
    expect(
      renderToStaticMarkup(createElement(BackgroundTasks, { tasks: [] })),
    ).toContain('aria-label="任务中心"');
  });
  it("groups several operations behind one accessible entry", () => {
    const tasks = ["clone", "git", "auth", "branch"].map((id) => ({
      id,
      label: id,
      summary: "已完成",
      running: false,
      open: () => {},
    }));
    const html = renderToStaticMarkup(
      createElement(BackgroundTasks, { tasks }),
    );
    expect(html).toContain('aria-label="任务中心"');
    expect((html.match(/<button/g) || []).length).toBe(1);
  });
});
