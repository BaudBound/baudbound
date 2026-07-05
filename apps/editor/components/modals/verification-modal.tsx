"use client";

import { AlertTriangle, CheckCircle2, Circle, Loader2, X, XCircle } from "lucide-react";
import { useEffect, useId, useMemo, useState } from "react";
import { Button } from "@/components/ui/button";
import { Dialog, DialogContent, DialogDescription, DialogHeader, DialogTitle } from "@/components/ui/dialog";
import {
	summarizeVerification,
	type VerificationCheck,
	type VerificationOutcome,
	type VerificationSummary,
} from "@/utils/verification";

type VerificationStepStatus = VerificationOutcome | "pending" | "running";

type VerificationStepView = VerificationCheck & {
	status: VerificationStepStatus;
};

type VerificationModalProps = {
	checks: VerificationCheck[];
	onClose: () => void;
	open: boolean;
};

type VerificationProgressProps = {
	active?: boolean;
	checks: VerificationCheck[];
	onComplete?: (summary: VerificationSummary) => void;
};

const VERIFICATION_INITIAL_DELAY_MS = 1;
const VERIFICATION_STEP_DURATION_MS = 1;
const VERIFICATION_STEP_GAP_MS = 1;

export function VerificationModal({ checks, onClose, open }: VerificationModalProps) {
	const titleId = useId();
	const summary = summarizeVerification(checks);

	if (!open) {
		return null;
	}

	return (
		<Dialog open={open} onOpenChange={(nextOpen) => !nextOpen && onClose()}>
			<DialogContent
				aria-labelledby={titleId}
				className="grid max-h-[calc(100dvh-2rem)] grid-rows-[auto_minmax(0,1fr)] gap-0 overflow-hidden p-0 sm:max-w-2xl"
				onInteractOutside={(event) => event.preventDefault()}
				showCloseButton={false}
			>
				<DialogHeader className="flex flex-row items-start justify-between gap-4 border-b border-baud-border p-4">
					<div>
						<DialogTitle id={titleId}>Verification</DialogTitle>
						<DialogDescription>{getVerificationSummary(true, summary.failed, summary.warnings)}</DialogDescription>
					</div>
					<Button type="button" onClick={onClose} aria-label="Close verification" size="icon" variant="icon">
						<X size={15} />
					</Button>
				</DialogHeader>

				<div className="grid min-h-0 grid-rows-[minmax(0,1fr)_auto] gap-4 p-4">
					<div className="min-h-0 overflow-y-auto pr-1">
						<VerificationProgress checks={checks} active={open} />
					</div>

					<div className="flex justify-end">
						<Button type="button" onClick={onClose} variant="toolbar">
							Close
						</Button>
					</div>
				</div>
			</DialogContent>
		</Dialog>
	);
}

