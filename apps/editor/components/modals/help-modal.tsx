"use client";

import {
	BookOpenText,
	Calculator,
	Database,
	FlaskConical,
	Keyboard,
	LifeBuoy,
	type LucideIcon,
	MousePointer2,
	PackageCheck,
	StickyNote,
	Variable,
} from "lucide-react";
import type { ReactNode } from "react";
import { useState } from "react";
import { Dialog, DialogContent, DialogDescription, DialogHeader, DialogTitle } from "@/components/ui/dialog";
import { runtimeErrorFields } from "@/data/nodes/node-definition";
import { getNodeConfigFields, getPaletteGroups, getRuntimeDataOutputs } from "@/data/nodes/registry";
import { builtInVariableGroups } from "@/data/project/built-in-variables";
import { variableOperationDefinitions, variableTypeDefinitions } from "@/data/project/variables";

type HelpModalProps = {
	onClose: () => void;
	open: boolean;
};

const shortcutRows = [
	{ keys: "Ctrl / Cmd + C", description: "Copy the selected node." },
	{ keys: "Ctrl / Cmd + V", description: "Paste the copied node at the center of the current canvas view." },
	{ keys: "Ctrl / Cmd + drag", description: "Box-select nodes and comments from empty canvas space." },
	{ keys: "Delete / Backspace", description: "Delete the selected node or connection." },
];

const contextMenuRows = [
	{ target: "Node", action: "Right click a node to copy, duplicate, or delete it." },
	{ target: "Connection", action: "Right click a connection to disconnect it." },
	{ target: "Canvas", action: "Right click empty canvas space to paste a copied node when one is available." },
];

const canvasToolRows = [
	{
		icon: StickyNote,
		tool: "Add comment",
		description:
			"Adds an editor-only comment at the center of the current canvas view. Comments are saved in editor metadata and do not run.",
	},
];

