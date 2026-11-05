// @vitest-environment jsdom
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import { act, cleanup, renderHook, waitFor } from "@testing-library/react";
import { useTaskRead } from "./useTaskRead";
import { isTaskRead } from "./taskRead";
beforeEach(() => {
  const values = new Map<string, string>();
  vi.stubGlobal("localStorage", {
    getItem: (k: string) => values.get(k) ?? null,
    setItem: (k: string, v: string) => values.set(k, v),
  });
});
afterEach(() => {
  cleanup();
  vi.unstubAllGlobals();
});
it("shares all-read with sessions, keeps accounts isolated and reminds a new turn", async () => {
  const a = renderHook(() => useTaskRead("a"));
  const center = renderHook(() => useTaskRead("a"));
  const other = renderHook(() => useTaskRead("b"));
  const task = { id: "t", state: "completed", running: false, finished: 100 };
  act(() => center.result.current.markRead([task]));
  await waitFor(() =>
    expect(isTaskRead(task, a.result.current.read)).toBe(true),
  );
  expect(isTaskRead(task, other.result.current.read)).toBe(false);
  expect(isTaskRead({ ...task, id: "new" }, a.result.current.read)).toBe(false);
  const live = { ...task, id: "live", state: "running", running: true };
  act(() => center.result.current.markRead([live]));
  expect(
    isTaskRead(
      { ...live, state: "completed", running: false },
      a.result.current.read,
    ),
  ).toBe(false);
});
