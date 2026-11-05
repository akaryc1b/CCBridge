import { expect, it } from "vitest";
import { sessionReadTask, sessionStatus } from "./sessionStatus";
import { markTasksRead, isTaskRead } from "./taskRead";
it("separates state from read and does not pre-read future turns", () => {
  const t = { task_id: "one", state: "running", updated_at: 4 };
  const read = markTasksRead([sessionReadTask(t)], {});
  expect(read).toEqual({});
  const done = { ...t, state: "completed", updated_at: 5 };
  expect(sessionStatus(done, true)).toBe("已完成");
  expect(isTaskRead(sessionReadTask(done), read)).toBe(false);
  const seen = markTasksRead([sessionReadTask(done)], read);
  expect(isTaskRead(sessionReadTask(done), seen)).toBe(true);
  expect(isTaskRead(sessionReadTask({ ...done, task_id: "two" }), seen)).toBe(
    false,
  );
  expect(sessionStatus(t, false)).toBe("状态待确认");
  expect(sessionStatus({ ...t, state: "waiting_approval" }, true)).toBe(
    "待审批",
  );
});
