import { AlertDialog } from "radix-ui";
import { Button } from "../ui/button";
import type { ReactNode } from "react";
export function Confirm({
  open,
  title,
  children,
  action,
  onCancel,
  onConfirm,
  busy = false,
  destructive = false,
}: {
  open: boolean;
  title: string;
  children: ReactNode;
  action: string;
  onCancel: () => void;
  onConfirm: () => void;
  busy?: boolean;
  destructive?: boolean;
}) {
  return (
    <AlertDialog.Root
      open={open}
      onOpenChange={(v) => {
        if (!v && !busy) onCancel();
      }}
    >
      <AlertDialog.Portal>
        <AlertDialog.Overlay className="confirm-overlay" />
        <AlertDialog.Content className="confirm-dialog">
          <AlertDialog.Title>{title}</AlertDialog.Title>
          <AlertDialog.Description asChild>
            <div className="confirm-body">{children}</div>
          </AlertDialog.Description>
          <div className="confirm-actions">
            <Button variant="outline" disabled={busy} onClick={onCancel}>
              取消
            </Button>
            <Button
              variant={destructive ? "destructive" : "default"}
              disabled={busy}
              onClick={onConfirm}
            >
              {busy ? "处理中…" : action}
            </Button>
          </div>
        </AlertDialog.Content>
      </AlertDialog.Portal>
    </AlertDialog.Root>
  );
}
