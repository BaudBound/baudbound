"use client";

import { BookOpenText, Keyboard, LifeBuoy, type LucideIcon, MousePointer2, Variable } from "lucide-react";
import { useState } from "react";
import { Dialog, DialogContent, DialogDescription, DialogHeader, DialogTitle } from "@/components/ui/dialog";
import { runtimeErrorFields } from "@/data/nodes/node-definition";
import { builtInVariableGroups } from "@/data/project/built-in-variables";

type HelpModalProps = {
	onClose: () => void;
	open: boolean;
};

const shortcutRows = [
	{ keys: "Ctrl / Cmd + C", description: "Copy the selected node." },
	{ keys: "Ctrl / Cmd + V", description: "Paste the copied node at the center of the current canvas view." },
	{ keys: "Delete / Backspace", description: "Delete the selected node or connection." },
];

const contextMenuRows = [
	{ target: "Node", action: "Right click a node to copy, duplicate, or delete it." },
	{ target: "Connection", action: "Right click a connection to disconnect it." },
	{ target: "Canvas", action: "Right click empty canvas space to paste a copied node when one is available." },
];

const referenceFormatRows = [
	{
		label: "Saved variable",
		pattern: "{{variable_name}}",
		example: "{{status}}",
		description: "Reads a value saved by Set Variable or provided by the runner.",
	},
	{
		label: "Node output",
		pattern: "{{node-id.output_name}}",
		example: "{{n-mr3zyt6f-12.status_code}}",
		description:
			"Reads read-only output data produced by a previous node. Replace node-id with the actual source node id.",
	},
	{
		label: "Nested data",
		pattern: "{{variable_or_node.path.to.field}}",
		example: "{{n-mr3zyt6f-12.json.user.name}}",
		description: "Reads nested object fields from saved variables or node output data with dot notation.",
	},
	{
		label: "List item",
		pattern: "{{variable_or_node.list[0]}}",
		example: "{{player_data.groups[0].members}}",
		description: "Reads list items by index. For Each can use the same format to loop a nested list.",
	},
	{
		label: "Bracket lookup",
		pattern: '{{variable_or_node["key-name"]}}',
		example: '{{n-mr3zyt6f-12.headers["content-type"]}}',
		description: "Reads keys that contain dashes, spaces, or other special characters.",
	},
	{
		label: "Built-in value",
		pattern: "{{built_in_name}}",
		example: "{{manifest_name}}",
		description: "Reads an always-available read-only manifest or system value.",
	},
];

const referenceRuleRows = [
	"Use double braces for saved variables, built-in values, and node runtime data.",
	"Do not add spaces inside reference braces.",
	"Replace node-id with the real node id, for example n-mr3zyt6f-12.",
	"Built-in variables and node output variables are read-only and cannot be changed with Set Variable.",
	"User variable names cannot start with manifest_ or system_; those prefixes are reserved for built-ins.",
	"Node output references only have data after that node has executed in the current run.",
];

const staticHelpSections = [
	{ id: "controls", label: "Controls", icon: Keyboard },
	{ id: "references", label: "References", icon: BookOpenText },
];

