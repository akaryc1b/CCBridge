/// <reference types="vite/client" />
import { it, expect } from "vitest";
import source from "../App.tsx?raw";
import { shouldRefreshTaskList } from "./taskQueue";
it("discovers external tasks with no active transcript cache and refreshes their completion", () => {
  expect(shouldRefreshTaskList([], [{ state: "running" }])).toBe(true);
  expect(shouldRefreshTaskList([{ state: "running" }], [])).toBe(true);
  expect(shouldRefreshTaskList([], [])).toBe(false);
});
it("polls recent tasks before skipping hidden-window environment reads", () => {
  const start = source.indexOf("const timer = setInterval(() => {");
  const poll = source.slice(start, source.indexOf("}, 5000);", start));
  expect(poll.indexOf("safe(refreshTasks())")).toBeGreaterThanOrEqual(0);
  expect(poll.indexOf("safe(refreshTasks())")).toBeLessThan(
    poll.indexOf("if (document.hidden) return"),
  );
});
