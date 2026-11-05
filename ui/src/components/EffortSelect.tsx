import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "./ui/select";
import type { RemoteModel } from "@/lib/useClaudeInfo";
export const effortLabel = (value: string | undefined) =>
  (
    ({
      low: "低",
      medium: "中",
      high: "高",
      xhigh: "超高",
      max: "最高",
    }) as Record<string, string>
  )[value || ""] || "远端默认";
export function effortLevels(models: RemoteModel[], model: string) {
  return (
    models.find((m) => m.value === (model || "default") || m.resolved === model)
      ?.effort_levels || []
  );
}
export function EffortSelect({
  value,
  onChange,
  levels,
  disabled,
}: {
  value: string;
  onChange: (v: string) => void;
  levels: string[];
  disabled: boolean;
}) {
  return (
    <Select
      value={value || "remote_default"}
      onValueChange={onChange}
      disabled={disabled}
    >
      <SelectTrigger className="effort-select" aria-label="思考强度">
        <SelectValue>
          {value && value !== "remote_default"
            ? `思考 · ${effortLabel(value)}`
            : "思考 · 远端默认"}
        </SelectValue>
      </SelectTrigger>
      <SelectContent>
        <SelectItem value="remote_default">远端默认</SelectItem>
        {value && value !== "remote_default" && !levels.includes(value) && (
          <SelectItem value={value} disabled>
            {effortLabel(value)} · 当前模型未提供
          </SelectItem>
        )}
        {levels.map((level) => (
          <SelectItem key={level} value={level}>
            {effortLabel(level)}
          </SelectItem>
        ))}
      </SelectContent>
    </Select>
  );
}
