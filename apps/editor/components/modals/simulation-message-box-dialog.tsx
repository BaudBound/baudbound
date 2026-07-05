"use client";

import { AlertTriangle, HelpCircle, Info, OctagonX } from "lucide-react";
import type { ComponentType } from "react";
import { useId } from "react";
import { Button } from "@/components/ui/button";
import {
	Dialog,
	DialogContent,
	DialogDescription,
	DialogFooter,
	DialogHeader,
	DialogTitle,
} from "@/components/ui/dialog";
import type { SimulationSideEffect } from "@/utils/simulation";

type MessageBoxSideEffect = Extract<SimulationSideEffect, { type: "message_box" }>;

type SimulationMessageBoxDialogProps = {
	messageBox: MessageBoxSideEffect | null;
	onSelect: (button: string) => void;
};

const variantIcon: Record<MessageBoxSideEffect["variant"], ComponentType<{ className?: string }>> = {
	error: OctagonX,
	info: Info,
	question: HelpCircle,
	warning: AlertTriangle,
};

const variantClassName: Record<MessageBoxSideEffect["variant"], string> = {
	error: "text-baud-danger",
	info: "text-baud-blue",
	question: "text-baud-purple",
	warning: "text-baud-amber",
};

export function SimulationMessageBoxDialog({ messageBox, onSelect }: SimulationMessageBoxDialogProps) {
	const titleId = useId();
	const Icon = messageBox ? variantIcon[messageBox.variant] : Info;

	return (
		<Dialog open={!!messageBox} onOpenChange={() => undefined}>
			<DialogContent
				aria-labelledby={titleId}
				className="sm:max-w-lg"
				showCloseButton={false}
				onEscapeKeyDown={(event) => event.preventDefault()}
				onInteractOutside={(event) => event.preventDefault()}
				onPointerDownOutside={(event) => event.preventDefault()}
			>
				{messageBox && (
					<>
						<DialogHeader className="grid grid-cols-[32px_minmax(0,1fr)] gap-x-3">
							<Icon className={`mt-0.5 size-6 ${variantClassName[messageBox.variant]}`} />
							<div className="min-w-0 space-y-2">
								<DialogTitle id={titleId} className="text-base text-baud-text">
									{messageBox.title || "Message"}
								</DialogTitle>
								<DialogDescription className="whitespace-pre-wrap break-words text-sm leading-5 text-baud-muted">
									{messageBox.message}
								</DialogDescription>
							</div>
						</DialogHeader>
						<DialogFooter className="bg-baud-panel">
							{messageBox.buttons.map((button) => (
								<Button
									key={button}
									type="button"
									onClick={() => onSelect(button)}
									variant={button === "cancel" || button === "no" ? "outline" : "toolbar"}
								>
									{formatButtonLabel(button)}
								</Button>
							))}
						</DialogFooter>
					</>
				)}
			</DialogContent>
		</Dialog>
	);
}

function formatButtonLabel(button: string) {
	return button
		.split("_")
		.map((part) => `${part.slice(0, 1).toUpperCase()}${part.slice(1)}`)
		.join(" ");
}
