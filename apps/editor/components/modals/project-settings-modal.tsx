"use client";

import { X } from "lucide-react";
import { type ClipboardEvent, type KeyboardEvent, useEffect, useId, useState } from "react";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import {
	Dialog,
	DialogContent,
	DialogDescription,
	DialogFooter,
	DialogHeader,
	DialogTitle,
} from "@/components/ui/dialog";
import { Input } from "@/components/ui/input";
import { OptionCombobox } from "@/components/ui/option-combobox";
import { Textarea } from "@/components/ui/textarea";
import { targetRuntimes } from "@/data/project/runtimes";
import type { ProjectSettings, TargetRuntime } from "@/lib/types";
import { DEFAULT_MINIMUM_RUNNER_VERSION } from "@/lib/version";
import { getScriptVersionError, getUpdateDescriptorUrlError } from "@/utils/script-update";

type ProjectSettingsModalProps = {
	description?: string;
	open: boolean;
	projectId?: string;
	saveLabel?: string;
	settings: ProjectSettings;
	title?: string;
	onClose: () => void;
	onSave: (settings: ProjectSettings) => void;
};

export function ProjectSettingsModal({
	description = "Configure package metadata and runtime settings used during export.",
	open,
	projectId,
	saveLabel = "Save Settings",
	settings,
	title = "Project Settings",
	onClose,
	onSave,
}: ProjectSettingsModalProps) {
	const titleId = useId();
	const descriptionId = useId();
	const [draft, setDraft] = useState(settings);
	const [tagsDraft, setTagsDraft] = useState<string[]>(settings.tags);
	const [tagInput, setTagInput] = useState("");

	useEffect(() => {
		if (!open) {
			return;
		}

		setDraft(settings);
		setTagsDraft(settings.tags);
		setTagInput("");
	}, [open, settings]);

	const nameError = draft.name.trim().length === 0 ? "Project name is required." : "";
	const nameLengthError = draft.name.length > 128 ? "Project name cannot exceed 128 characters." : "";
	const descriptionError = draft.description.length > 4096 ? "Description cannot exceed 4096 characters." : "";
	const authorError = draft.author.length > 128 ? "Author cannot exceed 128 characters." : "";
	const minimumRunnerError =
		draft.minimumRunnerVersion.length > 64 ? "Minimum runner cannot exceed 64 characters." : "";
	const versionError = getScriptVersionError(draft.version);
	const updateUrlError = getUpdateDescriptorUrlError(draft.updateUrl);
	const websiteError = getOptionalUrlError(draft.website);
	const sourceError = getOptionalUrlError(draft.source);
	const tagsError = getTagsError(appendTags(tagsDraft, tagInput));
	const hasErrors = Boolean(
		nameError ||
			nameLengthError ||
			descriptionError ||
			authorError ||
			versionError ||
			updateUrlError ||
			minimumRunnerError ||
			websiteError ||
			sourceError ||
			tagsError,
	);

	const handleSave = () => {
		if (hasErrors) {
			return;
		}

		const nextTags = appendTags(tagsDraft, tagInput);

		onSave({
			...draft,
			name: draft.name.trim(),
			description: draft.description.trim(),
			author: draft.author.trim(),
			version: draft.version.trim(),
			updateUrl: draft.updateUrl.trim(),
			website: draft.website.trim(),
			source: draft.source.trim(),
			minimumRunnerVersion: draft.minimumRunnerVersion.trim() || DEFAULT_MINIMUM_RUNNER_VERSION,
			tags: nextTags,
		});
		onClose();
	};

	return (
		<Dialog open={open} onOpenChange={(nextOpen) => !nextOpen && onClose()}>
			<DialogContent aria-labelledby={titleId} className="sm:max-w-2xl">
				<DialogHeader>
					<DialogTitle id={titleId}>{title}</DialogTitle>
					<DialogDescription>{description}</DialogDescription>
				</DialogHeader>

				<div className="grid max-h-[70vh] gap-4 overflow-y-auto pr-1">
					{projectId && (
						<div>
							<label htmlFor={`${titleId}-project-id`} className="mb-1 block font-mono text-sm text-baud-muted">
								Project ID
							</label>
							<Input id={`${titleId}-project-id`} value={projectId} readOnly className="font-mono" />
							<p className="mt-1 text-xs leading-4 text-baud-muted">
								This stable ID lets the runner recognize later exports as the same script.
							</p>
						</div>
					)}
					<div className="grid grid-cols-1 gap-3 sm:grid-cols-2">
						<TextField
							label="Name"
							value={draft.name}
							error={nameError || nameLengthError}
							maxLength={128}
							onChange={(value) => setDraft((current) => ({ ...current, name: value }))}
						/>
						<div>
							<span className="mb-1 block font-mono text-sm text-baud-muted">Target Runtime</span>
							<OptionCombobox
								ariaLabel="Target runtime"
								options={targetRuntimes.map((runtime) => ({ label: runtime, value: runtime }))}
								value={draft.targetRuntime}
								onChange={(value) => setDraft((current) => ({ ...current, targetRuntime: value as TargetRuntime }))}
							/>
						</div>
					</div>

					<div className="grid grid-cols-1 gap-3 sm:grid-cols-2">
						<TextField
							label="Script Version"
							value={draft.version}
							error={versionError}
							maxLength={128}
							onChange={(value) => setDraft((current) => ({ ...current, version: value }))}
						/>
						<TextField
							label="Update URL"
							value={draft.updateUrl}
							error={updateUrlError}
							maxLength={2048}
							onChange={(value) => setDraft((current) => ({ ...current, updateUrl: value }))}
						/>
					</div>
					<p className="-mt-3 text-xs leading-4 text-baud-muted">
						The optional update URL must use HTTPS and point to update.json. The editor does not contact it.
					</p>

					<div>
						<label htmlFor={descriptionId} className="mb-1 block font-mono text-sm text-baud-muted">
							Description
						</label>
						<Textarea
							id={descriptionId}
							value={draft.description}
							onChange={(event) => setDraft((current) => ({ ...current, description: event.target.value }))}
							className="min-h-24"
							maxLength={4096}
						/>
						{descriptionError && <p className="mt-1 text-xs leading-4 text-baud-danger">{descriptionError}</p>}
					</div>

					<div className="grid grid-cols-1 gap-3 sm:grid-cols-2">
						<TextField
							label="Author"
							value={draft.author}
							error={authorError}
							maxLength={128}
							onChange={(value) => setDraft((current) => ({ ...current, author: value }))}
						/>
						<TextField
							label="Minimum Runner"
							value={draft.minimumRunnerVersion}
							error={minimumRunnerError}
							maxLength={64}
							onChange={(value) => setDraft((current) => ({ ...current, minimumRunnerVersion: value }))}
						/>
					</div>

					<div className="grid grid-cols-1 gap-3 sm:grid-cols-2">
						<TextField
							label="Website"
							value={draft.website}
							error={websiteError}
							maxLength={2048}
							onChange={(value) => setDraft((current) => ({ ...current, website: value }))}
						/>
						<TextField
							label="Source"
							value={draft.source}
							error={sourceError}
							maxLength={2048}
							onChange={(value) => setDraft((current) => ({ ...current, source: value }))}
						/>
					</div>

					<TagField
						tags={tagsDraft}
						inputValue={tagInput}
						error={tagsError}
						onInputChange={setTagInput}
						onTagsChange={setTagsDraft}
					/>
				</div>

				<DialogFooter className="bg-baud-panel">
					<Button type="button" variant="toolbar" onClick={onClose}>
						Cancel
					</Button>
					<Button type="button" variant="primary" onClick={handleSave} disabled={hasErrors}>
						{saveLabel}
					</Button>
				</DialogFooter>
			</DialogContent>
		</Dialog>
	);
}