const referenceFormatRows = [
	{
		label: "Saved variable",
		pattern: "{{variable_name}}",
		example: "{{status}}",
		description: "Reads a value saved by Variable Operation or provided by the runner.",
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
		label: "Derived metadata",
		pattern: "{{variable_or_node.path.$length}}",
		example: "{{n-mr3zyt6f-12.json.players.$length}}",
		description:
			"Reads generated facts such as length, count, type, or empty state from variables, nested data, or node outputs.",
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
	"Use derived fields for generated value facts, for example {{foo.$length}} or {{node-id.output.$count}}. Plain .length and .count always mean real data fields.",
	"Built-in variables and node output variables are read-only and cannot be changed with Variable Operation.",
	"User variable names cannot start with manifest_ or system_; those prefixes are reserved for built-ins.",
	"Node output references only have data after that node has executed in the current run.",
];

const expressionOperatorRows = [
	{ syntax: "+", description: "Add numbers." },
	{ syntax: "-", description: "Subtract numbers or negate a value." },
	{ syntax: "*", description: "Multiply numbers." },
	{ syntax: "/", description: "Divide numbers. Division by zero is rejected." },
	{ syntax: "%", description: "Remainder after division." },
	{ syntax: "^", description: "Exponent power, for example 2 ^ 3." },
	{ syntax: "()", description: "Group parts of an expression." },
];

const calculationFunctionRows = [
	{ syntax: "round(value)", description: "Round to the nearest whole number.", example: "round(3.6)" },
	{ syntax: "floor(value)", description: "Round down.", example: "floor(3.9)" },
	{ syntax: "ceil(value)", description: "Round up.", example: "ceil(3.1)" },
	{ syntax: "min(a, b, ...)", description: "Return the smallest value.", example: "min(3, 8, 2)" },
	{ syntax: "max(a, b, ...)", description: "Return the largest value.", example: "max(3, 8, 2)" },
	{ syntax: "random()", description: "Random number between 0 and 1.", example: "random()" },
	{ syntax: "random(max)", description: "Random number from 0 up to max.", example: "random(10)" },
	{ syntax: "random(min, max)", description: "Random number between min and max.", example: "random(5, 15)" },
];

const derivedMetadataRows = [
	["$length", "String characters, list items, object keys, or 0 for scalar values."],
	["$count", "Alias for $length."],
	["$type", "Value type such as string, number, boolean, list, object, null, or missing."],
	["$is_empty", "True for empty strings, empty lists, empty objects, null, or missing values."],
];

const conditionComparisonRows = [
	{ label: "Equal", description: "Passes when Value and Target are the same." },
	{ label: "Not equal", description: "Passes when Value and Target are different." },
	{ label: "Greater than / at least", description: "Numeric comparison against the Target value." },
	{ label: "Less than / at most", description: "Numeric comparison against the Target value." },
	{ label: "Contains", description: "Passes when Value text contains the Target text." },
	{ label: "Starts with", description: "Passes when Value text starts with the Target text." },
	{ label: "Ends with", description: "Passes when Value text ends with the Target text." },
	{ label: "Regex match", description: "Passes when Value matches the selected safe regex pattern." },
	{ label: "Is empty", description: "Passes when Value is empty text." },
	{ label: "Is null", description: "Passes when Value is null or the text null." },
];

const nodeBehaviorRows = [
	{
		label: "Success / failed outputs",
		description:
			"Fallible nodes continue through success when the operation works and failed when the runner reports an error.",
	},
	{
		label: "Runtime data",
		description:
			"Nodes can expose read-only runtime outputs. Select a node and open Runtime Data in Properties to see available fields.",
	},
	{
		label: "Custom node names",
		description:
			"Every node can have a custom display name. The node type remains visible so users can still identify what it does.",
	},
	{
		label: "Loop",
		description:
			"Runs the loop output branch once per iteration. The body branch should end naturally and must not connect back to the Loop input. The done output runs after all iterations complete.",
	},
	{
		label: "While",
		description:
			"Checks condition rows before every iteration. The loop output branch runs while the conditions pass and should end naturally. The done output runs when the conditions fail.",
	},
	{
		label: "For Each",
		description:
			"Loops through a list value. The loop output branch runs once for each item and should end naturally. The items field can reference a list variable or nested list such as {{payload.users}}.",
	},
	{
		label: "HTTP Request",
		description:
			"Simulation sends the request from the browser, so CORS and mixed-content browser rules still apply. The runner will execute it locally.",
	},
	{
		label: "Webhook responses",
		description:
			"Webhook Trigger can wait for a response node. Webhook Response sends the first response for that request; if no response is reached before timeout, the trigger fallback response is used.",
	},
	{
		label: "WebSocket Write",
		description:
			"WebSocket Write sends a message to an active connection id, usually from a WebSocket Trigger output such as {{node-id.connection_id}}.",
	},
	{
		label: "Assets",
		description:
			"Assets stay client-side in the editor and are packed into the exported package under assets/. File signatures are checked before import/export.",
	},
];

const simulationRows = [
	{
		label: "Verify before export",
		description:
			"Verification checks graph structure, variables, node configs, assets, serial device ids, package declarations, and export readiness.",
	},
	{
		label: "Simulation",
		description:
			"Simulation previews graph flow without a runner connection. Unsupported runner actions log what would happen instead of touching the local machine.",
	},
	{
		label: "Trigger inputs",
		description:
			"The Simulator tab lets you fire trigger nodes with sample payloads. Pressing a trigger verifies the script first, then starts that branch. Schedule triggers run from their configured interval while the simulator is active.",
	},
	{
		label: "Overrides",
		description:
			"Add a node override in the Simulator tab to force a fallible node through success or failed during simulation.",
	},
	{
		label: "Export package",
		description:
			"Export writes manifest.json, program.json, editor.json, permissions.json, capabilities.json, README.md, and assets/.",
	},
];

const staticHelpSections = [
	{ id: "controls", label: "Controls", icon: Keyboard },
	{ id: "references", label: "References", icon: BookOpenText },
	{ id: "expressions", label: "Expressions", icon: Calculator },
	{ id: "variables", label: "Variables", icon: Database },
	{ id: "nodes", label: "Nodes", icon: FlaskConical },
	{ id: "packages", label: "Simulation & Export", icon: PackageCheck },
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
									Editor controls, references, expressions, variables, simulation, and package behavior.
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
							{activeSection === "expressions" && <ExpressionsSection />}
							{activeSection === "variables" && <VariablesSection />}
							{activeSection === "nodes" && <NodesSection />}
							{activeSection === "packages" && <SimulationAndExportSection />}
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

			<div className="space-y-3">
				<SectionTitle icon={StickyNote} title="Comments" />
				<div className="grid gap-2">
					{canvasToolRows.map((row) => {
						const Icon = row.icon;

						return (
							<div
								key={row.tool}
								className="grid gap-3 rounded-lg border border-baud-border bg-baud-elevated px-4 py-3 text-sm sm:grid-cols-[180px_1fr]"
							>
								<span className="flex items-center gap-2 font-semibold text-baud-text">
									<Icon size={15} className="text-baud-red" />
									{row.tool}
								</span>
								<p className="text-baud-muted">{row.description}</p>
							</div>
						);
					})}
				</div>
			</div>
		</section>
	);
}

