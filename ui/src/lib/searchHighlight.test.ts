import { describe, it, expect } from "vitest";
import { renderToStaticMarkup } from "react-dom/server";
import { searchHighlight } from "./searchHighlight";
describe("search highlights", () => {
  it("highlights Chinese and case-insensitive English", () => {
    expect(renderToStaticMarkup(searchHighlight("检查权限流程", "权限"))).toBe(
      "检查<mark>权限</mark>流程",
    );
    expect(renderToStaticMarkup(searchHighlight("Hello HELLO", "hello"))).toBe(
      "<mark>Hello</mark> <mark>HELLO</mark>",
    );
  });
  it("treats regex and HTML as literal text", () => {
    expect(renderToStaticMarkup(searchHighlight("a+b then aab", "a+b"))).toBe(
      "<mark>a+b</mark> then aab",
    );
    expect(
      renderToStaticMarkup(
        searchHighlight("<script>alert(1)</script>", "script"),
      ),
    ).not.toContain("<script>");
  });
});
