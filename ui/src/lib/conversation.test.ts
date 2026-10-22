import { describe, it, expect } from "vitest";
import {
  normalizeTask,
  conversationTurns,
  latestSessions,
  displayModel,
  eventText,
  eventLabel,
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
    ).toBe("未上报");
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

describe("readable events", () => {
  it("shows main assistant text, not tool inputs or child text", () => {
    expect(
      eventText({
        type: "assistant",
        message: {
          content: [
            { type: "text", text: "正在检查" },
            { type: "tool_use", name: "Read" },
          ],
        },
      }),
    ).toBe("正在检查");
    expect(
      eventText({
        type: "assistant",
        parent_tool_use_id: "child",
        message: { content: [{ type: "text", text: "child" }] },
      }),
    ).toBe("");
    expect(
      eventLabel({
        type: "assistant",
        message: { content: [{ type: "tool_use", name: "Read" }] },
      }),
    ).toBe("读取文件");
  });
});

import { groupSessions, nextModelLabel } from "./conversation";
describe("project sessions and visible model", () => {
  it("groups exact and child workspaces without prefix collisions, with the longest registered path", () => {
    const projects = [
      { name: "EHS", path: "/home/u/ehs" },
      { name: "前端", path: "/home/u/ehs/web" },
    ];
    const groups = groupSessions(
      [
        {
          task_id: "1",
          session_id: "s",
          created_at: 1,
          workspace: "/home/u/ehs/api",
        },
        {
          task_id: "2",
          session_id: "s",
          created_at: 2,
          workspace: "/home/u/ehs/api",
        },
        { task_id: "3", created_at: 3, workspace: "/home/u/ehs/web/src" },
        { task_id: "4", created_at: 4, workspace: "/home/u/ehs-other" },
        {
          task_id: "5",
          created_at: 1,
          updated_at: 8,
          workspace: "/home/u/ehs",
        },
      ],
      projects,
    );
    expect(groups[0].name).toBe("EHS");
    expect(groups[0].sessions.map((t) => t.task_id)).toEqual(["5", "2"]);
    expect(groups.find((g) => g.name === "前端")?.sessions[0].task_id).toBe(
      "3",
    );
    expect(groups.find((g) => g.path === "/home/u/ehs-other")?.registered).toBe(
      false,
    );
  });
  it("shows configured model directly, remote default remains explicitly unconfirmed", () => {
    expect(
      nextModelLabel({ model_requested: "claude-opus-5-5" }, "sonnet"),
    ).toBe("claude-opus-5-5");
    expect(nextModelLabel(undefined, "sonnet")).toBe("sonnet");
    expect(nextModelLabel({}, "sonnet")).toBe("远端默认");
    expect(
      nextModelLabel({ model_resolved: "claude-opus-5-5" }, "sonnet"),
    ).toBe("claude-opus-5-5（远端默认）");
  });
});

it('distinguishes waiting from unreported models without guessing',()=>{expect(displayModel({state:'running',model:'opus'}).actual).toBe('等待上报');expect(displayModel({state:'completed',model:'opus'}).actual).toBe('未上报')});