function ExpressionsSection() {
	return (
		<section className="space-y-6">
			<div className="space-y-3">
				<SectionTitle icon={Calculator} title="Calculate Node" />
				<InfoCard>
					The Calculate node evaluates numeric expressions and exposes the result as runtime data. Variable references
					are resolved before the expression is evaluated, so expressions can combine constants and runtime values.
				</InfoCard>
				<DocTable
					columns={["Syntax", "Example", "Description"]}
					rows={calculationFunctionRows.map((row) => [row.syntax, row.example, row.description])}
				/>
			</div>

			<div className="space-y-3">
				<SectionTitle icon={Calculator} title="Math Operators" />
				<DocTable
					columns={["Operator", "Description"]}
					rows={expressionOperatorRows.map((row) => [row.syntax, row.description])}
				/>
			</div>

			<div className="space-y-3">
				<SectionTitle icon={Calculator} title="If / Else Comparisons" />
				<InfoCard>
					If / Else and While do not use typed expression syntax. Each condition row has a Value field, a comparison
					dropdown, and a Target field. Enable Invert condition on a row to flip that row result before it is combined
					with AND or OR.
				</InfoCard>
				<DocTable
					columns={["Dropdown option", "Description"]}
					rows={conditionComparisonRows.map((row) => [row.label, row.description])}
				/>
			</div>
		</section>
	);
}

function VariablesSection() {
	return (
		<section className="space-y-6">
			<div className="space-y-3">
				<SectionTitle icon={Database} title="Variable Operations" />
				<InfoCard>
					Variable Operation creates or edits user-writable variables. Built-in variables and node output references are
					read-only, and variable names cannot start with <Code>{"manifest_"}</Code> or <Code>{"system_"}</Code>.
				</InfoCard>
				<DocTable
					columns={["Operation", "What it does"]}
					rows={Object.values(variableOperationDefinitions).map((definition) => [
						definition.label,
						definition.description,
					])}
				/>
			</div>

			<div className="space-y-3">
				<SectionTitle icon={Variable} title="Variable Types" />
				<DocTable
					columns={["Type", "Example", "Description"]}
					rows={Object.entries(variableTypeDefinitions).map(([type, definition]) => [
						type,
						definition.example,
						definition.description,
					])}
				/>
			</div>

			<div className="space-y-3">
				<SectionTitle icon={Variable} title="Scopes" />
				<div className="grid gap-2">
					<InfoCard>
						<Code>{"runtime"}</Code> exists for one script run. <Code>{"persistent"}</Code> is stored between runs.{" "}
						<Code>{"global"}</Code> is provided by the runner. <Code>{"secret"}</Code> is for sensitive encrypted
						values.
					</InfoCard>
				</div>
			</div>
		</section>
	);
}

