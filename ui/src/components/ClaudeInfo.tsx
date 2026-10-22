import { RefreshCw } from "lucide-react";
import { Button } from "./ui/button";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "./ui/select";
import { InlineAlert, StatusRow } from "./common/Feedback";
import { Progress } from "./ui/progress";
import { time } from "@/lib/format";
import type { RemoteModel } from "@/lib/useClaudeInfo";
export function modelLabel(value: string, models: RemoteModel[]) {
  const row = models.find((m) => m.value === (value || "default"));
  if (!row && (!value || value === "default")) return "跟随远端默认";
  if (!row) return value ? `${value}（不在远端列表）` : "跟随远端默认";
  if (row.value === "default")
    return row.resolved ? `跟随远端默认 · ${row.resolved}` : "跟随远端默认";
  return row.resolved ? `${row.value} · ${row.resolved}` : row.label;
}
export function RemoteModelSelect({
  value,
  onChange,
  models,
  loading,
  disabled,
  inheritLabel,
  label = "模型",
}: {
  value: string;
  onChange: (v: string) => void;
  models: RemoteModel[];
  loading: boolean;
  disabled?: boolean;
  inheritLabel?: string;
  label?: string;
}) {
  const known =
    models.some((m) => m.value === (value || "default")) ||
    (value === "__inherit" && !!inheritLabel);
  return (
    <Select
      value={value || "default"}
      onValueChange={onChange}
      disabled={disabled || loading || !models.length}
    >
      <SelectTrigger className="remote-model-select" aria-label={label}>
        <SelectValue>
          {loading
            ? "获取模型…"
            : !models.length
              ? "未获取模型列表"
              : value === "__inherit"
                ? inheritLabel
                : modelLabel(value, models)}
        </SelectValue>
      </SelectTrigger>
      <SelectContent position="popper" align="start">
        {inheritLabel && (
          <SelectItem value="__inherit">{inheritLabel}</SelectItem>
        )}
        {!known && value && (
          <SelectItem value={value} disabled>
            {modelLabel(value, models)}
          </SelectItem>
        )}
        {models.map((m) => (
          <SelectItem key={m.value} value={m.value}>
            {modelLabel(m.value, models)}
          </SelectItem>
        ))}
      </SelectContent>
    </Select>
  );
}
export function quotaReset(w: Record<string, any>): number | null {
  if (
    typeof w.resets_at === "number" &&
    Number.isFinite(w.resets_at) &&
    w.resets_at > 0
  )
    return w.resets_at;
  if (typeof w.resets_at_iso === "string") {
    const ms = Date.parse(w.resets_at_iso);
    if (Number.isFinite(ms)) return ms / 1000;
  }
  return null;
}
export function ClaudeUsage({
  connected,
  data,
  supported,
  refresh,
}: {
  connected: boolean;
  data: Record<string, any>;
  supported: boolean;
  refresh: () => void;
}) {
  const windows = data.windows || {};
  return (
    <section className="claude-usage">
      <div className="section-heading">
        <h3>Claude 用量</h3>
        <Button
          variant="ghost"
          size="icon"
          aria-label="读取Claude用量"
          disabled={!supported || data.loading}
          onClick={refresh}
        >
          <RefreshCw size={14} className={data.loading ? "spin" : ""} />
        </Button>
      </div>
      {!supported && (
        <p className="hint">{connected ? "更新 Runner 后获取" : "未连接"}</p>
      )}
      {supported && data.state === "not_applicable" && (
        <p className="hint">当前登录方式不提供订阅额度。</p>
      )}
      {supported &&
        data.state !== "not_applicable" &&
        (
          [
            "five_hour",
            "seven_day",
            ...(
              [
                "seven_day_opus",
                "seven_day_sonnet",
                "seven_day_oauth_apps",
              ] as const
            ).filter((key) => windows[key]),
          ] as const
        ).map((key) => {
          const w = windows[key];
          const reset = w ? quotaReset(w) : null;
          const expired = reset !== null && reset <= Date.now() / 1000;
          return (
            <StatusRow
              key={key}
              label={
                key === "five_hour"
                  ? "5 小时"
                  : key === "seven_day_opus"
                    ? "每周 · Opus"
                    : key === "seven_day_oauth_apps"
                      ? "每周 · 第三方应用"
                      : key === "seven_day_sonnet"
                        ? "每周 · Sonnet"
                        : "每周"
              }
            >
              {!supported ? (
                <span className="muted">更新 Runner 后获取</span>
              ) : !w ? (
                <span className="muted">
                  {data.loading
                    ? "读取中…"
                    : data.error
                      ? "读取失败"
                      : data.state === "unrecognized"
                        ? "上报格式不支持"
                        : data.state === "unavailable"
                          ? "Claude 未返回额度"
                          : "暂无额度数据"}
                </span>
              ) : expired ? (
                <span className="muted">已到恢复时间 · 等待上报</span>
              ) : (
                <div className="quota-value">
                  <span>
                    {Math.round(w.used_percentage)}% 已用
                    {reset !== null ? ` · ${time(reset)} 恢复` : ""}
                  </span>
                  <Progress value={w.used_percentage} />
                  {data.live === false && w.observed_at && (
                    <small className="muted">
                      上报于 {time(w.observed_at)}
                    </small>
                  )}
                </div>
              )}
            </StatusRow>
          );
        })}
      {supported && data.state !== "not_applicable" && (
        <p className="hint">
          {data.account_state === "not_applicable"
            ? "Claude 未提供账户额度，当前为会话历史。"
            : data.account_state === "unavailable"
              ? "账户查询暂无数据，当前为会话历史。"
              : data.source === "claude_account"
                ? `Claude 账户 · ${time(data.checked_at)} 更新`
                : data.state === "unrecognized"
                  ? "额度事件缺少可识别的数值。"
                  : Object.keys(windows).length
                    ? "来源：会话历史，非账户实时查询。"
                    : "近期 CCBridge 会话未上报额度。"}
        </p>
      )}
      {data.live_error && (
        <InlineAlert kind="warning">
          {data.live_error} 当前显示会话历史。
        </InlineAlert>
      )}
      {data.error && <InlineAlert>{data.error}</InlineAlert>}
    </section>
  );
}
