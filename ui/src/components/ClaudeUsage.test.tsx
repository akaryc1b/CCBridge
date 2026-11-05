// @vitest-environment jsdom
import { afterEach, expect, it, vi } from "vitest";
import { cleanup, render, screen } from "@testing-library/react";
import { ClaudeUsage } from "./ClaudeInfo";
afterEach(cleanup);
it("only renders quota windows reported by the account", () => {
  render(
    <ClaudeUsage
      connected
      supported
      refresh={vi.fn()}
      data={{
        state: "reported",
        source: "claude_account",
        live: true,
        windows: { seven_day: { used_percentage: 32 } },
      }}
    />,
  );
  expect(screen.getByText("每周")).toBeTruthy();
  expect(screen.queryByText("5 小时")).toBeNull();
  expect(screen.getByText(/32% 已用/)).toBeTruthy();
});
it("does not fabricate zero usage or five-hour rows when live windows are absent", () => {
  render(
    <ClaudeUsage
      connected
      supported
      refresh={vi.fn()}
      data={{
        state: "unavailable",
        source: "claude_account",
        live: true,
        windows: {},
      }}
    />,
  );
  expect(screen.queryByText("5 小时")).toBeNull();
  expect(screen.queryByText(/0% 已用/)).toBeNull();
  expect(screen.getByText("Claude 未返回额度窗口")).toBeTruthy();
});