function NodesSection() {
	const paletteGroups = getPaletteGroups();

	return (
		<section className="space-y-6">
			<div className="space-y-3">
				<SectionTitle icon={FlaskConical} title="Node Behavior" />
				<DocTable
					columns={["Feature", "Description"]}
					rows={nodeBehaviorRows.map((row) => [row.label, row.description])}
				/>
			</div>

			<div className="space-y-3">
				<SectionTitle icon={BookOpenText} title="Node Reference" />
				<div className="grid gap-3">
					{paletteGroups.map((group) => (
						<div key={group.id} className="rounded-lg border border-baud-border bg-baud-elevated">
							<div className="border-b border-baud-border px-4 py-3">
								<h3 className="font-semibold text-baud-text">{group.label}</h3>
							</div>
							<div className="divide-y divide-baud-border">
								<NodeReferenceGroup group={group} />
							</div>
						</div>
					))}
				</div>
			</div>

			<div className="space-y-3">
				<SectionTitle icon={BookOpenText} title="Common Runtime Outputs" />
				<div className="grid gap-2">
					<InfoCard>
						Target runtime compatibility is enforced before export and during import. Get Pixel Color, Get Active
						Window, and Window Focus currently require Windows Desktop because those runner actions use native Win32
						desktop APIs.
					</InfoCard>
					<InfoCard>
						HTTP Request exposes status code, status text, headers, body, parsed JSON, and duration. Webhook Trigger
						exposes request method, path, headers, query, body, JSON, and response state. Webhook Response exposes sent
						status, status code, content type, headers, body, and trigger id. WebSocket Trigger exposes path, connection
						id, headers, query, message, parsed JSON, and remote address. WebSocket Write exposes connection id,
						message, and bytes sent. File Read exposes path, content, and bytes. Get Pixel Color exposes hex, rgb, rgba,
						and channel values. Process, window, serial, sound, message box, and file nodes expose operation-specific
						data when relevant.
					</InfoCard>
				</div>
			</div>
		</section>
	);
}

function NodeReferenceGroup({
	group,
	nested = false,
}: {
	group: ReturnType<typeof getPaletteGroups>[number];
	nested?: boolean;
}) {
	return (
		<>
			{nested && (
				<div className="bg-baud-panel/40 px-4 py-2 text-xs font-bold tracking-[0.14em] text-baud-muted uppercase">
					{group.label}
				</div>
			)}
			{group.items.map((item) => (
				<NodeReferenceItem key={item.actionType} item={item} />
			))}
			{group.children?.map((child) => (
				<NodeReferenceGroup key={child.id} group={child} nested />
			))}
		</>
	);
}

function NodeReferenceItem({ item }: { item: ReturnType<typeof getPaletteGroups>[number]["items"][number] }) {
	const configFields = getNodeConfigFields(item.actionType);
	const runtimeOutputs = getRuntimeDataOutputs(item.actionType);

	return (
		<div className="grid gap-3 px-4 py-3 text-sm lg:grid-cols-[220px_minmax(0,1fr)]">
			<div className="min-w-0">
				<div className="font-semibold text-baud-text">{item.label}</div>
				<Code>{item.actionType}</Code>
				<div className="mt-2 font-mono text-xs uppercase text-baud-muted">risk {item.risk}</div>
			</div>
			<div className="min-w-0 space-y-2 text-baud-muted">
				<p>{item.description}</p>
				<NodeReferenceLine
					label="Config"
					value={
						configFields.length > 0 ? configFields.map((field) => field.label).join(", ") : "No editable config fields"
					}
				/>
				<NodeReferenceLine
					label="Runtime data"
					value={runtimeOutputs.length > 0 ? runtimeOutputs.map((output) => output.name).join(", ") : "No runtime data"}
				/>
			</div>
		</div>
	);
}

