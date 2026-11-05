import { expect, it } from "vitest";
import { workspaceProtected } from "../components/WorkspacePolicy";
it("keeps the global policy except for the selected subtree and preserves legacy defaults", () => {
  expect(
    workspaceProtected(
      { read_only: true, writable_workspaces: ["/home/u/projects/a"] },
      "/home/u/projects/a/repo",
    ),
  ).toBe(false);
  expect(
    workspaceProtected(
      { read_only: true, writable_workspaces: ["/home/u/projects/a"] },
      "/home/u/projects/ab",
    ),
  ).toBe(true);
  expect(workspaceProtected({ read_only: true }, "/home/u/projects/a")).toBe(
    true,
  );
  expect(workspaceProtected({ read_only: false }, "/home/u/projects/a")).toBe(
    false,
  );
});
