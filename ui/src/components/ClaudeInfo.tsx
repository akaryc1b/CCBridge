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
  const observed = Object.values(windows)
    .map((w: any) => w.observed_at)
    .filter(Boolean) as number[];
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
      {supported &&
        (["five_hour", "seven_day"] as const).map((key) => {
          const w = windows[key];
          const expired = w && w.resets_at <= Date.now() / 1000;
          return (
            <StatusRow
              key={key}
              label={key === "five_hour" ? "5 小时" : "每周"}
            >
              {!supported ? (
                <span className="muted">更新 Runner 后获取</span>
              ) : !w ? (
                <span className="muted">
                  {data.loading
                    ? "读取中…"
                    : data.error
                      ? "读取失败"
                      : "未上报"}
                </span>
              ) : expired ? (
                <span className="muted">已到恢复时间 · 等待上报</span>
              ) : (
                <div className="quota-value">
                  <span>
                    {Math.round(w.used_percentage)}% 已用 · {time(w.resets_at)}{" "}
                    恢复
                  </span>
                  <Progress value={w.used_percentage} />
                </div>
              )}
            </StatusRow>
          );
        })}
      {supported && observed.length > 0 && (
        <p className="hint">最近上报 · {time(Math.min(...observed))}</p>
      )}
      {data.error && <InlineAlert>{data.error}</InlineAlert>}
    </section>
  );
}