function NodeReferenceLine({ label, value }: { label: string; value: string }) {
	return (
		<div className="grid gap-2 sm:grid-cols-[120px_1fr]">
			<span className="font-mono text-xs font-bold tracking-[0.14em] text-baud-muted uppercase">{label}</span>
			<span>{value}</span>
		</div>
	);
}

function SimulationAndExportSection() {
	return (
		<section className="space-y-6">
			<div className="space-y-3">
				<SectionTitle icon={PackageCheck} title="Simulation And Export" />
				<DocTable
					columns={["Feature", "Description"]}
					rows={simulationRows.map((row) => [row.label, row.description])}
				/>
			</div>

			<div className="space-y-3">
				<SectionTitle icon={PackageCheck} title="Package Contents" />
				<InfoCard>
					Executable graph data is exported to <Code>{"program.json"}</Code>. Canvas positions are exported to{" "}
					<Code>{"editor.json"}</Code>. Permissions and capabilities are recalculated and written separately so the
					runner can compare declared values against its own calculation.
				</InfoCard>
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
				<SectionTitle icon={Variable} title="Derived Metadata" />
				<InfoCard>
					Metadata fields are calculated from the current value when the reference is read. They are not stored into the
					variable and cannot overwrite real object fields because they use the reserved <Code>{"$"}</Code> prefix. Use
					them on saved variables like <Code>{"{{foo.$length}}"}</Code> or on node outputs like{" "}
					<Code>{"{{n-mr3zyt6f-12.items.$count}}"}</Code>.
				</InfoCard>
				<DocTable columns={["Field", "Description"]} rows={derivedMetadataRows} />
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

function InfoCard({ children }: { children: ReactNode }) {
	return (
		<div className="rounded-lg border border-baud-border bg-baud-elevated p-4 text-sm leading-6 text-baud-muted">
			{children}
		</div>
	);
}

function DocTable({ columns, rows }: { columns: string[]; rows: string[][] }) {
	const columnClassName =
		columns.length === 2
			? "sm:grid-cols-[minmax(140px,220px)_minmax(0,1fr)]"
			: "sm:grid-cols-[minmax(140px,200px)_minmax(150px,240px)_minmax(0,1fr)]";

	return (
		<div className="rounded-lg border border-baud-border bg-baud-elevated">
			<div
				className={`grid gap-3 border-b border-baud-border px-4 py-2 text-xs font-bold tracking-[0.16em] text-baud-muted uppercase ${columnClassName}`}
			>
				{columns.map((column) => (
					<span key={column}>{column}</span>
				))}
			</div>
			<div className="divide-y divide-baud-border">
				{rows.map((row) => (
					<div key={row.join("|")} className={`grid gap-3 px-4 py-3 text-sm ${columnClassName}`}>
						{row.map((cell, index) => {
							const column = columns[index] ?? cell;
							const key = `${column}-${cell}`;
							return index === 0 || looksLikeCode(cell) ? (
								<Code key={key}>{cell}</Code>
							) : (
								<p key={key} className="text-baud-muted">
									{cell}
								</p>
							);
						})}
					</div>
				))}
			</div>
		</div>
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

function looksLikeCode(value: string) {
	return (
		value.includes("(") ||
		value.includes(")") ||
		value.includes("{") ||
		value.includes("[") ||
		value.includes("_") ||
		value === "+" ||
		value === "-" ||
		value === "*" ||
		value === "/" ||
		value === "%" ||
		value === "^" ||
		value === "==" ||
		value === "!=" ||
		value === ">" ||
		value === ">=" ||
		value === "<" ||
		value === "<="
	);
}

function getBuiltInGroupSectionId(label: string) {
	return `built-in-${label.toLowerCase()}`;
}
