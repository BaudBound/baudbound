import { ChevronDown, ChevronRight, Search } from "lucide-react";
import { useMemo, useState } from "react";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { paletteNodeDragDataType } from "@/data/editor/drag-drop";
import { getPaletteGroups, getTargetRuntimeCompatibilityErrors } from "@/data/nodes/registry";
import type { PaletteGroup, PaletteItem, TargetRuntime } from "@/lib/types";
import { RiskBadge } from "./risk-badge";

type BlockLibraryProps = {
	targetRuntime: TargetRuntime;
	width: number;
	onAddBlock: (item: PaletteItem) => void;
};

const defaultExpandedPaletteGroups: Record<string, boolean> = {
	triggers: true,
	control: true,
	actions: true,
};

const paletteGroups = getPaletteGroups();
const defaultExpandedGroups = createDefaultExpandedGroups(paletteGroups);

export function BlockLibrary({ targetRuntime, width, onAddBlock }: BlockLibraryProps) {
	const [query, setQuery] = useState("");
	const [expandedGroups, setExpandedGroups] = useState<Record<string, boolean>>(defaultExpandedGroups);
	const compact = width < 190;

	const filteredGroups = useMemo(() => {
		const normalizedQuery = query.trim().toLowerCase();
		return paletteGroups.flatMap((group) => filterPaletteGroup(group, normalizedQuery));
	}, [query]);

	return (
		<aside className="flex shrink-0 flex-col border-r border-baud-border bg-baud-panel" style={{ width }}>
			<div className="border-b border-baud-border p-3">
				<div className="relative">
					<Search
						className="pointer-events-none absolute left-2.5 top-1/2 -translate-y-1/2 text-baud-muted"
						size={14}
					/>
					<Input
						value={query}
						onChange={(event) => setQuery(event.target.value)}
						aria-label="Search blocks"
						placeholder={compact ? "Search..." : "Search blocks..."}
						className="pl-8 pr-3 font-sans"
					/>
				</div>
			</div>

			<div className="min-h-0 flex-1 overflow-y-auto px-2 py-3">
				{filteredGroups.map((group) => {
					return (
						<PaletteGroupSection
							key={group.id}
							compact={compact}
							expandedGroups={expandedGroups}
							forceExpanded={query.trim().length > 0}
							group={group}
							targetRuntime={targetRuntime}
							onAddBlock={onAddBlock}
							onToggleGroup={(groupId) =>
								setExpandedGroups((current) => ({ ...current, [groupId]: !current[groupId] }))
							}
						/>
					);
				})}
			</div>
		</aside>
	);
}

function PaletteGroupSection({
	compact,
	expandedGroups,
	forceExpanded,
	group,
	nested = false,
	onAddBlock,
	onToggleGroup,
	targetRuntime,
}: {
	compact: boolean;
	expandedGroups: Record<string, boolean>;
	forceExpanded: boolean;
	group: PaletteGroup;
	nested?: boolean;
	onAddBlock: (item: PaletteItem) => void;
	onToggleGroup: (groupId: string) => void;
	targetRuntime: TargetRuntime;
}) {
	const Icon = group.icon;
	const expanded = forceExpanded || (expandedGroups[group.id] ?? false);
	const hasChildren = group.items.length > 0 || (group.children?.length ?? 0) > 0;

	return (
		<section className={nested ? "mt-2" : "mb-3"}>
			<Button
				type="button"
				onClick={() => onToggleGroup(group.id)}
				className={`w-full justify-start px-1.5 text-left font-bold uppercase hover:bg-transparent hover:text-baud-text ${
					nested ? "text-[0.68rem] tracking-[0.12em] text-baud-muted/90" : "tracking-[0.18em] text-baud-muted"
				}`}
				disabled={!hasChildren}
				size="sm"
				variant="ghost"
			>
				{expanded ? <ChevronDown size={13} /> : <ChevronRight size={13} />}
				<Icon size={13} />
				<span className="min-w-0 truncate">{group.label}</span>
			</Button>

			{expanded && (
				<div className="mt-1 ml-3 space-y-1 border-l border-baud-border/80 pl-2">
					{group.children?.map((child) => (
						<PaletteGroupSection
							key={child.id}
							compact={compact}
							expandedGroups={expandedGroups}
							forceExpanded={forceExpanded}
							group={child}
							nested
							onAddBlock={onAddBlock}
							onToggleGroup={onToggleGroup}
							targetRuntime={targetRuntime}
						/>
					))}
					{group.items.map((item) => (
						<PaletteItemButton
							key={item.actionType}
							compact={compact}
							item={item}
							onAddBlock={onAddBlock}
							targetRuntime={targetRuntime}
						/>
					))}
				</div>
			)}
		</section>
	);
}

function PaletteItemButton({
	compact,
	item,
	onAddBlock,
	targetRuntime,
}: {
	compact: boolean;
	item: PaletteItem;
	onAddBlock: (item: PaletteItem) => void;
	targetRuntime: TargetRuntime;
}) {
	const Icon = item.icon;
	const compatibilityErrors = getTargetRuntimeCompatibilityErrors(
		[{ actionType: item.actionType, id: item.label, label: item.label }],
		targetRuntime,
	);
	const unavailable = compatibilityErrors.length > 0;

	return (
		<Button
			type="button"
			onClick={() => onAddBlock(item)}
			draggable={!unavailable}
			onDragStart={(event) => {
				if (unavailable) {
					event.preventDefault();
					return;
				}

				event.dataTransfer.effectAllowed = "copy";
				event.dataTransfer.setData(paletteNodeDragDataType, item.actionType);
				event.dataTransfer.setData("text/plain", item.actionType);
			}}
			className="group w-full justify-start gap-2 px-2 text-left text-[0.9rem] font-normal"
			disabled={unavailable}
			title={unavailable ? compatibilityErrors.join(" ") : item.description}
			variant="ghost"
		>
			<Icon size={13} className="text-baud-muted group-hover:text-baud-text" />
			<span className="min-w-0 flex-1 truncate">{item.label}</span>
			{!compact && <RiskBadge risk={item.risk} />}
		</Button>
	);
}

function createDefaultExpandedGroups(groups: PaletteGroup[]) {
	return groups.reduce<Record<string, boolean>>((expanded, group) => {
		expanded[group.id] = defaultExpandedPaletteGroups[group.id] ?? false;
		for (const child of group.children ?? []) {
			expanded[child.id] = false;
		}
		return expanded;
	}, {});
}

function filterPaletteGroup(group: PaletteGroup, normalizedQuery: string): PaletteGroup[] {
	const items = group.items.filter((item) => {
		if (!normalizedQuery) {
			return true;
		}

		return `${item.label} ${item.description} ${item.actionType}`.toLowerCase().includes(normalizedQuery);
	});
	const children = group.children?.flatMap((child) => filterPaletteGroup(child, normalizedQuery));

	if (items.length === 0 && (!children || children.length === 0)) {
		return [];
	}

	return [
		{
			...group,
			items,
			children,
		},
	];
}
