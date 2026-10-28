import { describe, expect, it } from "vitest";
import { repositoryStatus, remoteTime } from "./repositoryOverview";
describe("repository overview status", () => {
  it("never presents missing tracking info as latest", () => {
    expect(repositoryStatus({ upstream: "", dirty: false }).label).toBe(
      "未设置上游",
    );
    expect(
      repositoryStatus({ upstream: "origin/main", tracking_known: false })
        .label,
    ).toBe("跟踪信息未获取");
  });
  it("distinguishes dirty, divergent, behind and ahead", () => {
    const base = {
      upstream: "origin/main",
      tracking_known: true,
      ahead: 0,
      behind: 0,
    };
    expect(repositoryStatus({ ...base, dirty: true }).label).toBe(
      "有未提交修改",
    );
    expect(repositoryStatus({ ...base, ahead: 2, behind: 3 }).label).toBe(
      "分支已分叉",
    );
    expect(repositoryStatus({ ...base, behind: 3 }).label).toBe(
      "可更新 3 个提交",
    );
    expect(repositoryStatus({ ...base, ahead: 2 }).label).toBe(
      "本地领先 2 个提交",
    );
    expect(repositoryStatus(base).label).toBe("已是最新");
  });
  it("shows task blockers before allowing update and keeps cache source visible", () => {
    expect(
      repositoryStatus({
        blockers: [{ title: "分析任务" }],
        upstream: "origin/main",
        tracking_known: true,
      }).label,
    ).toBe("任务占用中");
    expect(remoteTime({})).toBe("本地缓存 · 尚未获取");
    expect(remoteTime({ last_fetch: 1700000000 })).toContain("最近获取");
  });
});
