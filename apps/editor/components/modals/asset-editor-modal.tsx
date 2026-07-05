"use client";

import { FileAudio2, FileImage, FileText, PackageCheck, Plus, Trash2, Upload } from "lucide-react";
import { type ChangeEvent, type DragEvent, useId, useRef, useState } from "react";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Dialog, DialogContent, DialogDescription, DialogHeader, DialogTitle } from "@/components/ui/dialog";
import { createEditorAssets, formatBytes, supportedAssetExtensions, validateEditorAssets } from "@/data/project/assets";
import type { AssetKind, EditorAsset } from "@/lib/types";
import { cn } from "@/lib/utils";

type AssetEditorModalProps = {
	assets: EditorAsset[];
	onAssetsChange: (assets: EditorAsset[]) => void;
	onClose: () => void;
	open: boolean;
};

export function AssetEditorModal({ assets, onAssetsChange, onClose, open }: AssetEditorModalProps) {
	const titleId = useId();
	const inputRef = useRef<HTMLInputElement>(null);
	const [rejections, setRejections] = useState<string[]>([]);
	const [dragActive, setDragActive] = useState(false);
	const [isProcessing, setIsProcessing] = useState(false);
	const validation = validateEditorAssets(assets);
	const totalBytes = assets.reduce((total, asset) => total + asset.size, 0);

	const addFiles = async (files: File[]) => {
		if (files.length === 0) {
			return;
		}

		setIsProcessing(true);
		try {
			const result = await createEditorAssets(files, assets);
			setRejections(result.rejected);

			if (result.accepted.length > 0) {
				onAssetsChange([...assets, ...result.accepted]);
			}
		} finally {
			setIsProcessing(false);
		}
	};

	const handleFileChange = (event: ChangeEvent<HTMLInputElement>) => {
		void addFiles(Array.from(event.target.files ?? []));
		event.target.value = "";
	};

	const handleDrop = (event: DragEvent<HTMLButtonElement>) => {
		event.preventDefault();
		setDragActive(false);
		void addFiles(Array.from(event.dataTransfer.files));
	};

	const handleRemove = (assetId: string) => {
		onAssetsChange(assets.filter((asset) => asset.id !== assetId));
	};

	return (
		<Dialog open={open} onOpenChange={(nextOpen) => !nextOpen && onClose()}>
			<DialogContent
				aria-labelledby={titleId}
				className="grid max-h-[86vh] grid-rows-[auto_minmax(0,1fr)] sm:max-w-4xl"
			>
				<DialogHeader>
					<DialogTitle id={titleId}>Asset Editor</DialogTitle>
					<DialogDescription>
						Add local files that should be packed into the exported .bbs package. Files stay in this browser session and
						are never uploaded to a server.
					</DialogDescription>
				</DialogHeader>

				<div className="min-h-0 space-y-4 overflow-y-auto pr-1">
					<div className="grid gap-3 md:grid-cols-[1fr_280px]">
						<input
							ref={inputRef}
							type="file"
							multiple
							accept={supportedAssetExtensions.map((extension) => `.${extension}`).join(",")}
							className="hidden"
							onChange={handleFileChange}
						/>
						<button
							type="button"
							className={cn(
								"grid min-h-40 place-items-center rounded-lg border border-dashed border-baud-border bg-baud-elevated p-5 text-center transition-colors outline-none hover:border-baud-line focus-visible:border-baud-red focus-visible:ring-3 focus-visible:ring-baud-red/20",
								dragActive && "border-baud-red bg-baud-red/10",
							)}
							onClick={() => inputRef.current?.click()}
							onDragOver={(event) => {
								event.preventDefault();
								setDragActive(true);
							}}
							onDragLeave={() => setDragActive(false)}
							onDrop={handleDrop}
							disabled={isProcessing}
						>
							<div className="space-y-3">
								<div className="mx-auto grid size-11 place-items-center rounded-lg bg-baud-soft text-baud-text">
									<Upload size={18} />
								</div>
								<div>
									<p className="font-semibold text-baud-text">
										{isProcessing ? "Checking assets..." : "Drop assets here"}
									</p>
									<p className="mt-1 text-sm leading-5 text-baud-muted">
										Files are checked by package path, browser media type, and content signature before being added.
									</p>
								</div>
								<span className="mx-auto inline-flex h-8 items-center justify-center gap-1.5 rounded-lg border border-baud-blue/25 bg-[#11172a] px-2.5 text-sm font-medium text-white">
									<Plus size={14} />
									{isProcessing ? "Scanning files" : "Add local files"}
								</span>
							</div>
						</button>

						<div className="rounded-lg border border-baud-border bg-baud-elevated p-4">
							<div className="flex items-center gap-2 text-sm font-semibold text-baud-text">
								<PackageCheck size={15} />
								Package checks
							</div>
							<div className="mt-3 space-y-2 text-sm text-baud-muted">
								<InfoRow label="Assets" value={`${assets.length} attached`} />
								<InfoRow label="Total size" value={formatBytes(totalBytes)} />
								<InfoRow label="Size policy" value="No fixed editor cap" />
							</div>
							<div className="mt-4 flex flex-wrap gap-1.5">
								{supportedAssetExtensions.map((extension) => (
									<Badge key={extension} variant="outline" className="font-mono text-xs">
										.{extension}
									</Badge>
								))}
							</div>
						</div>
					</div>

					{(rejections.length > 0 || validation.errors.length > 0) && (
						<div className="rounded border border-baud-danger/35 bg-baud-danger/10 px-4 py-3 text-sm text-baud-danger">
							<div className="font-semibold">Some assets need attention</div>
							<ul className="mt-2 list-disc space-y-1 pl-5">
								{[...validation.errors, ...rejections].map((message) => (
									<li key={message}>{message}</li>
								))}
							</ul>
						</div>
					)}

					<div className="rounded-lg border border-baud-border bg-baud-elevated">
						<div className="flex items-center justify-between gap-3 border-b border-baud-border px-4 py-3">
							<h3 className="text-sm font-bold tracking-[0.16em] text-baud-muted uppercase">Package Assets</h3>
							<Badge variant="outline">{assets.length} files</Badge>
						</div>
						{assets.length === 0 ? (
							<div className="px-4 py-8 text-center text-sm text-baud-muted">
								No assets added yet. Assets you add here will be exported under the assets folder.
							</div>
						) : (
							<div className="divide-y divide-baud-border">
								{assets.map((asset) => (
									<AssetRow key={asset.id} asset={asset} onRemove={() => handleRemove(asset.id)} />
								))}
							</div>
						)}
					</div>
				</div>
			</DialogContent>
		</Dialog>
	);
}

