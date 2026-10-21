import { describe, it, expect } from "vitest";
import { containingProject, latestNavigation } from "./navigation";
describe("project navigation", () => {
  it("highlights the directory being browsed, not the independently selected task workspace", () => {
    const p = [
      { path: "/home/u/CCBridge" },
      { path: "/home/u/ehs" },
      { path: "/home/u/ehs/web" },
    ];
    expect(containingProject("/home/u/CCBridge/src", p)).toBe(
      "/home/u/CCBridge",
    );
    expect(containingProject("/home/u/ehs/web/src", p)).toBe("/home/u/ehs/web");
    expect(containingProject("/home/u/ehs-other", p)).toBe("");
  });
  it("latest click wins even if the previous network request completes later", async () => {
    const resolves: Record<string, (data: string) => void> = {};
    const committed: string[] = [];
    const busy: boolean[] = [];
    const browse = latestNavigation<string>(
      (p) =>
        new Promise((r) => {
          resolves[p] = r;
        }),
      (r) => committed.push(r),
      (b) => busy.push(b),
    );
    const a = browse("CCBridge"),
      b = browse("EHS");
    resolves.EHS("EHS");
    await b;
    resolves.CCBridge("CCBridge");
    await a;
    expect(committed).toEqual(["EHS"]);
    expect(busy).toEqual([true, true, false]);
  });
});
