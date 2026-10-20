import { describe, it, expect } from "vitest";
import {
  normalizeTask,
  conversationTurns,
  latestSessions,
  displayModel,
} from "./conversation";
describe("conversation state", () => {
  it("keeps legacy list IDs and titles usable", () => {
    expect(normalizeTask({ id: "a", title: "旧任务" })).toMatchObject({
      task_id: "a",
      prompt: "旧任务",
    });
  });
  it("shows every linked turn in order without rewriting models", () => {
    const a = {
      task_id: "a",
      model_requested: "sonnet",
      model_resolved: "claude-sonnet-4-6",
    };
    const b = {
      task_id: "b",
      reply_to: "a",
      model_requested: "opus",
      model_resolved: "claude-opus-5-5",
    };
    const turns = conversationTurns(b, { a, b });
    expect(turns).toEqual([a, b]);
    expect(displayModel(turns[0]).requested).toBe("sonnet");
    expect(displayModel(turns[1]).actual).toBe("claude-opus-5-5");
  });
  it("does not infer actual model from requested name or child usage", () => {
    expect(
      displayModel({ model: "opus", result: { modelUsage: { other: {} } } })
        .actual,
    ).toBe("实际模型尚未上报");
  });
  it("terminates a corrupted ancestry cycle", () => {
    const a = { task_id: "a", reply_to: "b" },
      b = { task_id: "b", reply_to: "a" };
    expect(conversationTurns(a, { a, b })).toHaveLength(2);
  });
  it("one sidebar entry per session, newest turn wins", () => {
    expect(
      latestSessions([
        { task_id: "a", session_id: "s", created_at: 1 },
        { task_id: "b", session_id: "s", created_at: 2 },
        { task_id: "c", created_at: 3 },
      ]).map((t) => t.task_id),
    ).toEqual(["c", "b"]);
  });
});
