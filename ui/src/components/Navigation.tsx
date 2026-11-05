import { useState } from "react";
import {
  Sun,
  Moon,
  Monitor,
  PanelLeftClose,
  PanelLeftOpen,
  type LucideIcon,
} from "lucide-react";
import { AppIcon } from "./AppIcon";
import { Button } from "./ui/button";
export function Navigation({
  pages,
  page,
  onPage,
  connected,
  connecting,
  account,
  theme,
  onTheme,
  activeTasks,
}: {
  pages: readonly (readonly [string, string, LucideIcon])[];
  page: string;
  onPage: (page: string) => void;
  connected: boolean;
  connecting: boolean;
  account: string;
  theme: string;
  onTheme: (theme: string) => void;
  activeTasks: boolean;
}) {
  const [compact, setCompact] = useState(
    () => localStorage.getItem("ccbridge-nav-compact") === "true",
  );
  return (
    <aside
      className={`sidebar ${compact ? "compact" : ""}`}
      aria-label="主菜单"
    >
      <div className="window-drag-space" data-tauri-drag-region />
      <div className="brand" data-tauri-drag-region>
        <AppIcon />
        <span className="nav-label">CCBridge</span>
      </div>
      <nav>
        {pages.map(([id, label, Icon]) => (
          <button
            key={id}
            aria-label={label}
            title={compact ? label : undefined}
            aria-current={page === id ? "page" : undefined}
            className={page === id ? "selected" : ""}
            onClick={() => onPage(id)}
          >
            <Icon size={17} />
            <span className="nav-label">{label}</span>
            {id === "chat" && activeTasks && <span className="nav-dot" />}
          </button>
        ))}
      </nav>
      <div className="sidebar-bottom">
        <div
          className="connection-mini"
          title={connecting ? "连接中…" : connected ? account : "未连接"}
        >
          <span className={connected ? "dot live" : "dot"} />
          <span className="nav-label">
            {connecting ? "连接中…" : connected ? account : "未连接"}
          </span>
        </div>
        <div className="theme-switch">
          {(
            [
              ["light", Sun, "亮色"],
              ["dark", Moon, "暗色"],
              ["system", Monitor, "跟随系统"],
            ] as const
          ).map(([value, Icon, label]) => (
            <Button
              key={value}
              variant="ghost"
              size="icon"
              aria-label={label}
              title={label}
              aria-pressed={theme === value}
              className={theme === value ? "chosen" : ""}
              onClick={() => onTheme(value)}
            >
              <Icon size={14} />
            </Button>
          ))}
        </div>
        <Button
          variant="ghost"
          size="sm"
          className="nav-collapse"
          aria-label={compact ? "展开主菜单" : "收起主菜单"}
          title={compact ? "展开主菜单" : "收起主菜单"}
          onClick={() =>
            setCompact((previous) => {
              localStorage.setItem("ccbridge-nav-compact", String(!previous));
              return !previous;
            })
          }
        >
          {compact ? <PanelLeftOpen size={16} /> : <PanelLeftClose size={16} />}
          <span className="nav-label">收起菜单</span>
        </Button>
      </div>
    </aside>
  );
}
