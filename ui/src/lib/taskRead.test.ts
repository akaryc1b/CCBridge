import { describe, expect, it } from "vitest";
import { isTaskRead, markTasksRead, needsAction, taskAction } from "./taskRead";
describe("task center read state", () => {
  it("marks ended results read without changing or deleting tasks", () => {
    const tasks = [
      "done",
      "completed",
      "failed",
      "partial",
      "unknown",
      "interrupted",
      "cancelled",
    ].map((state) => ({ id: state, state, running: false }));
    const original = JSON.stringify(tasks);
    const read = markTasksRead(tasks, {});
    expect(tasks.every((t) => isTaskRead(t, read))).toBe(true);
    expect(JSON.stringify(tasks)).toBe(original);
  });
  it("does not dismiss running, queued or approval actions", () => {
    const tasks = ["running", "queued", "waiting_approval"].map((state) => ({
      id: state,
      state,
      running: state === "running",
    }));
    expect(markTasksRead(tasks, {})).toEqual({});
    expect(tasks.every(needsAction)).toBe(true);
  });
  it("renotifies changed outcomes and keeps read state serializable", () => {
    const task = {
      id: "one",
      state: "failed",
      running: false,
      finished: 1,
      error: "network",
    };
    const read = JSON.parse(JSON.stringify(markTasksRead([task], {})));
    expect(isTaskRead(task, read)).toBe(true);
    expect(isTaskRead({ ...task, finished: 2 }, read)).toBe(false);
    expect(isTaskRead({ ...task, state: "completed" }, read)).toBe(false);
    expect(isTaskRead({ ...task, error: "auth" }, read)).toBe(false);
    expect(isTaskRead(task, {})).toBe(false);
  });
  it("uses concrete actions instead of implementation terminology", () => {
    expect(taskAction("git_update")).toBe("查看更新结果");
    expect(taskAction("branch_fetch")).toBe("查看分支");
    expect(taskAction("install_runner")).toBe("查看服务端程序");
    expect(taskAction()).toBe("查看详情");
  });
});
