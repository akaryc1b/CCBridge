import { DropdownMenu as Menu } from "radix-ui";
import { Loader2, ListTodo } from "lucide-react";
import { Button } from "./ui/button";
export type BackgroundTask = {
  id: string;
  label: string;
  summary: string;
  running: boolean;
  open: () => void;
};
export function BackgroundTasks({ tasks }: { tasks: BackgroundTask[] }) {
  if (!tasks.length) return null;
  return (
    <Menu.Root>
      <Menu.Trigger asChild>
        <Button variant="outline" aria-label={`后台任务 ${tasks.length}`}>
          {tasks.some((t) => t.running) ? (
            <Loader2 size={15} className="spin" />
          ) : (
            <ListTodo size={15} />
          )}
          后台任务 · {tasks.length}
        </Button>
      </Menu.Trigger>
      <Menu.Portal>
        <Menu.Content
          className="context-menu background-task-menu"
          sideOffset={5}
          align="end"
        >
          {tasks.map((t) => (
            <Menu.Item key={t.id} onSelect={t.open}>
              <span>{t.label}</span>
              <span className="muted">{t.summary}</span>
            </Menu.Item>
          ))}
        </Menu.Content>
      </Menu.Portal>
    </Menu.Root>
  );
}