export function HelpModal({ onClose, open }: HelpModalProps) {
	const [activeSection, setActiveSection] = useState("controls");
	const helpSections = [
		...staticHelpSections,
		...builtInVariableGroups.map((group) => ({
			id: getBuiltInGroupSectionId(group.label),
			label: `${group.label} Variables`,
			icon: Variable,
		})),
	];
	const activeBuiltInGroup = builtInVariableGroups.find(
		(group) => getBuiltInGroupSectionId(group.label) === activeSection,
	);

	return (
		<Dialog open={open} onOpenChange={(nextOpen) => !nextOpen && onClose()}>
			<DialogContent
				className="h-[86vh] max-h-[86vh] overflow-hidden p-0 sm:max-w-6xl"
				onInteractOutside={(event) => event.preventDefault()}
			>
				<div className="grid h-full min-h-0 grid-rows-[auto_minmax(0,1fr)]">
					<DialogHeader className="border-b border-baud-border px-6 py-5">
						<div className="flex items-center gap-3">
							<div className="grid size-9 place-items-center rounded border border-baud-red/35 bg-baud-red/10 text-baud-red">
								<LifeBuoy size={18} />
							</div>
							<div>
								<DialogTitle className="text-lg text-baud-text">Editor Help</DialogTitle>
								<DialogDescription>
									Keyboard shortcuts, variable references, and always-available built-in values.
								</DialogDescription>
							</div>
						</div>
					</DialogHeader>

					<div className="grid min-h-0 overflow-hidden grid-cols-1 md:grid-cols-[240px_1fr]">
						<nav className="border-b border-baud-border bg-baud-bg/45 p-3 md:overflow-y-auto md:border-r md:border-b-0">
							<div className="mb-2 px-2 text-xs font-bold tracking-[0.18em] text-baud-muted uppercase">Docs</div>
							<div className="flex gap-2 overflow-x-auto md:flex-col md:overflow-visible">
								{helpSections.map((section) => (
									<HelpNavButton
										key={section.id}
										active={activeSection === section.id}
										icon={section.icon}
										label={section.label}
										onClick={() => setActiveSection(section.id)}
									/>
								))}
							</div>
						</nav>

						<div className="min-h-0 overflow-y-scroll px-6 py-5 [scrollbar-gutter:stable]">
							{activeSection === "controls" && <ControlsSection />}
							{activeSection === "references" && <ReferencesSection />}
							{activeBuiltInGroup && <BuiltInVariableSection group={activeBuiltInGroup} />}
						</div>
					</div>
				</div>
			</DialogContent>
		</Dialog>
	);
}

function HelpNavButton({
	active,
	icon: Icon,
	label,
	onClick,
}: {
	active: boolean;
	icon: LucideIcon;
	label: string;
	onClick: () => void;
}) {
	return (
		<button
			type="button"
			onClick={onClick}
			className={`flex h-10 shrink-0 items-center gap-2 rounded-md px-3 text-left text-sm font-semibold transition-colors ${
				active
					? "border border-baud-red/35 bg-baud-red/10 text-baud-text"
					: "border border-transparent text-baud-muted hover:bg-baud-elevated hover:text-baud-text"
			}`}
		>
			<Icon size={15} className={active ? "text-baud-red" : "text-baud-muted"} />
			<span className="whitespace-nowrap">{label}</span>
		</button>
	);
}

function ControlsSection() {
	return (
		<section className="space-y-6">
			<div className="space-y-3">
				<SectionTitle icon={Keyboard} title="Hotkeys" />
				<div className="grid gap-2">
					{shortcutRows.map((row) => (
						<div
							key={row.keys}
							className="grid gap-3 rounded-lg border border-baud-border bg-baud-elevated px-4 py-3 text-sm sm:grid-cols-[180px_1fr]"
						>
							<kbd className="font-mono font-semibold text-baud-text">{row.keys}</kbd>
							<p className="text-baud-muted">{row.description}</p>
						</div>
					))}
				</div>
			</div>

			<div className="space-y-3">
				<SectionTitle icon={MousePointer2} title="Context Menus" />
				<div className="grid gap-2">
					{contextMenuRows.map((row) => (
						<div
							key={row.target}
							className="grid gap-3 rounded-lg border border-baud-border bg-baud-elevated px-4 py-3 text-sm sm:grid-cols-[180px_1fr]"
						>
							<span className="font-semibold text-baud-text">{row.target}</span>
							<p className="text-baud-muted">{row.action}</p>
						</div>
					))}
				</div>
			</div>
		</section>
	);
}