function TextField({
	error,
	label,
	maxLength,
	value,
	onChange,
}: {
	error?: string;
	label: string;
	maxLength?: number;
	value: string;
	onChange: (value: string) => void;
}) {
	const inputId = useId();

	return (
		<div>
			<label htmlFor={inputId} className="mb-1 block font-mono text-sm text-baud-muted">
				{label}
			</label>
			<Input
				id={inputId}
				value={value}
				maxLength={maxLength}
				onChange={(event) => onChange(event.target.value)}
				aria-invalid={Boolean(error)}
			/>
			{error && <p className="mt-1 text-xs leading-4 text-baud-danger">{error}</p>}
		</div>
	);
}

function TagField({
	error,
	inputValue,
	tags,
	onInputChange,
	onTagsChange,
}: {
	error?: string;
	inputValue: string;
	tags: string[];
	onInputChange: (value: string) => void;
	onTagsChange: (tags: string[]) => void;
}) {
	const inputId = useId();

	const commitInput = () => {
		const nextTags = appendTags(tags, inputValue);
		if (nextTags !== tags) {
			onTagsChange(nextTags);
		}
		onInputChange("");
	};

	const handleKeyDown = (event: KeyboardEvent<HTMLInputElement>) => {
		if (!isTagCommitKey(event.key)) {
			return;
		}

		if (!inputValue.trim()) {
			return;
		}

		event.preventDefault();
		commitInput();
	};

	const handlePaste = (event: ClipboardEvent<HTMLInputElement>) => {
		const pastedText = event.clipboardData.getData("text");
		const pastedTags = parseTags(pastedText);
		if (pastedTags.length <= 1) {
			return;
		}

		event.preventDefault();
		onTagsChange(appendTags(tags, pastedText));
		onInputChange("");
	};

	return (
		<div>
			<label htmlFor={inputId} className="mb-1 block font-mono text-sm text-baud-muted">
				Tags
			</label>
			<div className="flex min-h-9 flex-wrap items-center gap-1 rounded-lg border border-baud-border bg-baud-soft px-2 py-1 transition-[border-color,box-shadow] focus-within:border-baud-red/75 focus-within:shadow-[0_0_0_2px_rgb(230_45_62_/_0.14)]">
				{tags.map((tag) => (
					<Badge key={tag} variant="outline" className="h-6 gap-1 border-baud-line bg-baud-panel px-2 text-baud-text">
						<span>{tag}</span>
						<button
							type="button"
							aria-label={`Remove ${tag}`}
							className="-mr-1 grid size-4 place-items-center rounded text-baud-muted hover:text-baud-text"
							onClick={() => onTagsChange(tags.filter((currentTag) => currentTag !== tag))}
						>
							<X size={12} />
						</button>
					</Badge>
				))}
				<input
					id={inputId}
					value={inputValue}
					onChange={(event) => onInputChange(event.target.value)}
					onKeyDown={handleKeyDown}
					onPaste={handlePaste}
					onBlur={commitInput}
					className="min-w-24 flex-1 bg-transparent px-1 py-1 text-sm outline-none placeholder:text-baud-muted"
					placeholder={tags.length === 0 ? "Add tags..." : ""}
					maxLength={64}
				/>
			</div>
			{error && <p className="mt-1 text-xs leading-4 text-baud-danger">{error}</p>}
			<p className="mt-1 text-xs leading-4 text-baud-muted">Press Enter, Space, Tab, or comma to create a tag.</p>
		</div>
	);
}

function isTagCommitKey(key: string) {
	return key === "Enter" || key === " " || key === "Spacebar" || key === "," || key === "Tab";
}

function appendTags(currentTags: string[], value: string) {
	const nextTags = [...currentTags];
	for (const tag of parseTags(value)) {
		if (!nextTags.includes(tag)) {
			nextTags.push(tag);
		}
	}

	return nextTags.length === currentTags.length ? currentTags : nextTags;
}

function parseTags(value: string) {
	return value
		.split(/[,\s]+/)
		.map((tag) => tag.trim())
		.filter(Boolean);
}

function getOptionalUrlError(value: string) {
	if (value.length > 2048) {
		return "URL cannot exceed 2048 characters.";
	}
	const trimmedValue = value.trim();
	if (!trimmedValue) {
		return "";
	}

	try {
		const url = new URL(trimmedValue);
		return url.protocol === "http:" || url.protocol === "https:" ? "" : "Use an http or https URL.";
	} catch {
		return "Use a valid URL.";
	}
}

function getTagsError(tags: string[]) {
	if (tags.length > 32) return "A project can have at most 32 tags.";
	if (tags.some((tag) => tag.length > 64)) return "Each tag can contain at most 64 characters.";
	return "";
}
