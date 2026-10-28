import { time } from "./format";
type Repo = Record<string, any>;
export function repositoryStatus(r: Repo): { label: string; tone: string } {
  if (r.error) return { label: "状态读取失败", tone: "error" };
  if (r.blockers?.length) return { label: "任务占用中", tone: "warning" };
  if (r.dirty) return { label: "有未提交修改", tone: "warning" };
  if (!r.upstream) return { label: "未设置上游", tone: "warning" };
  if (!r.tracking_known) return { label: "跟踪信息未获取", tone: "muted" };
  if (r.ahead > 0 && r.behind > 0)
    return { label: "分支已分叉", tone: "warning" };
  if (r.behind > 0)
    return { label: `可更新 ${r.behind} 个提交`, tone: "normal" };
  if (r.ahead > 0)
    return { label: `本地领先 ${r.ahead} 个提交`, tone: "muted" };
  return { label: "已是最新", tone: "success" };
}
export function remoteTime(r: Repo): string {
  return r.last_fetch
    ? `最近获取 ${time(r.last_fetch)} · 本地缓存`
    : "本地缓存 · 尚未获取";
}