function AssetRow({ asset, onRemove }: { asset: EditorAsset; onRemove: () => void }) {
	const Icon = getAssetIcon(asset.kind);

	return (
		<div className="grid grid-cols-[auto_minmax(0,1fr)_auto] items-center gap-3 px-4 py-3">
			<div className="grid size-9 place-items-center rounded-lg border border-baud-border bg-baud-soft text-baud-muted">
				<Icon size={16} />
			</div>
			<div className="min-w-0">
				<div className="flex min-w-0 items-center gap-2">
					<span className="truncate font-semibold text-baud-text">{asset.name}</span>
					<Badge variant="outline" className="shrink-0 text-xs">
						{asset.kind}
					</Badge>
				</div>
				<div className="mt-1 flex min-w-0 flex-wrap gap-x-3 gap-y-1 font-mono text-xs text-baud-muted">
					<span className="truncate">{asset.packagePath}</span>
					<span>{formatBytes(asset.size)}</span>
					<span className="truncate">{asset.mediaType}</span>
				</div>
			</div>
			<Button type="button" aria-label={`Remove ${asset.name}`} onClick={onRemove} size="icon-sm" variant="icon">
				<Trash2 size={14} />
			</Button>
		</div>
	);
}

function InfoRow({ label, value }: { label: string; value: string }) {
	return (
		<div className="flex items-center justify-between gap-3">
			<span>{label}</span>
			<span className="font-mono text-baud-text">{value}</span>
		</div>
	);
}

function getAssetIcon(kind: AssetKind) {
	if (kind === "audio") {
		return FileAudio2;
	}

	if (kind === "image") {
		return FileImage;
	}

	return FileText;
}
