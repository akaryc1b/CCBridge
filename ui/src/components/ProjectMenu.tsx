import { DropdownMenu as Menu } from "radix-ui";
import { MoreHorizontal, FolderMinus } from "lucide-react";
import { Button } from "./ui/button";
import type { RecordValue as Data } from "../lib/conversation";
export function ProjectMenu({
  project,
  disabled,
  onRemove,
}: {
  project: Data;
  disabled: boolean;
  onRemove: (project: Data) => void;
}) {
  return (
    <Menu.Root>
      <Menu.Trigger asChild>
        <Button
          variant="ghost"
          size="icon"
          aria-label={`管理项目 ${project.name}`}
          disabled={disabled}
          onClick={(e) => {
            e.preventDefault();
            e.stopPropagation();
          }}
        >
          <MoreHorizontal size={15} />
        </Button>
      </Menu.Trigger>
      <Menu.Portal>
        <Menu.Content className="context-menu" align="end">
          <Menu.Item onSelect={() => onRemove(project)}>
            <FolderMinus size={15} />
            从项目列表移除
          </Menu.Item>
        </Menu.Content>
      </Menu.Portal>
    </Menu.Root>
  );
}
