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

export function DeleteProjectDialog({
	name,
	open,
	onCancel,
	onConfirm,
}: {
	name: string;
	open: boolean;
	onCancel: () => void;
	onConfirm: () => void;
}) {
	return (
		<Dialog open={open} onOpenChange={(nextOpen) => !nextOpen && onCancel()}>
			<DialogContent className="sm:max-w-md">
				<DialogHeader>
					<DialogTitle>Delete project</DialogTitle>
					<DialogDescription>
						Delete <strong className="text-baud-text">{name}</strong> and its stored assets from this browser? This
						action cannot be undone.
					</DialogDescription>
				</DialogHeader>
				<DialogFooter>
					<Button type="button" variant="toolbar" onClick={onCancel}>
						Cancel
					</Button>
					<Button type="button" variant="destructive" onClick={onConfirm}>
						Delete project
					</Button>
				</DialogFooter>
			</DialogContent>
		</Dialog>
	);
}

export function ImportConflictDialog({
	name,
	open,
	onCancel,
	onImportCopy,
	onOpenExisting,
	onReplace,
}: {
	name: string;
	open: boolean;
	onCancel: () => void;
	onImportCopy: () => void;
	onOpenExisting: () => void;
	onReplace: () => void;
}) {
	return (
		<Dialog open={open} onOpenChange={(nextOpen) => !nextOpen && onCancel()}>
			<DialogContent className="sm:max-w-lg">
				<DialogHeader>
					<DialogTitle>Project already exists</DialogTitle>
					<DialogDescription>
						A local project has the same identity as <strong className="text-baud-text">{name}</strong>. Choose how to
						handle the imported package.
					</DialogDescription>
				</DialogHeader>
				<div className="grid gap-2 text-sm text-baud-muted">
					<p>
						<strong className="text-baud-text">Open existing</strong> keeps the local project unchanged.
					</p>
					<p>
						<strong className="text-baud-text">Replace</strong> replaces its saved content with the package.
					</p>
					<p>
						<strong className="text-baud-text">Import copy</strong> creates an independent project identity.
					</p>
				</div>
				<DialogFooter className="flex-wrap">
					<Button type="button" variant="toolbar" onClick={onCancel}>
						Cancel
					</Button>
					<Button type="button" variant="toolbar" onClick={onOpenExisting}>
						Open existing
					</Button>
					<Button type="button" variant="toolbar" onClick={onImportCopy}>
						Import copy
					</Button>
					<Button type="button" variant="primary" onClick={onReplace}>
						Replace
					</Button>
				</DialogFooter>
			</DialogContent>
		</Dialog>
	);
}
