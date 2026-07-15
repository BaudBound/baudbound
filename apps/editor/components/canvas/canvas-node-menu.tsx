import { Clipboard, Search } from "lucide-react";
import { useMemo, useState } from "react";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { getPaletteGroups, getTargetRuntimeCompatibilityErrors } from "@/data/nodes/registry";
import type { PaletteGroup, PaletteItem, TargetRuntime } from "@/lib/types";

type CanvasNodeMenuProps = {
	canPaste: boolean;
	targetRuntime: TargetRuntime;
	onAddNode: (item: PaletteItem) => void;
	onPaste: () => void;
};

type NodeCategory = Pick<PaletteGroup, "id" | "items" | "label">;

const nodeCategories = getPaletteGroups().flatMap(toNodeCategories);

export function CanvasNodeMenu({ canPaste, onAddNode, onPaste, targetRuntime }: CanvasNodeMenuProps) {
	const [query, setQuery] = useState("");
	const filteredCategories = useMemo(() => filterCategories(nodeCategories, query), [query]);
	const firstAvailableItem = useMemo(
		() =>
			filteredCategories
				.flatMap((category) => category.items)
				.find((item) => getCompatibilityErrors(item, targetRuntime).length === 0),
		[filteredCategories, targetRuntime],
	);

	return (
		<div
			aria-label="Add node"
			className="flex max-h-[min(32rem,calc(100vh-1rem))] w-88 flex-col overflow-hidden rounded border border-baud-border bg-baud-panel shadow-[0_18px_48px_rgba(0,0,0,0.42)]"
			role="dialog"
			onContextMenu={(event) => event.preventDefault()}
			onPointerDown={(event) => event.stopPropagation()}
		>
			<div className="flex shrink-0 items-center gap-2 border-b border-baud-border p-2">
				<div className="relative min-w-0 flex-1">
					<Search
						aria-hidden="true"
						className="pointer-events-none absolute left-2.5 top-1/2 -translate-y-1/2 text-baud-muted"
						size={14}
					/>
					<Input
						autoFocus
						aria-label="Search nodes"
						className="pl-8"
						placeholder="Search nodes..."
						value={query}
						onChange={(event) => setQuery(event.target.value)}
						onKeyDown={(event) => {
							if (event.key === "Enter" && firstAvailableItem) {
								event.preventDefault();
								onAddNode(firstAvailableItem);
							}
						}}
					/>
				</div>
				<Button
					type="button"
					aria-label="Paste copied node"
					disabled={!canPaste}
					onClick={onPaste}
					size="sm"
					title={canPaste ? "Paste copied node" : "Copy a node before pasting"}
					variant="toolbar"
				>
					<Clipboard size={14} />
					Paste
				</Button>
			</div>

			<div className="min-h-0 overflow-y-auto overflow-x-hidden p-2">
				{filteredCategories.length === 0 ? (
					<p className="px-2 py-8 text-center text-sm text-baud-muted">No nodes match your search.</p>
				) : (
					filteredCategories.map((category) => (
						<NodeCategorySection
							key={category.id}
							category={category}
							onAddNode={onAddNode}
							targetRuntime={targetRuntime}
						/>
					))
				)}
			</div>
		</div>
	);
}

function NodeCategorySection({
	category,
	onAddNode,
	targetRuntime,
}: {
	category: NodeCategory;
	onAddNode: (item: PaletteItem) => void;
	targetRuntime: TargetRuntime;
}) {
	return (
		<section aria-labelledby={`canvas-node-category-${category.id}`} className="pb-2 last:pb-0">
			<div className="flex items-center gap-2 px-2 py-1.5">
				<h3
					className="shrink-0 text-[0.65rem] font-bold uppercase tracking-[0.14em] text-baud-muted"
					id={`canvas-node-category-${category.id}`}
				>
					{category.label}
				</h3>
				<span aria-hidden="true" className="h-px flex-1 bg-baud-border" />
			</div>
			<div className="grid gap-0.5">
				{category.items.map((item) => (
					<CanvasNodeButton key={item.actionType} item={item} onAddNode={onAddNode} targetRuntime={targetRuntime} />
				))}
			</div>
		</section>
	);
}

function CanvasNodeButton({
	item,
	onAddNode,
	targetRuntime,
}: {
	item: PaletteItem;
	onAddNode: (item: PaletteItem) => void;
	targetRuntime: TargetRuntime;
}) {
	const Icon = item.icon;
	const compatibilityErrors = getCompatibilityErrors(item, targetRuntime);
	const unavailable = compatibilityErrors.length > 0;

	return (
		<Button
			type="button"
			className="h-auto min-w-0 justify-start gap-2 rounded px-2 py-1.5 text-left font-normal"
			disabled={unavailable}
			onClick={() => onAddNode(item)}
			title={unavailable ? compatibilityErrors.join(" ") : item.description}
			variant="ghost"
		>
			<Icon className="text-baud-muted" size={14} />
			<span className="min-w-0 flex-1">
				<span className="block truncate text-sm text-baud-text">{item.label}</span>
				<span className="block truncate text-xs text-baud-muted">{item.description}</span>
			</span>
		</Button>
	);
}

function getCompatibilityErrors(item: PaletteItem, targetRuntime: TargetRuntime) {
	return getTargetRuntimeCompatibilityErrors(
		[{ actionType: item.actionType, id: item.label, label: item.label }],
		targetRuntime,
	);
}

function toNodeCategories(group: PaletteGroup): NodeCategory[] {
	const categories = group.items.length > 0 ? [{ id: group.id, items: group.items, label: group.label }] : [];
	return [...categories, ...(group.children?.flatMap(toNodeCategories) ?? [])];
}

function filterCategories(categories: NodeCategory[], query: string) {
	const normalizedQuery = query.trim().toLowerCase();
	if (!normalizedQuery) {
		return categories;
	}

	return categories.flatMap((category) => {
		const items = category.items.filter((item) =>
			`${category.label} ${item.label} ${item.description} ${item.actionType}`.toLowerCase().includes(normalizedQuery),
		);
		return items.length > 0 ? [{ ...category, items }] : [];
	});
}
