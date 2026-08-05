import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import {
	AlertTriangle,
	Check,
	Clock3,
	Eye,
	EyeOff,
	FolderOpen,
	Info,
	Maximize2,
	Menu,
	Minimize2,
	OctagonX,
	X,
} from "lucide-react";
import {
	type KeyboardEvent,
	type PointerEvent,
	type ReactNode,
	useCallback,
	useEffect,
	useMemo,
	useRef,
	useState,
} from "react";
import { ColorValueInput } from "@/components/color-value-input";
import { NumericField } from "@/components/numeric-field";
import { getNumericDraftError, runtimeFloatMaximum, runtimeFloatMinimum } from "@/components/numeric-field-model";
import { Button } from "@/components/ui/button";
import {
	DropdownMenu,
	DropdownMenuContent,
	DropdownMenuItem,
	DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu";
import { Input } from "@/components/ui/input";
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from "@/components/ui/select";
import { Switch } from "@/components/ui/switch";
import { Textarea } from "@/components/ui/textarea";
import {
	type DesktopDialogPayload,
	type DialogChoice,
	type FormDialogField,
	type FormDialogPayload,
	formDialogActionLabels,
	type MessageDialogButton,
	type MessageDialogPayload,
	messageDialogButtonSets,
	messageDialogCloseButton,
	parseDesktopDialogPayload,
} from "@/lib/desktop-dialog-model";
import { formatDialogTimeout, remainingDialogTimeoutMs } from "@/lib/dialog-timeout";
import { datetimeInTimeZoneToIso } from "@/lib/time-format";

type DesktopDialogResponse = {
	button: MessageDialogButton;
	values: Record<string, unknown>;
};

type DesktopDialogConsoleWireState = {
	pendingCount: number;
	request: unknown;
	requestId: string;
};

type DesktopDialogConsoleState = Omit<DesktopDialogConsoleWireState, "request"> & {
	request: DesktopDialogPayload;
};

type DesktopDialogConsoleWindowState = {
	fullscreen: boolean;
};

type FormDialogDraft = boolean | number | string | string[];
type FormDialogDrafts = Record<string, FormDialogDraft>;

const floatContract = {
	kind: "float",
	maximum: runtimeFloatMaximum,
	minimum: runtimeFloatMinimum,
	signed: true,
} as const;

const variantIcon = {
	error: OctagonX,
	info: Info,
	warning: AlertTriangle,
};

const variantColor = {
	error: "text-baud-danger",
	info: "text-baud-blue",
	warning: "text-baud-amber",
};

export function DesktopDialogView({ requestId }: { requestId: string }) {
	const [payload, setPayload] = useState<DesktopDialogPayload | null>(null);
	const [loadError, setLoadError] = useState<string | null>(null);
	const [submitting, setSubmitting] = useState(false);

	useEffect(() => {
		let active = true;
		void invoke<unknown>("fetch_desktop_dialog", { requestId })
			.then((next) => {
				if (active) {
					setPayload(parseDesktopDialogPayload(next));
					setSubmitting(false);
				}
			})
			.catch((error: unknown) => {
				if (active) setLoadError(errorMessage(error));
			});
		return () => {
			active = false;
		};
	}, [requestId]);

	const submit = useCallback(
		async (response: DesktopDialogResponse) => {
			if (submitting) return;
			setSubmitting(true);
			try {
				await invoke("submit_desktop_dialog", { requestId, response });
			} catch (error) {
				setLoadError(errorMessage(error));
				setSubmitting(false);
			}
		},
		[requestId, submitting],
	);

	const cancel = useCallback(async () => {
		if (submitting) return;
		setSubmitting(true);
		try {
			await invoke("cancel_desktop_dialog", { requestId });
		} catch (error) {
			setLoadError(errorMessage(error));
			setSubmitting(false);
		}
	}, [requestId, submitting]);

	if (loadError) {
		return (
			<main className="grid h-full place-items-center bg-baud-bg p-6 text-baud-text">
				<section className="w-full max-w-md" role="alert">
					<h1 className="text-base font-semibold">Dialog unavailable</h1>
					<p className="mt-2 break-words text-sm leading-5 text-baud-muted">{loadError}</p>
				</section>
			</main>
		);
	}

	if (!payload) {
		return (
			<main aria-label="Loading dialog" className="grid h-full place-items-center bg-baud-bg text-sm text-baud-muted">
				Loading...
			</main>
		);
	}

	return (
		<DesktopDialogContent
			cancel={cancel}
			key={requestId}
			payload={payload}
			pendingCount={null}
			requestId={requestId}
			submit={submit}
			submitting={submitting}
		/>
	);
}

export function DesktopDialogConsoleView() {
	const [state, setState] = useState<DesktopDialogConsoleState | null>(null);
	const [loadError, setLoadError] = useState<string | null>(null);
	const [submitting, setSubmitting] = useState(false);
	const loadSequence = useRef(0);
	const headerMenu = <DesktopDialogConsoleMenu />;

	const loadState = useCallback(async () => {
		const sequence = ++loadSequence.current;
		try {
			const wireState = await invoke<DesktopDialogConsoleWireState | null>("fetch_desktop_dialog_console");
			const next = wireState
				? { ...wireState, request: parseDesktopDialogPayload(wireState.request) }
				: null;
			if (sequence !== loadSequence.current) return;
			setState(next);
			setLoadError(null);
			setSubmitting(false);
		} catch (error) {
			if (sequence !== loadSequence.current) return;
			setLoadError(errorMessage(error));
			setSubmitting(false);
		}
	}, []);

	useEffect(() => {
		let active = true;
		let unlisten: (() => void) | null = null;
		void listen("desktop-dialog-console-changed", () => {
			if (active) void loadState();
		})
			.then((nextUnlisten) => {
				if (active) {
					unlisten = nextUnlisten;
					void loadState();
				} else {
					nextUnlisten();
				}
			})
			.catch((error: unknown) => {
				if (active) setLoadError(`Failed to listen for dialog console updates: ${errorMessage(error)}`);
			});
		return () => {
			active = false;
			loadSequence.current += 1;
			unlisten?.();
		};
	}, [loadState]);

	const submit = useCallback(
		async (response: DesktopDialogResponse) => {
			if (!state || submitting) return;
			setSubmitting(true);
			try {
				await invoke("submit_desktop_dialog", { requestId: state.requestId, response });
			} catch (error) {
				setLoadError(errorMessage(error));
				setSubmitting(false);
			}
		},
		[state, submitting],
	);

	const cancel = useCallback(async () => {
		if (!state || submitting) return;
		setSubmitting(true);
		try {
			await invoke("cancel_desktop_dialog", { requestId: state.requestId });
		} catch (error) {
			setLoadError(errorMessage(error));
			setSubmitting(false);
		}
	}, [state, submitting]);

	if (loadError) {
		return (
			<main className="grid h-full place-items-center bg-baud-bg p-6 text-baud-text">
				<section className="w-full max-w-md" role="alert">
					<h1 className="text-base font-semibold">Dialog console unavailable</h1>
					<p className="mt-2 break-words text-sm leading-5 text-baud-muted">{loadError}</p>
				</section>
			</main>
		);
	}

	if (!state) {
		return <DesktopDialogIdle headerMenu={headerMenu} />;
	}

	return (
		<DesktopDialogContent
			cancel={cancel}
			headerMenu={headerMenu}
			key={state.requestId}
			payload={state.request}
			pendingCount={state.pendingCount}
			requestId={state.requestId}
			submit={submit}
			submitting={submitting}
		/>
	);
}

function DesktopDialogContent({
	cancel,
	headerMenu,
	payload,
	pendingCount,
	requestId,
	submit,
	submitting,
}: {
	cancel: () => Promise<void>;
	headerMenu?: ReactNode;
	payload: DesktopDialogPayload;
	pendingCount: number | null;
	requestId: string;
	submit: (response: DesktopDialogResponse) => Promise<void>;
	submitting: boolean;
}) {
	return payload.kind === "message_dialog" ? (
		<MessageDialog
			headerMenu={headerMenu}
			payload={payload}
			pendingCount={pendingCount}
			submit={submit}
			submitting={submitting}
		/>
	) : (
		<FormDialog
			cancel={cancel}
			headerMenu={headerMenu}
			payload={payload}
			pendingCount={pendingCount}
			requestId={requestId}
			submit={submit}
			submitting={submitting}
		/>
	);
}

function DesktopDialogIdle({ headerMenu }: { headerMenu: ReactNode }) {
	return (
		<DialogShell
			actions={null}
			headerMenu={headerMenu}
			requestingScript={null}
			timeoutAtUnixMs={null}
			title="Waiting for dialog requests"
			titleIcon={<Clock3 aria-hidden="true" className="size-8 shrink-0 text-baud-muted" />}
			titleId="desktop-dialog-title"
		>
			<p className="mt-4 text-sm leading-6 text-baud-muted">No dialog request is active.</p>
		</DialogShell>
	);
}

function DesktopDialogConsoleMenu() {
	const [open, setOpen] = useState(false);
	const [fullscreen, setFullscreen] = useState(false);
	const [loading, setLoading] = useState(false);
	const [error, setError] = useState<string | null>(null);

	const refresh = useCallback(async () => {
		setLoading(true);
		try {
			const state = await invoke<DesktopDialogConsoleWindowState>("fetch_desktop_dialog_console_window_state");
			setFullscreen(state.fullscreen);
			setError(null);
		} catch (nextError) {
			setError(errorMessage(nextError));
		} finally {
			setLoading(false);
		}
	}, []);

	useEffect(() => {
		void refresh();
	}, [refresh]);

	const changeFullscreen = useCallback(async (nextFullscreen: boolean) => {
		setLoading(true);
		try {
			const state = await invoke<DesktopDialogConsoleWindowState>("set_desktop_dialog_console_fullscreen", {
				fullscreen: nextFullscreen,
			});
			setFullscreen(state.fullscreen);
			setError(null);
			setOpen(false);
		} catch (nextError) {
			setError(errorMessage(nextError));
		} finally {
			setLoading(false);
		}
	}, []);

	const FullscreenIcon = fullscreen ? Minimize2 : Maximize2;
	return (
		<DropdownMenu
			onOpenChange={(nextOpen) => {
				setOpen(nextOpen);
				if (nextOpen) void refresh();
			}}
			open={open}
		>
			<DropdownMenuTrigger asChild>
				<Button
					aria-label="Dialog console menu"
					className="size-7 shrink-0 p-0"
					size="sm"
					title="Dialog console menu"
					variant="subtle"
				>
					<Menu aria-hidden="true" className="size-4" />
				</Button>
			</DropdownMenuTrigger>
			<DropdownMenuContent align="end" collisionPadding={8}>
				<DropdownMenuItem
					disabled={loading}
					onSelect={(event) => {
						event.preventDefault();
						void changeFullscreen(!fullscreen);
					}}
				>
					<FullscreenIcon aria-hidden="true" />
					{fullscreen ? "Exit fullscreen" : "Enter fullscreen"}
				</DropdownMenuItem>
				{error && (
					<p className="max-w-64 break-words px-2 py-1.5 text-xs leading-4 text-baud-danger" role="alert">
						{error}
					</p>
				)}
			</DropdownMenuContent>
		</DropdownMenu>
	);
}

function DialogShell({
	actions,
	children,
	descriptionId,
	headerMenu,
	onKeyDown,
	pendingCount,
	requestingScript,
	timeoutAtUnixMs,
	title,
	titleIcon,
	titleId,
}: {
	actions: ReactNode;
	children: ReactNode;
	descriptionId?: string;
	headerMenu?: ReactNode;
	onKeyDown?: (event: KeyboardEvent<HTMLElement>) => void;
	pendingCount?: number | null;
	requestingScript: string | null;
	timeoutAtUnixMs: number | null;
	title: string;
	titleIcon?: ReactNode;
	titleId: string;
}) {
	return (
		<main
			className="fixed inset-0 grid min-h-0 min-w-0 max-w-full grid-rows-[auto_minmax(0,1fr)_auto] overflow-hidden bg-baud-bg text-baud-text"
			data-desktop-dialog-shell
		>
			<header className="min-w-0 overflow-hidden border-b border-baud-border bg-baud-panel px-5 py-3">
				<div className="grid min-w-0 grid-cols-[auto_minmax(0,1fr)] items-center gap-4">
					<div className="flex shrink-0 items-center gap-2">
						<img alt="" aria-hidden="true" className="size-5 shrink-0" height="20" src="/logo-notext.svg" width="20" />
						<span className="text-sm font-semibold">BaudBound</span>
					</div>
					{headerMenu ? (
						<div className="flex min-w-0 items-center justify-end gap-2">
							{requestingScript && (
								<span className="block min-w-0 truncate text-right text-xs leading-4 text-baud-muted">
									Requested by {requestingScript}
								</span>
							)}
							{headerMenu}
						</div>
					) : (
						<span className="block min-w-0 truncate text-right text-xs text-baud-muted">
							{requestingScript ? `Requested by ${requestingScript}` : null}
						</span>
					)}
				</div>
			</header>
			<section
				aria-describedby={descriptionId}
				aria-labelledby={titleId}
				aria-modal="true"
				className="min-h-0 min-w-0 max-w-full overflow-x-hidden overflow-y-auto px-5 py-5"
				data-desktop-dialog-content
				onKeyDown={onKeyDown}
				role="dialog"
			>
				<div className="flex min-w-0 items-center gap-3">
					{titleIcon}
					<h1 className="min-w-0 break-words text-base font-semibold" id={titleId}>
						{title}
					</h1>
				</div>
				{children}
			</section>
			<footer
				className="flex min-h-15 min-w-0 max-w-full flex-wrap items-center justify-between gap-3 overflow-hidden border-t border-baud-border bg-baud-panel px-5 py-3"
				data-desktop-dialog-footer
			>
				<div className="flex min-w-0 flex-wrap items-center gap-3">
					<DialogTimeoutCountdown timeoutAtUnixMs={timeoutAtUnixMs} />
					{typeof pendingCount === "number" && pendingCount > 0 && (
						<span className="rounded-full border border-baud-border px-2 py-0.5 text-xs text-baud-muted">
							{pendingCount} waiting
						</span>
					)}
				</div>
				{actions && <div className="ml-auto flex min-w-0 flex-wrap items-center justify-end gap-2">{actions}</div>}
			</footer>
		</main>
	);
}

function DialogTimeoutCountdown({ timeoutAtUnixMs }: { timeoutAtUnixMs: number | null }) {
	const [nowUnixMs, setNowUnixMs] = useState(() => Date.now());

	useEffect(() => {
		if (timeoutAtUnixMs === null) return;
		setNowUnixMs(Date.now());
		const interval = window.setInterval(() => setNowUnixMs(Date.now()), 250);
		return () => window.clearInterval(interval);
	}, [timeoutAtUnixMs]);

	if (timeoutAtUnixMs === null) return null;
	const remainingMs = remainingDialogTimeoutMs(timeoutAtUnixMs, nowUnixMs);
	return (
		<div className="flex shrink-0 items-center gap-1.5 text-xs text-baud-muted" role="timer">
			<Clock3 aria-hidden="true" className="size-3.5" />
			<span>{remainingMs > 0 ? `Times out in ${formatDialogTimeout(remainingMs)}` : "Timing out..."}</span>
		</div>
	);
}

function MessageDialog({
	headerMenu,
	payload,
	pendingCount,
	submit,
	submitting,
}: {
	headerMenu?: ReactNode;
	payload: MessageDialogPayload;
	pendingCount: number | null;
	submit: (response: DesktopDialogResponse) => Promise<void>;
	submitting: boolean;
}) {
	const configuredButtons = messageDialogButtonSets[payload.buttons];
	const initialButton = messageDialogCloseButton(payload.buttons) ?? configuredButtons[0];
	const initialRef = useRef<HTMLButtonElement>(null);
	const Icon = variantIcon[payload.variant];

	useEffect(() => initialRef.current?.focus(), []);

	const select = (button: MessageDialogButton) => submit({ button, values: {} });
	const onKeyDown = (event: KeyboardEvent<HTMLElement>) => {
		if (event.key !== "Escape") return;
		const closeButton = messageDialogCloseButton(payload.buttons);
		if (!closeButton) {
			event.preventDefault();
			return;
		}
		event.preventDefault();
		void select(closeButton);
	};

	return (
		<section className="h-full min-w-0 max-w-full overflow-hidden">
			<DialogShell
				actions={configuredButtons.map((button) => (
					<Button
						disabled={submitting}
						key={button}
						onClick={() => void select(button)}
						ref={button === initialButton ? initialRef : undefined}
						variant={button === "cancel" || button === "no" ? "outline" : "default"}
					>
						{buttonLabel(button)}
					</Button>
				))}
				descriptionId="desktop-dialog-description"
				headerMenu={headerMenu}
				onKeyDown={onKeyDown}
				pendingCount={pendingCount}
				requestingScript={payload.requestingScript}
				timeoutAtUnixMs={payload.timeoutAtUnixMs}
				title={payload.title}
				titleIcon={<Icon aria-hidden="true" className={`size-8 shrink-0 ${variantColor[payload.variant]}`} />}
				titleId="desktop-dialog-title"
			>
				<p
					className="mt-4 whitespace-pre-wrap break-words text-sm leading-6 text-baud-muted"
					id="desktop-dialog-description"
				>
					{payload.message}
				</p>
			</DialogShell>
		</section>
	);
}

function FormDialog({
	cancel,
	headerMenu,
	payload,
	pendingCount,
	requestId,
	submit,
	submitting,
}: {
	cancel: () => Promise<void>;
	headerMenu?: ReactNode;
	payload: FormDialogPayload;
	pendingCount: number | null;
	requestId: string;
	submit: (response: DesktopDialogResponse) => Promise<void>;
	submitting: boolean;
}) {
	const [drafts, setDrafts] = useState<FormDialogDrafts>(() => initialDrafts(payload.fields));
	const [revealedPasswords, setRevealedPasswords] = useState<Set<string>>(new Set());
	const formRef = useRef<HTMLFormElement>(null);
	const cancelRef = useRef<HTMLButtonElement>(null);
	const errors = useMemo(() => validateDrafts(payload.fields, drafts), [drafts, payload.fields]);
	const valid = Object.keys(errors).length === 0;

	useEffect(() => {
		const firstControl = formRef.current?.querySelector<HTMLElement>(
			"input:not([type='hidden']), textarea, button[role='switch']",
		);
		(firstControl ?? cancelRef.current)?.focus();
	}, []);

	const setDraft = useCallback((key: string, value: FormDialogDraft) => {
		setDrafts((current) => ({ ...current, [key]: value }));
	}, []);

	const clearPasswords = useCallback(() => {
		const passwordKeys = payload.fields.filter((field) => field.type === "password").map((field) => field.key);
		if (passwordKeys.length > 0) {
			setDrafts((current) => {
				const next = { ...current };
				for (const key of passwordKeys) next[key] = "";
				return next;
			});
		}
		setRevealedPasswords(new Set());
	}, [payload.fields]);

	const send = useCallback(() => {
		if (!valid || submitting) return;
		const values = normalizeDrafts(payload.fields, drafts);
		clearPasswords();
		void submit({ button: "ok", values });
	}, [clearPasswords, drafts, payload.fields, submit, submitting, valid]);

	const cancelDialog = useCallback(() => {
		clearPasswords();
		void cancel();
	}, [cancel, clearPasswords]);

	const onDialogKeyDown = (event: KeyboardEvent<HTMLElement>) => {
		if (event.key === "Escape") {
			event.preventDefault();
			cancelDialog();
		}
	};

	return (
		<section className="h-full min-w-0 max-w-full overflow-hidden">
			<DialogShell
				actions={
					<>
						<Button disabled={submitting} onClick={cancelDialog} ref={cancelRef} variant="outline">
							{formDialogActionLabels.cancel}
						</Button>
						<Button disabled={!valid || submitting} onClick={send}>
							{formDialogActionLabels.submit}
						</Button>
					</>
				}
				descriptionId={payload.description ? "desktop-dialog-description" : undefined}
				headerMenu={headerMenu}
				onKeyDown={onDialogKeyDown}
				pendingCount={pendingCount}
				requestingScript={payload.requestingScript}
				timeoutAtUnixMs={payload.timeoutAtUnixMs}
				title={payload.title}
				titleId="desktop-dialog-title"
			>
				{payload.description && (
					<p
						className="mt-3 whitespace-pre-wrap break-words text-sm leading-6 text-baud-muted"
						id="desktop-dialog-description"
					>
						{payload.description}
					</p>
				)}
				<form
					autoComplete="off"
					className="mt-5 grid gap-5"
					onSubmit={(event) => {
						event.preventDefault();
						send();
					}}
					ref={formRef}
				>
					{payload.fields.map((field, index) => (
						<FormDialogFieldControl
							draft={"key" in field ? drafts[field.key] : undefined}
							error={"key" in field ? errors[field.key] : undefined}
							field={field}
							id={`desktop-dialog-field-${index}`}
							key={"key" in field ? field.key : `display-${index}`}
							onChange={"key" in field ? (value) => setDraft(field.key, value) : undefined}
							onSubmit={send}
							requestId={requestId}
							passwordVisible={field.type === "password" && revealedPasswords.has(field.key)}
							setPasswordVisible={
								field.type === "password"
									? (visible) => {
											setRevealedPasswords((current) => {
												const next = new Set(current);
												if (visible) next.add(field.key);
												else next.delete(field.key);
												return next;
											});
										}
									: undefined
							}
						/>
					))}
				</form>
			</DialogShell>
		</section>
	);
}

function FormDialogFieldControl({
	draft,
	error,
	field,
	id,
	onChange,
	onSubmit,
	passwordVisible,
	requestId,
	setPasswordVisible,
}: {
	draft?: FormDialogDraft;
	error?: string;
	field: FormDialogField;
	id: string;
	onChange?: (value: FormDialogDraft) => void;
	onSubmit: () => void;
	passwordVisible: boolean;
	requestId: string;
	setPasswordVisible?: (visible: boolean) => void;
}) {
	if (field.type === "information") {
		return (
			<section
				className="rounded border border-baud-border border-l-[3px] bg-baud-soft/60 px-4 py-3 shadow-[0_1px_0_rgb(255_255_255/0.025)]"
				data-form-dialog-information
				style={{ borderLeftColor: field.accentColor }}
			>
				{field.label && <h2 className="text-sm font-semibold text-baud-text">{field.label}</h2>}
				{field.description && (
					<p className="mt-1 whitespace-pre-wrap break-words text-sm leading-5 text-baud-muted">{field.description}</p>
				)}
			</section>
		);
	}
	if (field.type === "section_heading") {
		return (
			<section className="border-b pb-2" style={{ borderColor: field.accentColor }}>
				{field.label && <h2 className="text-base font-semibold text-baud-text">{field.label}</h2>}
				{field.description && (
					<p className="mt-1 whitespace-pre-wrap break-words text-sm text-baud-muted">{field.description}</p>
				)}
			</section>
		);
	}
	if (field.type === "divider") {
		return <hr className="border-0 border-t" style={{ borderColor: field.accentColor }} />;
	}
	if (field.type === "image") {
		return (
			<figure>
				<img
					alt={field.label}
					className="w-full rounded border border-baud-border bg-baud-panel"
					src={field.dataUrl}
					style={{ height: field.imageHeight, objectFit: field.imageFit }}
				/>
				{field.description && (
					<figcaption className="mt-1 text-xs leading-5 text-baud-muted">{field.description}</figcaption>
				)}
			</figure>
		);
	}

	const errorId = `${id}-error`;
	const descriptionId = field.description ? `${id}-description` : undefined;
	const describedBy = [descriptionId, error ? errorId : undefined].filter(Boolean).join(" ") || undefined;
	const label = (
		<span className="w-fit text-sm font-medium text-baud-text" id={`${id}-label`}>
			{field.label}
			{field.required && <span className="ml-1 text-baud-danger">*</span>}
		</span>
	);
	const description = field.description && (
		<p className="text-xs leading-5 text-baud-muted" id={descriptionId}>
			{field.description}
		</p>
	);
	const validation = error && (
		<p className="text-xs leading-4 text-baud-danger" id={errorId} role="alert">
			{error}
		</p>
	);

	if (field.type === "checkbox") {
		return (
			<div className="grid gap-1.5">
				<div className="flex min-w-0 items-center justify-between gap-4">
					{label}
					<Switch
						aria-describedby={describedBy}
						aria-invalid={!!error || undefined}
						aria-labelledby={`${id}-label`}
						checked={draft === true}
						id={id}
						onCheckedChange={(checked) => onChange?.(checked)}
					/>
				</div>
				{description}
				{validation}
			</div>
		);
	}

	if (field.type === "dropdown") {
		return (
			<div className="grid gap-1.5">
				{label}
				{description}
				<Select onValueChange={(value) => onChange?.(value)} value={typeof draft === "string" ? draft : ""}>
					<SelectTrigger
						aria-describedby={describedBy}
						aria-invalid={!!error || undefined}
						aria-labelledby={`${id}-label`}
					>
						<SelectValue placeholder="Select a choice" />
					</SelectTrigger>
					<SelectContent>
						{field.choices.map((choice) => (
							<SelectItem key={choice.key} value={choice.key}>
								{choice.displayValue}
							</SelectItem>
						))}
					</SelectContent>
				</Select>
				{validation}
			</div>
		);
	}

	if (field.type === "single_choice" || field.type === "multi_choice") {
		return (
			<fieldset aria-describedby={descriptionId} aria-invalid={!!error || undefined} className="grid min-w-0 gap-2">
				<legend className="text-sm font-medium text-baud-text" id={`${id}-label`}>
					{field.label}
					{field.required && <span className="ml-1 text-baud-danger">*</span>}
				</legend>
				{description}
				<ChoiceList
					choices={field.choices}
					id={id}
					multiSelect={field.type === "multi_choice"}
					onChange={(selected) => onChange?.(selected)}
					selected={Array.isArray(draft) ? draft : []}
				/>
			</fieldset>
		);
	}

	if (field.type === "file" || field.type === "folder") {
		return (
			<PathFieldControl
				describedBy={describedBy}
				description={description}
				draft={draft}
				error={error}
				field={field}
				id={id}
				label={label}
				onChange={onChange}
				requestId={requestId}
				validation={validation}
			/>
		);
	}

	if (field.type === "slider") {
		const value = typeof draft === "number" ? draft : field.defaultValue;
		return (
			<div className="grid gap-1.5">
				{label}
				{description}
				<div className="flex items-center gap-3 rounded border border-baud-border bg-baud-panel px-3 py-2">
					<input
						autoComplete="off"
						aria-describedby={describedBy}
						aria-labelledby={`${id}-label`}
						className="min-w-0 flex-1 accent-baud-danger"
						max={field.maximum}
						min={field.minimum}
						onChange={(event) => onChange?.(Number(event.target.value))}
						step={field.step}
						type="range"
						value={value}
					/>
					<output className="w-20 truncate text-right font-mono text-sm">{value}</output>
				</div>
				{validation}
			</div>
		);
	}

	if (field.type === "color") {
		const value = typeof draft === "string" ? draft : field.defaultValue;
		return (
			<div className="grid gap-1.5">
				{label}
				{description}
				<ColorValueInput
					ariaDescribedBy={describedBy}
					ariaLabelledBy={`${id}-label`}
					id={id}
					invalid={!!error}
					label={field.label}
					onChange={(nextValue) => onChange?.(nextValue)}
					value={value}
				/>
				{validation}
			</div>
		);
	}

	if (field.type === "date" || field.type === "time" || field.type === "datetime") {
		return (
			<div className="grid gap-1.5">
				{label}
				{description}
				<Input
					aria-describedby={describedBy}
					aria-invalid={!!error || undefined}
					aria-labelledby={`${id}-label`}
					id={id}
					onChange={(event) => onChange?.(event.target.value)}
					step={field.type === "date" ? undefined : "1"}
					type={field.type === "datetime" ? "datetime-local" : field.type}
					value={typeof draft === "string" ? draft : ""}
				/>
				{validation}
			</div>
		);
	}

	if (field.type === "number") {
		return (
			<div className="grid gap-1.5">
				{label}
				{description}
				<NumericField
					ariaLabel={field.label}
					contract={floatContract}
					id={id}
					onChange={(value) => onChange?.(value)}
					placeholder={field.placeholder}
					required={field.required}
					value={typeof draft === "string" ? draft : ""}
				/>
			</div>
		);
	}

	if (field.type === "multiline") {
		return (
			<div className="grid gap-1.5">
				{label}
				{description}
				<Textarea
					aria-describedby={describedBy}
					aria-invalid={!!error || undefined}
					aria-labelledby={`${id}-label`}
					className="min-h-24 w-full resize-y"
					id={id}
					maxLength={16_384}
					onChange={(event) => onChange?.(event.target.value)}
					placeholder={field.placeholder}
					value={typeof draft === "string" ? draft : ""}
				/>
				{validation}
			</div>
		);
	}

	const password = field.type === "password";
	return (
		<div className="grid gap-1.5">
			{label}
			{description}
			<div className="relative">
				<Input
					aria-describedby={describedBy}
					aria-invalid={!!error || undefined}
					aria-labelledby={`${id}-label`}
					className={`w-full ${password ? "secret-value-input pr-11" : ""}`}
					id={id}
					maxLength={16_384}
					onChange={(event) => onChange?.(event.target.value)}
					onKeyDown={(event) => {
						if (event.key === "Enter" && !event.nativeEvent.isComposing) {
							event.preventDefault();
							onSubmit();
						}
					}}
					placeholder={field.placeholder}
					spellCheck={!password}
					type={password && !passwordVisible ? "password" : "text"}
					value={typeof draft === "string" ? draft : ""}
				/>
				{password && setPasswordVisible && (
					<PasswordRevealButton visible={passwordVisible} setVisible={setPasswordVisible} />
				)}
			</div>
			{validation}
		</div>
	);
}

function PathFieldControl({
	describedBy,
	description,
	draft,
	error,
	field,
	id,
	label,
	onChange,
	requestId,
	validation,
}: {
	describedBy?: string;
	description: ReactNode;
	draft?: FormDialogDraft;
	error?: string;
	field: Extract<FormDialogField, { type: "file" | "folder" }>;
	id: string;
	label: ReactNode;
	onChange?: (value: FormDialogDraft) => void;
	requestId: string;
	validation: ReactNode;
}) {
	const [selecting, setSelecting] = useState(false);
	const [pickerError, setPickerError] = useState("");
	const paths = Array.isArray(draft) ? draft : typeof draft === "string" && draft ? [draft] : [];

	const selectPaths = async () => {
		if (selecting) return;
		setSelecting(true);
		setPickerError("");
		try {
			const selected = await invoke<string[]>("select_desktop_dialog_paths", {
				mode: field.type,
				multiple: field.type === "file" && field.multiple,
				requestId,
			});
			if (selected.length > 0) onChange?.(field.type === "file" && field.multiple ? selected : selected[0]);
		} catch (cause) {
			setPickerError(errorMessage(cause));
		} finally {
			setSelecting(false);
		}
	};

	return (
		<div className="grid gap-1.5">
			{label}
			{description}
			<Button
				aria-describedby={describedBy}
				aria-invalid={!!error || undefined}
				className="justify-start"
				disabled={selecting}
				id={id}
				onClick={() => void selectPaths()}
				variant="outline"
			>
				<FolderOpen className="size-4" />
				{selecting
					? "Opening..."
					: field.type === "folder"
						? "Select folder"
						: field.multiple
							? "Select files"
							: "Select file"}
			</Button>
			{paths.length > 0 && (
				<ul className="grid gap-1">
					{paths.map((path) => (
						<li
							className="flex min-w-0 items-center gap-2 rounded border border-baud-border bg-baud-panel px-2 py-1.5"
							key={path}
						>
							<span className="min-w-0 flex-1 truncate font-mono text-xs" title={path}>
								{path}
							</span>
							<Button
								aria-label={`Remove ${path}`}
								onClick={() => {
									const next = paths.filter((candidate) => candidate !== path);
									onChange?.(field.type === "file" && field.multiple ? next : "");
								}}
								className="size-7 p-0"
								size="sm"
								variant="subtle"
							>
								<X className="size-3.5" />
							</Button>
						</li>
					))}
				</ul>
			)}
			{pickerError && (
				<p className="text-xs text-baud-danger" role="alert">
					{pickerError}
				</p>
			)}
			{validation}
		</div>
	);
}

function PasswordRevealButton({ setVisible, visible }: { setVisible: (visible: boolean) => void; visible: boolean }) {
	const conceal = (event?: PointerEvent<HTMLButtonElement>) => {
		if (event?.currentTarget.hasPointerCapture(event.pointerId)) {
			event.currentTarget.releasePointerCapture(event.pointerId);
		}
		setVisible(false);
	};
	return (
		<button
			aria-label="Hold to reveal password"
			className="absolute inset-y-0 right-0 grid w-10 place-items-center text-baud-muted outline-none hover:text-baud-text focus-visible:text-baud-text [&_svg]:size-5"
			onBlur={() => setVisible(false)}
			onPointerCancel={conceal}
			onPointerDown={(event) => {
				if (event.button !== 0) return;
				event.currentTarget.setPointerCapture(event.pointerId);
				setVisible(true);
			}}
			onPointerUp={conceal}
			type="button"
		>
			{visible ? <EyeOff /> : <Eye />}
		</button>
	);
}

function ChoiceList({
	choices,
	id,
	multiSelect,
	onChange,
	selected,
}: {
	choices: DialogChoice[];
	id: string;
	multiSelect: boolean;
	onChange: (value: string[]) => void;
	selected: string[];
}) {
	const selectedSet = new Set(selected);
	const toggle = (key: string) => {
		if (!multiSelect) {
			onChange([key]);
			return;
		}
		const next = new Set(selectedSet);
		if (next.has(key)) next.delete(key);
		else next.add(key);
		onChange(choices.filter((choice) => next.has(choice.key)).map((choice) => choice.key));
	};
	return (
		<div className="grid max-h-72 gap-2 overflow-x-hidden overflow-y-auto pr-1">
			{choices.map((choice, index) => {
				const checked = selectedSet.has(choice.key);
				return (
					<label
						className={`flex min-h-11 w-full cursor-pointer items-center gap-3 rounded-md border bg-baud-panel px-3 py-2 text-left text-sm transition-colors has-[:focus-visible]:border-baud-red has-[:focus-visible]:ring-2 has-[:focus-visible]:ring-baud-red/30 ${
							checked
								? "border-baud-red text-baud-text"
								: "border-baud-border text-baud-muted hover:border-baud-red/60 hover:text-baud-text"
						}`}
						key={choice.key}
					>
						<span className="relative grid size-4 shrink-0 place-items-center">
							<input
								autoComplete="off"
								checked={checked}
								className="peer col-start-1 row-start-1 size-4 cursor-pointer appearance-none rounded-full border border-baud-muted outline-none checked:border-baud-red checked:bg-baud-red"
								id={`${id}-choice-${index}`}
								name={multiSelect ? undefined : id}
								onChange={() => toggle(choice.key)}
								type={multiSelect ? "checkbox" : "radio"}
							/>
							<Check
								aria-hidden="true"
								className="pointer-events-none col-start-1 row-start-1 size-3 text-white opacity-0 peer-checked:opacity-100"
								strokeWidth={3}
							/>
						</span>
						<span className="min-w-0 break-words">{choice.displayValue}</span>
					</label>
				);
			})}
		</div>
	);
}

function initialDrafts(fields: FormDialogField[]): FormDialogDrafts {
	const drafts: FormDialogDrafts = {};
	for (const field of fields) {
		switch (field.type) {
			case "information":
			case "section_heading":
			case "divider":
			case "image":
				break;
			case "checkbox":
				drafts[field.key] = field.defaultChecked;
				break;
			case "multi_choice":
			case "single_choice":
				drafts[field.key] = [];
				break;
			case "dropdown":
			case "password":
			case "folder":
				drafts[field.key] = "";
				break;
			case "file":
				drafts[field.key] = field.multiple ? [] : "";
				break;
			case "number":
				drafts[field.key] = field.defaultValue === null ? "" : String(field.defaultValue);
				break;
			case "slider":
				drafts[field.key] = field.defaultValue;
				break;
			case "color":
			case "date":
			case "datetime":
			case "multiline":
			case "text":
			case "time":
				drafts[field.key] = field.defaultValue;
				break;
		}
	}
	return drafts;
}

function validateDrafts(fields: FormDialogField[], drafts: FormDialogDrafts) {
	const errors: Record<string, string> = {};
	for (const field of fields) {
		if (!("key" in field)) continue;
		const draft = drafts[field.key];
		if (field.type === "number") {
			const error = getNumericDraftError(typeof draft === "string" ? draft : "", floatContract, field.required);
			if (error) errors[field.key] = error;
		} else if (field.type === "checkbox") {
			if (field.required && draft !== true) errors[field.key] = "This option must be enabled.";
		} else if (field.type === "single_choice" || field.type === "multi_choice") {
			if (field.required && (!Array.isArray(draft) || draft.length === 0)) {
				errors[field.key] = "Select at least one choice.";
			}
		} else if (field.type === "file" && field.multiple) {
			if (field.required && (!Array.isArray(draft) || draft.length === 0)) {
				errors[field.key] = "Select at least one file.";
			}
		} else if (field.type === "color" && (typeof draft !== "string" || !/^#[0-9A-Fa-f]{6}$/.test(draft))) {
			errors[field.key] = "Select a valid color.";
		} else if (
			field.type === "datetime" &&
			typeof draft === "string" &&
			draft &&
			!datetimeInTimeZoneToIso(draft.length === 16 ? `${draft}:00` : draft, field.timezone)
		) {
			errors[field.key] = "Select a valid date and time for this timezone.";
		} else if (field.required && (typeof draft !== "string" || draft.length === 0)) {
			errors[field.key] = "A value is required.";
		}
	}
	return errors;
}

function normalizeDrafts(fields: FormDialogField[], drafts: FormDialogDrafts) {
	const values: Record<string, unknown> = {};
	for (const field of fields) {
		if (!("key" in field)) continue;
		const draft = drafts[field.key];
		if (field.type === "number") {
			if (typeof draft === "string" && draft.trim()) values[field.key] = Number(draft);
		} else if (field.type === "slider") {
			values[field.key] = typeof draft === "number" ? draft : field.defaultValue;
		} else if (field.type === "checkbox") {
			values[field.key] = draft === true;
		} else if (field.type === "single_choice") {
			values[field.key] = Array.isArray(draft) ? (draft[0] ?? "") : "";
		} else if (field.type === "multi_choice") {
			values[field.key] = Array.isArray(draft) ? draft : [];
		} else if (field.type === "file" && field.multiple) {
			values[field.key] = Array.isArray(draft) ? draft : [];
		} else if (field.type === "datetime") {
			const local = typeof draft === "string" ? draft : "";
			values[field.key] = local
				? (datetimeInTimeZoneToIso(local.length === 16 ? `${local}:00` : local, field.timezone) ?? "")
				: "";
		} else {
			values[field.key] = typeof draft === "string" ? draft : "";
		}
	}
	return values;
}

function buttonLabel(button: MessageDialogButton) {
	return button.slice(0, 1).toUpperCase() + button.slice(1);
}

function errorMessage(error: unknown) {
	return error instanceof Error ? error.message : String(error);
}
