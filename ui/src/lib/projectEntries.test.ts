import { describe, it, expect } from "vitest";
import {
  projectAtPath,
  projectSessionGroups,
  directoryName,
  parentDirectory,
} from "./projectEntries";
const projects = [
  { path: "/home/u/A", name: "A" },
  { path: "/home/u/B", name: "B" },
];
describe("project entry contract", () => {
  it("keeps empty registered projects visible in recent view", () => {
    expect(
      projectSessionGroups([], projects, "", false, "").map((g) => [
        g.name,
        g.sessions.length,
      ]),
    ).toEqual([
      ["A", 0],
      ["B", 0],
    ]);
  });
  it("does not inject empty groups into search or archive", () => {
    expect(projectSessionGroups([], projects, "find", false, "")).toEqual([]);
    expect(projectSessionGroups([], projects, "", true, "")).toEqual([]);
  });
  it("keeps project filter and nested repo grouping", () => {
    expect(
      projectSessionGroups([], projects, "", false, "/home/u/B").map(
        (g) => g.name,
      ),
    ).toEqual(["B"]);
    const r = projectSessionGroups(
      [
        {
          task_id: "t",
          session_id: "s",
          workspace: "/home/u/A/repo",
          created_at: 2,
        },
      ],
      projects,
      "",
      false,
      "",
    );
    expect(r.map((g) => g.name)).toEqual(["A", "B"]);
    expect(r[0].sessions[0].workspace).toBe("/home/u/A/repo");
  });
  it("does not confuse nested directories with exact registration", () => {
    expect(projectAtPath("/home/u/A/", projects)?.name).toBe("A");
    expect(projectAtPath("/home/u/A/repo", projects)).toBeUndefined();
  });
  it("names plain folders and stops parent navigation at home", () => {
    expect(directoryName("/home/u/工具/")).toBe("工具");
    expect(parentDirectory("/home/u", "/home/u")).toBe("/home/u");
  });
});
