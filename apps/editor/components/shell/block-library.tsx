import { ChevronDown, ChevronRight, Search } from "lucide-react";
import { useMemo, useState } from "react";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { desktopOnlyActionTypes, getPaletteGroups } from "@/data/nodes/registry";
import type { PaletteItem } from "@/lib/types";
import { RiskBadge } from "./risk-badge";

type BlockLibraryProps = {
	isDesktopTarget: boolean;
	width: number;
	onAddBlock: (item: PaletteItem) => void;
};

const defaultExpandedPaletteGroups: Record<string, boolean> = {
	triggers: true,
	control: true,
	actions: true,
};

const paletteGroups = getPaletteGroups();

export function BlockLibrary({ isDesktopTarget, width, onAddBlock }: BlockLibraryProps) {
	const [query, setQuery] = useState("");
	const [expandedGroups, setExpandedGroups] = useState<Record<string, boolean>>(defaultExpandedPaletteGroups);
	const compact = width < 190;

	const filteredGroups = useMemo(() => {
		const normalizedQuery = query.trim().toLowerCase();
		return paletteGroups
			.map((group) => ({
				...group,
				items: group.items.filter((item) => {
					if (!normalizedQuery) {
						return true;
					}

					return `${item.label} ${item.description} ${item.actionType}`.toLowerCase().includes(normalizedQuery);
				}),
			}))
			.filter((group) => group.items.length > 0);
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
					const Icon = group.icon;
					const expanded = expandedGroups[group.id];

					return (
						<section key={group.id} className="mb-3">
							<Button
								type="button"
								onClick={() => setExpandedGroups((current) => ({ ...current, [group.id]: !expanded }))}
								className="w-full justify-start px-1.5 text-left text-baud-muted font-bold uppercase tracking-[0.18em] hover:bg-transparent hover:text-baud-text"
								size="sm"
								variant="ghost"
							>
								{expanded ? <ChevronDown size={13} /> : <ChevronRight size={13} />}
								<Icon size={13} />
								<span>{group.label}</span>
							</Button>

							{expanded && (
								<div className="mt-1 ml-3 space-y-1 border-l border-baud-border/80 pl-2">
									{group.items.map((item) => {
										const Icon = item.icon;
										const unavailable = desktopOnlyActionTypes.has(item.actionType) && !isDesktopTarget;

										return (
											<Button
												type="button"
												key={item.actionType}
												onClick={() => onAddBlock(item)}
												className="group w-full justify-start gap-2 px-2 text-left text-[0.9rem] font-normal"
												disabled={unavailable}
												title={unavailable ? "Requires a desktop target runtime" : item.description}
												variant="ghost"
											>
												<Icon size={13} className="text-baud-muted group-hover:text-baud-text" />
												<span className="min-w-0 flex-1 truncate">{item.label}</span>
												{!compact && <RiskBadge risk={item.risk} />}
											</Button>
										);
									})}
								</div>
							)}
						</section>
					);
				})}
			</div>
		</aside>
	);
}
