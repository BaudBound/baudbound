import { MoreHorizontal } from "lucide-react";
import {
  type ComponentType,
  type MouseEvent,
  type SVGProps,
  useEffect,
  useLayoutEffect,
  useRef,
  useState,
} from "react";

import { Button } from "@/components/ui/button";
import { cn } from "@/lib/utils";

type MenuIcon = ComponentType<SVGProps<SVGSVGElement>>;

export type ActionMenuItem = {
  destructive?: boolean;
  disabled?: boolean;
  icon?: MenuIcon;
  id: string;
  label: string;
  onSelect: () => void;
};

export function ActionMenu({
  items,
  label = "More actions",
}: {
  items: ActionMenuItem[];
  label?: string;
}) {
  const [open, setOpen] = useState(false);
  const [menuPosition, setMenuPosition] = useState({ left: 0, top: 0 });
  const rootRef = useRef<HTMLDivElement>(null);
  const triggerRef = useRef<HTMLButtonElement>(null);

  useLayoutEffect(() => {
    if (!open || !triggerRef.current) return;
    const rect = triggerRef.current.getBoundingClientRect();
    const menuWidth = 160;
    setMenuPosition({
      left: Math.max(8, Math.min(rect.right - menuWidth, window.innerWidth - menuWidth - 8)),
      top: Math.min(rect.bottom + 4, window.innerHeight - 132),
    });
  }, [open]);

  useEffect(() => {
    if (!open) return;

    function onPointerDown(event: PointerEvent) {
      if (!rootRef.current?.contains(event.target as Node)) {
        setOpen(false);
      }
    }

    function onKeyDown(event: KeyboardEvent) {
      if (event.key === "Escape") {
        setOpen(false);
      }
    }

    document.addEventListener("pointerdown", onPointerDown);
    document.addEventListener("keydown", onKeyDown);
    return () => {
      document.removeEventListener("pointerdown", onPointerDown);
      document.removeEventListener("keydown", onKeyDown);
    };
  }, [open]);

  function selectItem(event: MouseEvent<HTMLButtonElement>, item: ActionMenuItem) {
    event.stopPropagation();
    if (item.disabled) return;
    setOpen(false);
    item.onSelect();
  }

  return (
    <div className="relative" ref={rootRef}>
      <Button
        aria-expanded={open}
        aria-haspopup="menu"
        aria-label={label}
        className="size-8 p-0"
        onClick={() => setOpen((current) => !current)}
        ref={triggerRef}
        size="sm"
        title={label}
        variant={open ? "secondary" : "outline"}
      >
        <MoreHorizontal />
      </Button>
      {open ? (
        <div
          className="fixed z-50 grid min-w-[160px] gap-1 rounded-md border border-border bg-card p-1 shadow-lg"
          role="menu"
          style={{ left: menuPosition.left, top: menuPosition.top }}
        >
          {items.map((item) => {
            const Icon = item.icon;
            return (
              <button
                className={cn(
                  "flex h-8 w-full items-center gap-2 rounded-sm px-2 text-left text-sm text-foreground outline-none hover:bg-muted focus-visible:bg-muted disabled:pointer-events-none disabled:opacity-45",
                  item.destructive && "text-destructive hover:bg-destructive/10",
                )}
                disabled={item.disabled}
                key={item.id}
                onClick={(event) => selectItem(event, item)}
                role="menuitem"
                type="button"
              >
                {Icon ? <Icon className="size-4 shrink-0" /> : null}
                <span>{item.label}</span>
              </button>
            );
          })}
        </div>
      ) : null}
    </div>
  );
}
