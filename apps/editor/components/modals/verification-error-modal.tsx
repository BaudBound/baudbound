"use client";

import { XCircle } from "lucide-react";
import { useId } from "react";
import { Button } from "@/components/ui/button";
import { Dialog, DialogContent, DialogDescription, DialogHeader, DialogTitle } from "@/components/ui/dialog";
import type { VerificationCheck } from "@/utils/verification";

type VerificationErrorModalProps = {
	checks: VerificationCheck[];
	description: string;
	open: boolean;
	title: string;
	onClose: () => void;
};

export function VerificationErrorModal({ checks, description, open, title, onClose }: VerificationErrorModalProps) {
	const titleId = useId();
	const failedChecks = checks.filter((check) => check.outcome === "failed");

	return (
		<Dialog open={open} onOpenChange={(nextOpen) => !nextOpen && onClose()}>
			<DialogContent aria-labelledby={titleId} className="sm:max-w-xl">
				<DialogHeader>
					<DialogTitle id={titleId}>{title}</DialogTitle>
					<DialogDescription>{description}</DialogDescription>
				</DialogHeader>

				<div className="space-y-2">
					{failedChecks.map((check) => (
						<div key={check.id} className="flex gap-3 rounded border border-baud-danger/40 bg-baud-danger/10 p-3">
							<XCircle size={16} className="mt-0.5 shrink-0 text-baud-danger" />
							<div className="min-w-0">
								<h3 className="text-sm font-bold text-baud-danger">{check.title}</h3>
								<p className="mt-1 text-sm leading-5 text-baud-danger">{check.message}</p>
							</div>
						</div>
					))}
				</div>

				<div className="flex justify-end">
					<Button type="button" variant="toolbar" onClick={onClose}>
						Close
					</Button>
				</div>
			</DialogContent>
		</Dialog>
	);
}
