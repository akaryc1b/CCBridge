import { describe, expect, it, vi } from "vitest";
import { createElement } from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { GitUpdate } from "../components/GitUpdate";

const state = vi.hoisted(() => ({ snap: null as Record<string, any> | null }));
vi.mock("./useOperation", () => ({
  useOperation: () => ({ snap: state.snap, error: "" }),
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
    ].map((key) => [key, Plain]),
  );
});

function render(status: string) {
  state.snap = { state: status, items: [], result: { results: [] } };
  return renderToStaticMarkup(
    createElement(GitUpdate, {
      open: true,
      path: "/fixture",
      connectionId: "fixture",
      onClose() {},
      onUpdated() {},
      onAuth() {},
    }),
  );
}

describe("Git update terminal actions", () => {
  it("offers a clear read-only next check after success", () => {
    const html = render("done");
    expect(html).toContain("检查新更新");
    expect(html).toContain("完成");
    expect(html).not.toContain("快进更新");
  });
  it("keeps running work backgroundable without starting a second update", () => {
    const html = render("running");
    expect(html).toContain("后台运行");
    expect(html).not.toContain("检查新更新");
    expect(html).not.toContain("快进更新");
  });
  it("requires reinspection for an unknown outcome", () => {
    const html = render("unknown");
    expect(html).toContain("检查新更新");
    expect(html).not.toContain("快进更新");
  });
});