export function VerificationProgress({ active = true, checks, onComplete }: VerificationProgressProps) {
	const initialSteps = useMemo(() => createInitialSteps(checks), [checks]);
	const [steps, setSteps] = useState<VerificationStepView[]>(initialSteps);

	useEffect(() => {
		if (!active) {
			return;
		}

		const timers: number[] = [];
		let cancelled = false;
		setSteps(createInitialSteps(checks));

		const runStep = (index: number) => {
			if (cancelled) {
				return;
			}

			if (index >= checks.length) {
				onComplete?.(summarizeVerification(checks));
				return;
			}

			setSteps((currentSteps) =>
				currentSteps.map((step, stepIndex) => (stepIndex === index ? { ...step, status: "running" } : step)),
			);

			timers.push(
				window.setTimeout(() => {
					if (cancelled) {
						return;
					}

					setSteps((currentSteps) =>
						currentSteps.map((step, stepIndex) =>
							stepIndex === index ? { ...step, status: checks[index].outcome } : step,
						),
					);

					timers.push(window.setTimeout(() => runStep(index + 1), VERIFICATION_STEP_GAP_MS));
				}, VERIFICATION_STEP_DURATION_MS),
			);
		};

		timers.push(window.setTimeout(() => runStep(0), VERIFICATION_INITIAL_DELAY_MS));

		return () => {
			cancelled = true;
			for (const timer of timers) {
				window.clearTimeout(timer);
			}
		};
	}, [active, checks, onComplete]);

	const completedSteps = steps.filter((step) => step.status !== "pending" && step.status !== "running").length;
	const failedSteps = steps.filter((step) => step.status === "failed").length;
	const warningSteps = steps.filter((step) => step.status === "warning").length;
	const running = steps.some((step) => step.status === "running");
	const complete = completedSteps === steps.length;
	const progress = steps.length > 0 ? (completedSteps / steps.length) * 100 : 0;

	return (
		<div className="space-y-4">
			<div>
				<div className="mb-2 flex items-center justify-between font-mono text-xs text-baud-muted">
					<span>{running ? "Running checks" : complete ? "Checks complete" : "Preparing checks"}</span>
					<span>
						{completedSteps}/{steps.length}
					</span>
				</div>
				<div className="h-1.5 overflow-hidden rounded bg-baud-soft">
					<div className="h-full bg-baud-green transition-[width] duration-200" style={{ width: `${progress}%` }} />
				</div>
			</div>

			<div className="space-y-2">
				{steps.map((step) => (
					<div key={step.id} className="flex gap-3 rounded border border-baud-border bg-baud-soft/60 p-3">
						<div className="mt-0.5">{getStepIcon(step.status)}</div>
						<div className="min-w-0 flex-1">
							<div className="flex items-center justify-between gap-3">
								<h3 className="text-sm font-bold text-baud-text">{step.title}</h3>
								<span className={`font-mono text-xs uppercase ${getStepStatusClassName(step.status)}`}>
									{getStepStatusLabel(step.status)}
								</span>
							</div>
							<p className="mt-1 text-sm leading-5 text-baud-muted">{step.description}</p>
							{step.status !== "pending" && step.status !== "running" && (
								<p className={`mt-1 text-sm leading-5 ${getStepMessageClassName(step.status)}`}>{step.message}</p>
							)}
						</div>
					</div>
				))}
			</div>

			{complete && (
				<p className="text-sm leading-5 text-baud-muted">{getVerificationSummary(true, failedSteps, warningSteps)}</p>
			)}
		</div>
	);
}

function createInitialSteps(checks: VerificationCheck[]): VerificationStepView[] {
	return checks.map((check) => ({ ...check, status: "pending" }));
}

function getStepIcon(status: VerificationStepStatus) {
	if (status === "running") {
		return <Loader2 size={16} className="animate-spin text-baud-blue" />;
	}

	if (status === "passed") {
		return <CheckCircle2 size={16} className="text-baud-green" />;
	}

	if (status === "warning") {
		return <AlertTriangle size={16} className="text-baud-amber" />;
	}

	if (status === "failed") {
		return <XCircle size={16} className="text-baud-danger" />;
	}

	return <Circle size={16} className="text-baud-muted" />;
}

function getStepStatusClassName(status: VerificationStepStatus) {
	if (status === "passed") {
		return "text-baud-green";
	}

	if (status === "warning") {
		return "text-baud-amber";
	}

	if (status === "failed") {
		return "text-baud-danger";
	}

	if (status === "running") {
		return "text-baud-blue";
	}

	return "text-baud-muted";
}

function getStepMessageClassName(status: VerificationStepStatus) {
	if (status === "failed") {
		return "text-baud-danger";
	}

	if (status === "warning") {
		return "text-baud-amber";
	}

	return "text-baud-text";
}

function getStepStatusLabel(status: VerificationStepStatus) {
	if (status === "passed") {
		return "Passed";
	}

	if (status === "warning") {
		return "Warning";
	}

	if (status === "failed") {
		return "Failed";
	}

	if (status === "running") {
		return "Running";
	}

	return "Pending";
}

function getVerificationSummary(complete: boolean, failedSteps: number, warningSteps: number) {
	if (!complete) {
		return "Checking the script step by step.";
	}

	if (failedSteps > 0) {
		return `${failedSteps} failed check${failedSteps === 1 ? "" : "s"} must be resolved.`;
	}

	if (warningSteps > 0) {
		return `${warningSteps} warning${warningSteps === 1 ? "" : "s"} should be reviewed.`;
	}

	return "All checks passed.";
}
