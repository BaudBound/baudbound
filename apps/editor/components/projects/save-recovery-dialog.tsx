"use client";

import { Download, RefreshCw } from "lucide-react";
import { Button } from "@/components/ui/button";
import {
	Dialog,
	DialogContent,
	DialogDescription,
	DialogFooter,
	DialogHeader,
	DialogTitle,
} from "@/components/ui/dialog";
import type { SaveFailure } from "@/data/storage/save-failure";

export function SaveRecoveryDialog({
	failure,
	saving,
	onClose,
	onExport,
	onRetry,
}: {
	failure: SaveFailure | null;
	saving: boolean;
	onClose: () => void;
	onExport: () => void;
	onRetry: () => void;
}) {
	return (
		<Dialog open={failure !== null} onOpenChange={(open) => !open && onClose()}>
			<DialogContent className="sm:max-w-lg">
				<DialogHeader>
					<DialogTitle>{failure?.title ?? "Project was not saved"}</DialogTitle>
					<DialogDescription>{failure?.description}</DialogDescription>
				</DialogHeader>
				<DialogFooter>
					<Button type="button" variant="toolbar" disabled={saving} onClick={onClose}>
						Close
					</Button>
					<Button type="button" variant="toolbar" disabled={saving} onClick={onExport}>
						<Download /> Export current project
					</Button>
					{failure?.retryable && (
						<Button type="button" variant="primary" disabled={saving} onClick={onRetry}>
							<RefreshCw /> {saving ? "Saving..." : "Retry save"}
						</Button>
					)}
				</DialogFooter>
			</DialogContent>
		</Dialog>
	);
}
