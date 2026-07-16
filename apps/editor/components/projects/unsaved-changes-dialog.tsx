"use client";

import { Button } from "@/components/ui/button";
import {
	Dialog,
	DialogContent,
	DialogDescription,
	DialogFooter,
	DialogHeader,
	DialogTitle,
} from "@/components/ui/dialog";

export function UnsavedChangesDialog({
	open,
	saving,
	onCancel,
	onDiscard,
	onSave,
}: {
	open: boolean;
	saving: boolean;
	onCancel: () => void;
	onDiscard: () => void;
	onSave: () => void;
}) {
	return (
		<Dialog open={open} onOpenChange={(nextOpen) => !nextOpen && onCancel()}>
			<DialogContent className="sm:max-w-md">
				<DialogHeader>
					<DialogTitle>Save changes?</DialogTitle>
					<DialogDescription>Your project has changes that have not been saved to this browser.</DialogDescription>
				</DialogHeader>
				<DialogFooter>
					<Button type="button" variant="toolbar" disabled={saving} onClick={onCancel}>
						Cancel
					</Button>
					<Button type="button" variant="destructive" disabled={saving} onClick={onDiscard}>
						Discard
					</Button>
					<Button type="button" variant="primary" disabled={saving} onClick={onSave}>
						{saving ? "Saving..." : "Save and return"}
					</Button>
				</DialogFooter>
			</DialogContent>
		</Dialog>
	);
}
