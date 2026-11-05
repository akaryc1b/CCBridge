// @vitest-environment jsdom
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { Navigation } from "./Navigation";
import { Activity } from "lucide-react";
beforeEach(() => {
  const store = new Map<string, string>();
  vi.stubGlobal("localStorage", {
    getItem: (key: string) => store.get(key) ?? null,
    setItem: (key: string, value: string) => store.set(key, value),
    clear: () => store.clear(),
  });
});
afterEach(() => {
  cleanup();
  localStorage.clear();
});
it("keeps menu accessible when compact and remembers the preference", () => {
  const props = {
    pages: [
      ["chat", "会话", Activity],
      ["servers", "服务器", Activity],
    ] as const,
    page: "chat",
    onPage: () => {},
    connected: true,
    connecting: false,
    account: "qa",
    theme: "dark",
    onTheme: () => {},
    activeTasks: false,
  };
  const first = render(<Navigation {...props} />);
  fireEvent.click(screen.getByRole("button", { name: "收起主菜单" }));
  expect(screen.getByRole("button", { name: "服务器" })).toBeTruthy();
  expect(
    document.querySelector(".sidebar")?.classList.contains("compact"),
  ).toBe(true);
  first.unmount();
  render(<Navigation {...props} />);
  expect(screen.getByRole("button", { name: "展开主菜单" })).toBeTruthy();
  expect(
    screen.getByRole("button", { name: "暗色" }).getAttribute("aria-pressed"),
  ).toBe("true");
});
