import { expect, it } from "vitest";
import { streamingText, speedLabel } from "./streaming";
it("shows partial output before final assistant and replaces it without duplication", () => {
  const events = [
    {
      type: "stream_event",
      event: { type: "message_start", message: { id: "a" } },
    },
    {
      type: "stream_event",
      event: {
        type: "content_block_delta",
        index: 0,
        delta: { type: "text_delta", text: "中文" },
      },
    },
  ];
  expect(streamingText(events)).toEqual(["中文"]);
  expect(
    streamingText([
      ...events,
      {
        type: "assistant",
        message: { id: "a", content: [{ type: "text", text: "中文完整" }] },
      },
    ]),
  ).toEqual(["中文完整"]);
});
it("does not present stale, paused or estimated metrics as actual speed", () => {
  const task = {
    state: "running",
    telemetry: {
      observed_at_ms: 1000,
      phase: "generating",
      tokens_per_second: 42,
      speed_source: "text_heuristic",
    },
  };
  expect(speedLabel(task, true, 1100)).toContain("实时估算");
  expect(speedLabel(task, false, 1100)).toContain("速度暂停");
  expect(speedLabel(task, true, 8000)).toBe("等待输出…");
  expect(speedLabel({ ...task, state: "completed" }, true, 1100)).toBe("");
});
it("uses remote age instead of comparing client and server wall clocks", () => {
  const task = {
    state: "running",
    telemetry_received_at: 100000,
    telemetry: {
      observed_at_ms: 1,
      age_ms: 20,
      phase: "generating",
      tokens_per_second: 12,
      speed_source: "text_heuristic",
    },
  };
  expect(speedLabel(task, true, 100100)).toContain("12.0");
});
it("hides subagent output and does not double count the final main response", () => {
  expect(
    streamingText([
      {
        type: "assistant",
        parent_tool_use_id: "sub",
        message: {
          id: "child",
          content: [{ type: "text", text: "private child" }],
        },
      },
      {
        type: "assistant",
        message: {
          id: "main",
          content: [{ type: "text", text: "main result" }],
        },
      },
    ]),
  ).toEqual(["main result"]);
});
it("replaces the @ trigger without duplicating it and quotes paths with spaces", async () => {
  const { referenceDraft } = await import("./streaming");
  expect(referenceDraft("检查 @", "/fixture/a.md")).toBe(
    "检查 @/fixture/a.md ",
  );
  expect(referenceDraft("", "/fixture/a b.md")).toBe(
    "@/fixture/a b.md ".replace("@/fixture/a b.md", '@"/fixture/a b.md"'),
  );
});
it("summarizes actual tool calls without counting streamed chunks as steps", async () => {
  const { toolSummary } = await import("./streaming");
  const e = {
    type: "assistant",
    message: {
      id: "m",
      content: [{ type: "tool_use", id: "tool-a", name: "Read" }],
    },
  };
  expect(
    toolSummary([
      e,
      e,
      { type: "stream_event", event: { type: "content_block_delta" } },
    ]),
  ).toBe("读取 1 个文件");
});
