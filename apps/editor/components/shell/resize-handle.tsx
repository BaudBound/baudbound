import type { PointerEvent } from "react";
import { Button } from "@/components/ui/button";

type ResizeHandleProps = {
	axis: "horizontal" | "vertical";
	label: string;
	onPointerDown: (event: PointerEvent<HTMLButtonElement>) => void;
};

export function ResizeHandle({ axis, label, onPointerDown }: ResizeHandleProps) {
	const className =
		axis === "horizontal" ? "w-1 cursor-ew-resize hover:bg-baud-red/70" : "h-1 cursor-ns-resize hover:bg-baud-red/70";

	return (
		<Button
			type="button"
			aria-label={label}
			title={label}
			onPointerDown={onPointerDown}
			className={`shrink-0 rounded-none bg-baud-border/60 p-0 transition-colors ${className}`}
			size="none"
			variant="ghost"
		/>
	);
}
