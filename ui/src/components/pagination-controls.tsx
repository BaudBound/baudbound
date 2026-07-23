import { ChevronLeft, ChevronRight } from "lucide-react";

import { Button } from "@/components/ui/button";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";

export function PaginationControls({
  onPageChange,
  onPageSizeChange,
  page,
  pageSize,
  selectedCount,
  total,
}: {
  onPageChange: (page: number) => void;
  onPageSizeChange: (pageSize: number) => void;
  page: number;
  pageSize: number;
  selectedCount?: number;
  total: number;
}) {
  const pageCount = Math.max(1, Math.ceil(total / pageSize));
  const first = total === 0 ? 0 : page * pageSize + 1;
  const last = Math.min(total, (page + 1) * pageSize);

  return (
    <div className="flex flex-wrap items-center justify-between gap-3 border-t border-border px-3 py-3">
      <span className="text-xs text-muted-foreground">
        {first} to {last} of {total}
        {selectedCount !== undefined ? ` | ${selectedCount} selected` : ""}
      </span>
      <div className="flex items-center gap-2">
        <Select
          onValueChange={(value) => onPageSizeChange(Number(value))}
          value={String(pageSize)}
        >
          <SelectTrigger
            aria-label="Rows per page"
            className="w-[124px] shrink-0 whitespace-nowrap [&_[data-slot=select-value]]:whitespace-nowrap"
          >
            <SelectValue />
          </SelectTrigger>
          <SelectContent>
            {[25, 50, 100, 200].map((size) => (
              <SelectItem key={size} value={String(size)}>
                {size} rows
              </SelectItem>
            ))}
          </SelectContent>
        </Select>
        <span className="min-w-20 text-center text-xs text-muted-foreground">
          Page {page + 1} of {pageCount}
        </span>
        <Button
          aria-label="Previous page"
          className="size-9 p-0"
          disabled={page === 0}
          onClick={() => onPageChange(page - 1)}
          variant="outline"
        >
          <ChevronLeft />
        </Button>
        <Button
          aria-label="Next page"
          className="size-9 p-0"
          disabled={page + 1 >= pageCount}
          onClick={() => onPageChange(page + 1)}
          variant="outline"
        >
          <ChevronRight />
        </Button>
      </div>
    </div>
  );
}
