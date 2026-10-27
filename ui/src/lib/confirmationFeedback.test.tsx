import { it, expect, vi } from "vitest";
import { createElement } from "react";
import { renderToStaticMarkup } from "react-dom/server";
vi.mock("radix-ui", () => {
  const Plain = ({ children }: any) => children;
  return {
    AlertDialog: Object.fromEntries(
      ["Root", "Portal", "Overlay", "Content", "Title", "Description"].map(
        (k) => [k, Plain],
      ),
    ),
  };
});
import { Confirm } from "../components/common/Confirm";
it("shows a failed confirmation within the open dialog", () => {
  const html = renderToStaticMarkup(
    createElement(Confirm, {
      open: true,
      title: "释放占用？",
      action: "确认",
      children: "确认原进程已停止",
      error: "仍检测到原任务进程组",
      onCancel() {},
      onConfirm() {},
    }),
  );
  expect(html).toContain("仍检测到原任务进程组");
  expect(html).toContain('role="alert"');
});
