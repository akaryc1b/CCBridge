import { describe, it, expect } from "vitest";
import { queueLabel, queueBlockers } from "./taskQueue";
describe("task queue copy", () => {
  it("shows blockers by title, not UUID", () => {
    const t = {
      state: "queued",
      queue: { blocked_by: [{ id: "uuid", title: "页面加载态整改" }] },
    };
    expect(queueLabel(t)).toBe("等待「页面加载态整改」完成");
  });
  it("does not invent a blocker while launching", () => {
    expect(
      queueLabel({
        state: "queued",
        phase: "waiting",
        queue: { blocked_by: [] },
      }),
    ).toBe("等待执行");
    expect(queueLabel({ state: "queued", phase: "executing" })).toBe(
      "正在启动",
    );
  });
  it("keeps unknown ownership visible", () =>
    expect(queueLabel({ ownership_unknown: true })).toBe(
      "状态未知 · 目录仍被占用",
    ));
  it("filters invalid blocker IDs", () =>
    expect(
      queueBlockers({ queue: { blocked_by: [{ title: "操作" }] } })[0].id,
    ).toBeUndefined());
});
import { shouldRefreshQueued } from "./taskQueue";
it("refreshes selected queued task when it leaves the active list", () => {
  expect(shouldRefreshQueued({ task_id: "a", state: "queued" }, [])).toBe(true);
  expect(
    shouldRefreshQueued({ task_id: "a", state: "queued" }, [
      { task_id: "a", state: "queued" },
    ]),
  ).toBe(false);
  expect(
    shouldRefreshQueued({ task_id: "a", state: "queued" }, [
      { task_id: "a", state: "running" },
    ]),
  ).toBe(true);
});

it("explains a policy-paused write without inventing a running blocker", () => {
  expect(queueLabel({ state: "queued", blocked_reason: "policy" })).toBe(
    "已暂停 · 禁止修改已开启",
  );
});

import { taskCenterRecord } from "./taskQueue";
it("does not let cached transcript status revive a completed task", () => {
  const row = taskCenterRecord(
    { task_id: "a", state: "completed", ownership_unknown: false },
    {
      task_id: "a",
      state: "running",
      result: { result: "kept" },
      ownership_unknown: true,
    },
  );
  expect(row.state).toBe("completed");
  expect(row.ownership_unknown).toBe(false);
  expect(row.result.result).toBe("kept");
});
