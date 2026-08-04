import { useRef } from "react";

import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";

export function DetailDialog({
  children,
  description,
  footer,
  onOpenChange,
  open,
  title,
}: {
  children: React.ReactNode;
  description: string;
  footer?: React.ReactNode;
  onOpenChange: (open: boolean) => void;
  open: boolean;
  title: string;
}) {
  const contentRef = useRef<HTMLDivElement>(null);

  return (
    <Dialog onOpenChange={onOpenChange} open={open}>
      <DialogContent
        className={`max-h-[calc(100vh-2rem)] w-[min(calc(100vw-2rem),1200px)] overflow-hidden ${
          footer ? "grid-rows-[auto_minmax(0,1fr)_auto]" : "grid-rows-[auto_minmax(0,1fr)]"
        }`}
        onOpenAutoFocus={(event) => {
          event.preventDefault();
          contentRef.current?.focus();
        }}
        ref={contentRef}
      >
        <DialogHeader>
          <DialogTitle>{title}</DialogTitle>
          <DialogDescription className="break-all">
            {description}
          </DialogDescription>
        </DialogHeader>
        <div className="min-h-0 overflow-y-auto pr-1">{children}</div>
        {footer ? <div className="flex flex-wrap justify-end gap-2 border-t border-border pt-4">{footer}</div> : null}
      </DialogContent>
    </Dialog>
  );
}
