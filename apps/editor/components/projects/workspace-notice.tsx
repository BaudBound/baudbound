"use client";

import { X } from "lucide-react";
import { Button } from "@/components/ui/button";

export function WorkspaceNotice({ children, onDismiss }: { children: React.ReactNode; onDismiss: () => void }) {
	return (
		<div className="fixed right-4 bottom-9 z-40 max-w-md border border-amber-300/25 bg-[#17140d] p-3 text-sm text-amber-50 shadow-xl">
			<div className="flex items-start gap-3">
				<p className="min-w-0 leading-5">{children}</p>
				<Button
					type="button"
					variant="ghost"
					size="icon-xs"
					className="-mt-1 -mr-1 text-amber-100 hover:bg-amber-100/10 hover:text-white"
					aria-label="Dismiss notice"
					onClick={onDismiss}
				>
					<X />
				</Button>
			</div>
		</div>
	);
}