function ReferencesSection() {
	return (
		<section className="space-y-6">
			<div className="space-y-3">
				<SectionTitle icon={BookOpenText} title="Reference Formats" />
				<div className="rounded-lg border border-baud-border bg-baud-elevated p-4 text-sm leading-6 text-baud-muted">
					<p>
						Runtime references point at data from a specific node. Select the source node, copy its id from Properties,
						then combine it with an output name from that node&apos;s Runtime Data section.
					</p>
					<p className="mt-2">
						Example: an HTTP Request node with id <Code>{"n-mr3zyt6f-12"}</Code> exposes <Code>{"status_code"}</Code>,
						so the reference is <Code>{"{{n-mr3zyt6f-12.status_code}}"}</Code>.
					</p>
				</div>
				<div className="rounded-lg border border-baud-border bg-baud-elevated">
					<div className="grid gap-3 border-b border-baud-border px-4 py-2 text-xs font-bold tracking-[0.16em] text-baud-muted uppercase sm:grid-cols-[150px_minmax(150px,230px)_minmax(150px,230px)_minmax(0,1fr)]">
						<span>Type</span>
						<span>Pattern</span>
						<span>Example</span>
						<span>Description</span>
					</div>
					<div className="divide-y divide-baud-border">
						{referenceFormatRows.map((row) => (
							<div
								key={row.label}
								className="grid gap-3 px-4 py-3 text-sm sm:grid-cols-[150px_minmax(150px,230px)_minmax(150px,230px)_minmax(0,1fr)]"
							>
								<span className="font-semibold text-baud-text">{row.label}</span>
								<Code>{row.pattern}</Code>
								<Code>{row.example}</Code>
								<p className="text-baud-muted">{row.description}</p>
							</div>
						))}
					</div>
				</div>
			</div>

			<div className="space-y-3">
				<SectionTitle icon={Variable} title="Failure References" />
				<div className="rounded-lg border border-baud-border bg-baud-elevated">
					<div className="border-b border-baud-border px-4 py-3 text-sm text-baud-muted">
						Nodes with success and failed outputs expose read-only <Code>{"{{node-id.error.*}}"}</Code> values when
						execution continues through the failed output.
					</div>
					<div className="divide-y divide-baud-border">
						{runtimeErrorFields.map((field) => (
							<div
								key={field.name}
								className="grid gap-3 px-4 py-3 text-sm sm:grid-cols-[160px_minmax(0,1fr)_minmax(190px,260px)]"
							>
								<span className="font-mono text-baud-text">{field.name}</span>
								<p className="text-baud-muted">{field.description}</p>
								<Code>{`{{${field.example ?? `n-mr3zyt6f-12.error.${field.name}`}}}`}</Code>
							</div>
						))}
					</div>
				</div>
			</div>

			<div className="space-y-3">
				<SectionTitle icon={Variable} title="Reference Rules" />
				<div className="grid gap-2">
					{referenceRuleRows.map((rule) => (
						<div
							key={rule}
							className="rounded-lg border border-baud-border bg-baud-elevated px-4 py-3 text-sm text-baud-muted"
						>
							{rule}
						</div>
					))}
				</div>
			</div>
		</section>
	);
}

function BuiltInVariableSection({ group }: { group: (typeof builtInVariableGroups)[number] }) {
	return (
		<section className="space-y-4">
			<SectionTitle icon={Variable} title={`${group.label} Variables`} />
			<div className="rounded-lg border border-baud-border bg-baud-elevated">
				<div className="border-b border-baud-border px-4 py-3">
					<h3 className="font-semibold text-baud-text">{group.label}</h3>
					<p className="mt-1 text-sm text-baud-muted">{group.description}</p>
				</div>
				<div className="divide-y divide-baud-border">
					{group.variables.map((variable) => (
						<div
							key={variable.name}
							className="grid min-w-0 gap-3 px-4 py-3 text-sm sm:grid-cols-[minmax(180px,220px)_minmax(0,1fr)_minmax(120px,160px)_90px]"
						>
							<Code>{variable.token}</Code>
							<p className="min-w-0 text-baud-muted">{variable.description}</p>
							<span className="min-w-0 break-words font-mono text-baud-text/80">{variable.example}</span>
							<span className="font-mono text-baud-muted">read-only</span>
						</div>
					))}
				</div>
			</div>
		</section>
	);
}

function SectionTitle({ icon: Icon, title }: { icon: LucideIcon; title: string }) {
	return (
		<div className="flex items-center gap-2">
			<Icon size={16} className="text-baud-red" />
			<h2 className="text-sm font-bold tracking-[0.18em] text-baud-muted uppercase">{title}</h2>
		</div>
	);
}

function Code({ children }: { children: string }) {
	return (
		<code className="max-w-full break-words rounded border border-baud-border bg-baud-bg px-1.5 py-0.5 font-mono text-baud-text">
			{children}
		</code>
	);
}

function getBuiltInGroupSectionId(label: string) {
	return `built-in-${label.toLowerCase()}`;
}
