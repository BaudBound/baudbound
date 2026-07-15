import type { Edge, Node } from "@xyflow/react";
import { GripVertical, Info, Plus, Trash2, X } from "lucide-react";
import { Fragment, type ReactNode, type PointerEvent as ReactPointerEvent, useEffect, useId, useState } from "react";
import { CopyTextButton } from "@/components/common/copy-text-button";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { OptionCombobox } from "@/components/ui/option-combobox";
import { Switch } from "@/components/ui/switch";
import { Textarea } from "@/components/ui/textarea";
import { inspectorTabs } from "@/data/editor/inspector-tabs";
import {
	combinatorOptions,
	comparisonOperatorOptions,
	playSoundSourceOptions,
	type SelectOption,
	textTransformOperationOptions,
	variableOperationOptions,
	variableScopeOptions,
	variableTypeOptions,
} from "@/data/nodes/definitions/options";
import {
	type ConditionRow,
	createConditionRow,
	createHeaderRow,
	createSwitchCaseRow,
	getSwitchCaseRowsFromValue,
	type HeaderRow,
	isConditionRow,
	isHeaderRow,
	type SwitchCaseRow,
} from "@/data/nodes/definitions/rows";
import type { NodeConfigField } from "@/data/nodes/node-definition";
import { getNodeConfigFields } from "@/data/nodes/registry";
import { builtInVariableNames } from "@/data/project/built-in-variables";
import { createSerialDeviceOptions, serialLineEndingOptions } from "@/data/project/serial";
import {
	getDefaultVariableOperationValue,
	getVariableOperationFixedType,
	normalizeVariableOperation,
	type VariableType,
	validateObjectFieldPath,
	validateVariableOperationType,
	validateVariableOperationValue,
	validateWritableVariableName,
	variableOperationDefinitions,
	variableScopeDefinitions,
	variableTypeDefinitions,
} from "@/data/project/variables";
import { type ActiveReorderDragState, useReorderController } from "@/hooks/use-reorder-controller";
import type {
	ActionType,
	EditorAsset,
	InspectorTab,
	JsonValue,
	ProjectSettings,
	ScriptNodeData,
	SimulationOverride,
	SimulationOverrideOutcome,
	SimulationRunStatus,
	SimulationSettings,
	SimulationTriggerPayload,
} from "@/lib/types";
import { createEditorVariableRegistry } from "@/utils/editor-variables";
import { RiskBadge } from "../shell/risk-badge";
import { SimulatorPanel } from "../simulation/simulator-panel";
import { EdgeOrderPanel } from "./edge-order-panel";
import { KeyCaptureInput } from "./key-capture-input";
import { KeyReferencePanel } from "./key-reference-panel";
import { RuntimeDataPanel } from "./runtime-data-panel";
import { VariableCodeInput, type VariableCompletion } from "./variable-code-input";

type InspectorProps = {
	activeTab: InspectorTab;
	assets: EditorAsset[];
	edges: Edge[];
	nodes: Node<ScriptNodeData>[];
	projectSettings: ProjectSettings;
	selectedEdge: Edge | null;
	selectedNode: Node<ScriptNodeData> | null;
	simulationOverrides: SimulationOverride[];
	simulationSettings: SimulationSettings;
	simulationStatus: SimulationRunStatus;
	width: number;
	onAddSimulationOverride: (nodeId: string) => void;
	onRemoveSimulationOverride: (nodeId: string) => void;
	onSimulationSettingsChange: (settings: SimulationSettings) => void;
	onStopSimulation: () => void;
	onTabChange: (tab: InspectorTab) => void;
	onTriggerSimulation: (triggerNodeId: string, payload: SimulationTriggerPayload) => void;
	onUpdateNodeConfig: (nodeId: string, key: string, value: JsonValue) => void;
	onUpdateSimulationOverride: (nodeId: string, outcome: SimulationOverrideOutcome) => void;
	onDeleteNode: (nodeId: string) => void;
	onDeleteEdge: (edgeId: string) => void;
	onReorderEdges: (edgeIds: string[]) => void;
	onSelectEdge: (edgeId: string) => void;
};

export function Inspector({
	activeTab,
	assets,
	edges,
	nodes,
	projectSettings,
	selectedEdge,
	selectedNode,
	simulationOverrides,
	simulationSettings,
	simulationStatus,
	width,
	onAddSimulationOverride,
	onRemoveSimulationOverride,
	onSimulationSettingsChange,
	onStopSimulation,
	onTabChange,
	onTriggerSimulation,
	onUpdateNodeConfig,
	onUpdateSimulationOverride,
	onDeleteEdge,
	onDeleteNode,
	onReorderEdges,
	onSelectEdge,
}: InspectorProps) {
	return (
		<aside className="flex shrink-0 flex-col border-l border-baud-border bg-baud-panel" style={{ width }}>
			<div className="grid h-10 grid-cols-2 border-b border-baud-border">
				{inspectorTabs.map((tab) => (
					<Button
						type="button"
						key={tab.id}
						onClick={() => onTabChange(tab.id)}
						aria-label={tab.label}
						className={`h-full min-w-0 truncate px-1 text-xs font-bold uppercase tracking-[0.04em] ${
							activeTab === tab.id ? "border-b-baud-red text-white" : ""
						}`}
						size="none"
						variant="tab"
					>
						{width < 340 ? tab.shortLabel : tab.label}
					</Button>
				))}
			</div>

			<div className="min-h-0 flex-1 overflow-y-auto">
				{activeTab === "properties" &&
					(selectedEdge ? (
						<EdgeOrderPanel
							edges={edges}
							nodes={nodes}
							selectedEdge={selectedEdge}
							onDeleteEdge={onDeleteEdge}
							onReorder={onReorderEdges}
							onSelectEdge={onSelectEdge}
						/>
					) : (
						<PropertiesPanel
							assets={assets}
							nodes={nodes}
							projectSettings={projectSettings}
							selectedNode={selectedNode}
							onUpdateNodeConfig={onUpdateNodeConfig}
							onDeleteNode={onDeleteNode}
						/>
					))}
				{activeTab === "simulator" && (
					<SimulatorPanel
						nodes={nodes}
						overrides={simulationOverrides}
						settings={simulationSettings}
						status={simulationStatus}
						onAddOverride={onAddSimulationOverride}
						onRemoveOverride={onRemoveSimulationOverride}
						onSettingsChange={onSimulationSettingsChange}
						onStopSimulation={onStopSimulation}
						onTriggerSimulation={onTriggerSimulation}
						onUpdateOverride={onUpdateSimulationOverride}
					/>
				)}
			</div>
		</aside>
	);
}

function PropertiesPanel({
	assets,
	nodes,
	projectSettings,
	selectedNode,
	onUpdateNodeConfig,
	onDeleteNode,
}: {
	assets: EditorAsset[];
	nodes: Node<ScriptNodeData>[];
	projectSettings: ProjectSettings;
	selectedNode: Node<ScriptNodeData> | null;
	onUpdateNodeConfig: (nodeId: string, key: string, value: JsonValue) => void;
	onDeleteNode: (nodeId: string) => void;
}) {
	if (!selectedNode) {
		return (
			<div className="p-4">
				<div className="flex items-start gap-3 rounded border border-baud-border bg-baud-soft p-3">
					<Info className="mt-0.5 text-baud-muted" size={16} />
					<div>
						<div className="text-sm font-semibold text-white">Select a node</div>
						<p className="mt-1 text-xs leading-5 text-baud-muted">Node configuration and ports appear here.</p>
					</div>
				</div>
			</div>
		);
	}

	const fields = getNodeConfigFields(selectedNode.data.actionType);
	const variableCompletions = createVariableCompletions(projectSettings, nodes);
	const visibleFields =
		selectedNode.data.actionType === "runtime.set_variable" ||
		selectedNode.data.actionType === "action.text.format" ||
		usesKeyReference(selectedNode.data.actionType)
			? []
			: fields;
	const showsKeyReference = usesKeyReference(selectedNode.data.actionType);

	return (
		<div className="space-y-5 p-4">
			<section>
				<div className="mb-3 flex items-center gap-2">
					<span className="size-2 rounded-sm bg-baud-purple" />
					<div className="min-w-0 flex-1">
						<h2 className="text-sm font-bold text-white">{selectedNode.data.label}</h2>
						<div className="mt-1 flex min-w-0 items-center gap-2 font-mono text-sm text-baud-muted">
							<span className="min-w-0 truncate">
								{selectedNode.data.kind} - id:{selectedNode.id}
							</span>
							<CopyTextButton text={selectedNode.id} label="Copy node id" />
						</div>
					</div>
					<Button
						type="button"
						onClick={() => onDeleteNode(selectedNode.id)}
						aria-label="Delete node"
						title="Delete node"
						size="icon"
						variant="destructive"
					>
						<Trash2 size={15} />
					</Button>
				</div>
				<RiskBadge risk={selectedNode.data.risk} />
			</section>

			<section>
				<div className="mb-3 rounded border border-baud-border bg-baud-soft/60 p-3">
					<TextInput
						label="Custom name"
						value={valueToInputString(selectedNode.data.config.customName)}
						onChange={(value) => onUpdateNodeConfig(selectedNode.id, "customName", value)}
					/>
				</div>
				<h3 className="mb-2 text-xs font-bold uppercase tracking-[0.18em] text-baud-muted">Configuration</h3>
				<NodeSpecificHelp actionType={selectedNode.data.actionType} />
				{visibleFields.length === 0 && !hasCustomConfigPanel(selectedNode.data.actionType) ? (
					<div className="rounded border border-baud-border bg-baud-soft px-3 py-2 font-mono text-sm text-baud-muted">
						No fields required
					</div>
				) : (
					<div className="space-y-3 rounded border border-baud-border bg-baud-soft/60 p-3">
						{selectedNode.data.actionType === "runtime.set_variable" && (
							<VariableOperationConfigPanel
								config={selectedNode.data.config}
								variableCompletions={variableCompletions}
								onChange={(key, value) => onUpdateNodeConfig(selectedNode.id, key, value)}
							/>
						)}
						{selectedNode.data.actionType === "action.text.format" && (
							<TextTransformConfigPanel
								config={selectedNode.data.config}
								variableCompletions={variableCompletions}
								onChange={(key, value) => onUpdateNodeConfig(selectedNode.id, key, value)}
							/>
						)}
						{usesKeyReference(selectedNode.data.actionType) && (
							<KeyCaptureConfigPanel
								config={selectedNode.data.config}
								onChange={(key, value) => onUpdateNodeConfig(selectedNode.id, key, value)}
							/>
						)}
						{usesConditionRows(selectedNode.data.actionType) && (
							<IfElseConfigPanel
								config={selectedNode.data.config}
								variableCompletions={variableCompletions}
								onChange={(key, value) => onUpdateNodeConfig(selectedNode.id, key, value)}
							/>
						)}
						{selectedNode.data.actionType === "control.switch" && (
							<SwitchConfigPanel
								config={selectedNode.data.config}
								variableCompletions={variableCompletions}
								onChange={(key, value) => onUpdateNodeConfig(selectedNode.id, key, value)}
							/>
						)}
						{(selectedNode.data.actionType === "action.http" ||
							selectedNode.data.actionType === "action.webhook_response") && (
							<HttpHeadersPanel
								config={selectedNode.data.config}
								variableCompletions={variableCompletions}
								onChange={(key, value) => onUpdateNodeConfig(selectedNode.id, key, value)}
							/>
						)}
						{selectedNode.data.actionType === "action.sound.play" && (
							<PlaySoundConfigPanel
								assets={assets}
								config={selectedNode.data.config}
								variableCompletions={variableCompletions}
								onChange={(key, value) => onUpdateNodeConfig(selectedNode.id, key, value)}
							/>
						)}
						{selectedNode.data.actionType === "action.serial.write" && (
							<SerialWriteConfigPanel
								config={selectedNode.data.config}
								deviceOptions={createSerialDeviceOptions(nodes)}
								variableCompletions={variableCompletions}
								onChange={(key, value) => onUpdateNodeConfig(selectedNode.id, key, value)}
							/>
						)}
						{visibleFields.map((field) => (
							<ConfigField
								key={field.key}
								field={field}
								value={selectedNode.data.config[field.key]}
								variableCompletions={variableCompletions}
								onChange={(value) => onUpdateNodeConfig(selectedNode.id, field.key, value)}
							/>
						))}
					</div>
				)}
			</section>

			<RuntimeDataPanel selectedNode={selectedNode} />

			{showsKeyReference && <KeyReferencePanel />}

			<div className="rounded border border-baud-border bg-baud-soft p-3 text-xs leading-5 text-baud-muted">
				{selectedNode.data.kind === "trigger" && "Entry point. Defines when the script starts."}
				{selectedNode.data.kind === "control" && "Branches or loops execution flow."}
				{selectedNode.data.kind === "action" && "Performs an operation with optional side effects."}
			</div>
		</div>
	);
}

function ConfigField({
	field,
	value,
	variableCompletions,
	onChange,
}: {
	field: NodeConfigField;
	value: JsonValue | undefined;
	variableCompletions: VariableCompletion[];
	onChange: (value: JsonValue) => void;
}) {
	const inputValue = valueToInputString(value);

	return (
		<div>
			<span className="mb-1 block font-mono text-sm text-baud-muted">{field.label}</span>
			{field.type === "select" ? (
				<ComboboxField value={inputValue} options={field.options ?? []} onChange={onChange} />
			) : field.type === "switch" ? (
				<div className="flex min-h-9 items-center justify-between gap-3 rounded-lg border border-baud-border bg-baud-panel/70 px-3 py-2 transition-colors hover:border-baud-line">
					<span className="text-sm text-baud-text">{value === true || value === "true" ? "Enabled" : "Disabled"}</span>
					<Switch checked={value === true || value === "true"} onCheckedChange={(checked) => onChange(checked)} />
				</div>
			) : field.type === "textarea" && field.usesVariables ? (
				<VariableCodeInput
					ariaLabel={field.label}
					value={inputValue}
					multiline
					variables={variableCompletions}
					onChange={onChange}
				/>
			) : field.type === "textarea" ? (
				<Textarea value={inputValue} onChange={(event) => onChange(event.target.value)} />
			) : field.usesVariables ? (
				<VariableCodeInput
					ariaLabel={field.label}
					value={inputValue}
					variables={variableCompletions}
					onChange={onChange}
				/>
			) : (
				<Input
					value={inputValue}
					type={field.type === "number" && !field.usesVariables ? "number" : "text"}
					onChange={(event) => onChange(event.target.value)}
				/>
			)}
			{field.help && <p className="mt-1 text-xs leading-4 text-baud-muted">{field.help}</p>}
		</div>
	);
}

function IfElseConfigPanel({
	config,
	variableCompletions,
	onChange,
}: {
	config: Record<string, JsonValue>;
	variableCompletions: VariableCompletion[];
	onChange: (key: string, value: JsonValue) => void;
}) {
	const conditions = getConditionRows(config.conditions, valueToInputString(config.combinator));
	const conditionReorder = useReorderController({
		rows: conditions,
		onCommit: (rows) => onChange("conditions", normalizeConditionRows(rows)),
	});
	const draggedCondition = conditionReorder.drag
		? conditions.find((condition) => condition.id === conditionReorder.drag?.draggedId)
		: null;
	let visibleConditionIndex = 0;

	return (
		<div className="space-y-3">
			<ul ref={conditionReorder.listRef} className="space-y-3" aria-label="Condition rows">
				{conditionReorder.entries.map((entry) => {
					if (entry.type === "drop-space") {
						return <ReorderDropSpace key={entry.id} height={entry.height} />;
					}

					const condition = entry.row;
					visibleConditionIndex += 1;
					const conditionIndex = visibleConditionIndex;

					return (
						<Fragment key={condition.id}>
							{conditionIndex > 1 && (
								<li>
									<ConditionCombinatorRow conditions={conditions} condition={condition} onChange={onChange} />
								</li>
							)}
							<li ref={conditionReorder.registerRow(condition.id)} data-reorder-card={condition.id}>
								<fieldset className="space-y-2 rounded border border-baud-border bg-baud-panel p-2 transition-[border-color,box-shadow] duration-150">
									<legend className="sr-only">Condition {conditionIndex}</legend>
									<div className="flex items-center justify-between">
										<div className="flex items-center gap-2">
											<DragHandle
												label={`Reorder condition ${conditionIndex}`}
												onPointerDown={(event) => conditionReorder.startDrag(condition.id, event)}
											/>
											<span className="font-mono text-sm text-baud-muted">Condition {conditionIndex}</span>
										</div>
										<RemoveRowButton
											label="Remove condition"
											onClick={() =>
												onChange(
													"conditions",
													normalizeConditionRows(conditions.filter((row) => row.id !== condition.id)),
												)
											}
										/>
									</div>
									<TextInput
										label="Value"
										value={condition.left}
										usesVariables
										variableCompletions={variableCompletions}
										onChange={(value) => updateCondition(conditions, condition.id, { left: value }, onChange)}
									/>
									<ComboboxField
										label="Expression"
										value={condition.operator}
										options={comparisonOperatorOptions}
										onChange={(value) => updateCondition(conditions, condition.id, { operator: value }, onChange)}
									/>
									<ConditionInvertCheckbox
										checked={condition.invert === true}
										onChange={(checked) => updateCondition(conditions, condition.id, { invert: checked }, onChange)}
									/>
									<TextInput
										label="Target"
										value={condition.right}
										usesVariables
										variableCompletions={variableCompletions}
										onChange={(value) => updateCondition(conditions, condition.id, { right: value }, onChange)}
									/>
								</fieldset>
							</li>
						</Fragment>
					);
				})}
			</ul>
			{draggedCondition && conditionReorder.drag && (
				<FloatingConditionCard condition={draggedCondition} drag={conditionReorder.drag} />
			)}

			<AddButton
				label="Add condition"
				onClick={() => onChange("conditions", normalizeConditionRows([...conditions, createConditionRow()]))}
			/>
		</div>
	);
}

function SwitchConfigPanel({
	config,
	variableCompletions,
	onChange,
}: {
	config: Record<string, JsonValue>;
	variableCompletions: VariableCompletion[];
	onChange: (key: string, value: JsonValue) => void;
}) {
	const cases = getSwitchCaseRowsFromValue(config.cases);
	const caseReorder = useReorderController({
		rows: cases,
		onCommit: (rows) => onChange("cases", rows),
	});
	const draggedCase = caseReorder.drag
		? cases.find((switchCase) => switchCase.id === caseReorder.drag?.draggedId)
		: null;
	let visibleCaseIndex = 0;

	return (
		<div className="space-y-3">
			<TextInput
				label="Switch value"
				value={valueToInputString(config.value)}
				usesVariables
				variableCompletions={variableCompletions}
				onChange={(value) => onChange("value", value)}
			/>
			<ul ref={caseReorder.listRef} className="space-y-3" aria-label="Switch cases">
				{caseReorder.entries.map((entry) => {
					if (entry.type === "drop-space") {
						return <ReorderDropSpace key={entry.id} height={entry.height} />;
					}

					const switchCase = entry.row;
					visibleCaseIndex += 1;
					const caseIndex = visibleCaseIndex;

					return (
						<li
							key={switchCase.id}
							ref={caseReorder.registerRow(switchCase.id)}
							data-reorder-card={switchCase.id}
							className="space-y-2 rounded border border-baud-border bg-baud-panel p-2 transition-[border-color,box-shadow] duration-150"
						>
							<div className="flex items-center justify-between">
								<div className="flex items-center gap-2">
									<DragHandle
										label={`Reorder switch case ${caseIndex}`}
										onPointerDown={(event) => caseReorder.startDrag(switchCase.id, event)}
									/>
									<span className="font-mono text-sm text-baud-muted">Case node {caseIndex}</span>
								</div>
								<RemoveRowButton
									label="Remove switch case"
									onClick={() =>
										onChange(
											"cases",
											cases.filter((row) => row.id !== switchCase.id),
										)
									}
								/>
							</div>
							<TextInput
								label="Name"
								value={switchCase.name}
								onChange={(value) => updateSwitchCase(cases, switchCase.id, { name: value }, onChange)}
							/>
							<TextInput
								label="Value"
								value={switchCase.value}
								usesVariables
								variableCompletions={variableCompletions}
								onChange={(value) => updateSwitchCase(cases, switchCase.id, { value }, onChange)}
							/>
						</li>
					);
				})}
			</ul>
			{draggedCase && caseReorder.drag && <FloatingSwitchCaseCard switchCase={draggedCase} drag={caseReorder.drag} />}
			<AddButton label="Add switch case" onClick={() => onChange("cases", [...cases, createSwitchCaseRow()])} />
		</div>
	);
}

function TextTransformConfigPanel({
	config,
	variableCompletions,
	onChange,
}: {
	config: Record<string, JsonValue>;
	variableCompletions: VariableCompletion[];
	onChange: (key: string, value: JsonValue) => void;
}) {
	const operation = normalizeTextTransformOperation(valueToInputString(config.operation));

	return (
		<div className="space-y-3">
			<ComboboxField
				label="Operation"
				value={operation}
				options={textTransformOperationOptions}
				onChange={(value) => onChange("operation", value)}
			/>
			<p className="text-xs leading-4 text-baud-muted">{getTextTransformHelp(operation)}</p>

			{operation === "template" && (
				<TextInput
					label="Template"
					value={valueToInputString(config.template)}
					usesVariables
					variableCompletions={variableCompletions}
					onChange={(value) => onChange("template", value)}
				/>
			)}

			{usesTextTransformInput(operation) && (
				<TextInput
					label="Input"
					value={valueToInputString(config.input)}
					usesVariables
					variableCompletions={variableCompletions}
					onChange={(value) => onChange("input", value)}
				/>
			)}

			{(operation === "replace" || operation === "regex_replace") && (
				<>
					<TextInput
						label={operation === "replace" ? "Search" : "Regex pattern"}
						value={valueToInputString(config.search)}
						usesVariables
						variableCompletions={variableCompletions}
						onChange={(value) => onChange("search", value)}
					/>
					<TextInput
						label="Replacement"
						value={valueToInputString(config.replacement)}
						usesVariables
						variableCompletions={variableCompletions}
						onChange={(value) => onChange("replacement", value)}
					/>
				</>
			)}

			{operation === "join" && (
				<TextInput
					label="Items"
					value={valueToInputString(config.items)}
					usesVariables
					variableCompletions={variableCompletions}
					onChange={(value) => onChange("items", value)}
				/>
			)}

			{(operation === "split" || operation === "join") && (
				<TextInput
					label="Delimiter"
					value={valueToInputString(config.delimiter)}
					usesVariables
					variableCompletions={variableCompletions}
					onChange={(value) => onChange("delimiter", value)}
				/>
			)}

			{operation === "substring" && (
				<div className="grid grid-cols-2 gap-2">
					<TextInput
						label="Start"
						value={valueToInputString(config.start)}
						usesVariables
						variableCompletions={variableCompletions}
						onChange={(value) => onChange("start", value)}
					/>
					<TextInput
						label="Length"
						value={valueToInputString(config.length)}
						usesVariables
						variableCompletions={variableCompletions}
						onChange={(value) => onChange("length", value)}
					/>
				</div>
			)}

			{(operation === "pad_start" || operation === "pad_end") && (
				<div className="grid grid-cols-2 gap-2">
					<TextInput
						label="Target length"
						value={valueToInputString(config.targetLength)}
						usesVariables
						variableCompletions={variableCompletions}
						onChange={(value) => onChange("targetLength", value)}
					/>
					<TextInput
						label="Pad text"
						value={valueToInputString(config.pad)}
						usesVariables
						variableCompletions={variableCompletions}
						onChange={(value) => onChange("pad", value)}
					/>
				</div>
			)}
		</div>
	);
}

function ConditionCombinatorRow({
	conditions,
	condition,
	onChange,
}: {
	conditions: ConditionRow[];
	condition: ConditionRow;
	onChange: (key: string, value: JsonValue) => void;
}) {
	return (
		<div className="flex items-center gap-2">
			<div className="h-px flex-1 bg-baud-border" />
			<ComboboxField
				value={normalizeCombinator(condition.combinator)}
				options={combinatorOptions}
				onChange={(value) => updateCondition(conditions, condition.id, { combinator: value }, onChange)}
				triggerClassName="w-auto font-mono"
				ariaLabel="Condition combinator"
			/>
			<div className="h-px flex-1 bg-baud-border" />
		</div>
	);
}

function ConditionInvertCheckbox({ checked, onChange }: { checked: boolean; onChange: (checked: boolean) => void }) {
	return (
		<div className="flex min-h-9 items-center justify-between gap-3 rounded-lg border border-baud-border bg-baud-panel/70 px-3 py-2 transition-colors hover:border-baud-line">
			<div>
				<span className="block text-sm text-baud-text">Invert condition</span>
				<span className="font-mono text-xs text-baud-muted">{checked ? "Enabled" : "Disabled"}</span>
			</div>
			<Switch checked={checked} onCheckedChange={onChange} />
		</div>
	);
}

function VariableOperationConfigPanel({
	config,
	variableCompletions,
	onChange,
}: {
	config: Record<string, JsonValue>;
	variableCompletions: VariableCompletion[];
	onChange: (key: string, value: JsonValue) => void;
}) {
	const operation = normalizeVariableOperation(valueToInputString(config.operation));
	const fixedType = getVariableOperationFixedType(operation);
	const selectedType = fixedType ?? normalizeVariableType(valueToInputString(config.valueType));
	const scope = normalizeScope(valueToInputString(config.scope));
	const savedName = valueToInputString(config.name);
	const savedValue = valueToInputString(config.value);
	const savedFieldPath = valueToInputString(config.fieldPath);
	const [draftName, setDraftName] = useState(savedName);
	const [draftValue, setDraftValue] = useState(savedValue);
	const [draftFieldPath, setDraftFieldPath] = useState(savedFieldPath);

	useEffect(() => {
		setDraftName(savedName);
		setDraftValue(savedValue);
		setDraftFieldPath(savedFieldPath);
	}, [savedName, savedValue, savedFieldPath]);

	const nameValidationMessage = validateWritableVariableName(draftName, builtInVariableNames);
	const typeCompatibilityMessage = validateVariableOperationType(operation, selectedType);
	const validationMessage = validateVariableOperationValue(operation, selectedType, draftValue, draftFieldPath);
	const definition = variableTypeDefinitions[selectedType];
	const operationDefinition = variableOperationDefinitions[operation];

	const handleTypeChange = (value: string) => {
		const nextType = normalizeVariableType(value);
		const nextValue = getDefaultVariableOperationValue(operation, nextType);
		onChange("valueType", nextType);
		onChange("value", nextValue);
		setDraftValue(nextValue);
	};

	const handleOperationChange = (value: string) => {
		const nextOperation = normalizeVariableOperation(value);
		const nextType = getVariableOperationFixedType(nextOperation) ?? selectedType;
		const nextValue = getDefaultVariableOperationValue(nextOperation, nextType);
		onChange("operation", nextOperation);
		onChange("valueType", nextType);
		onChange("value", nextValue);
		setDraftValue(nextValue);

		if (nextOperation === "set_object_field" && !draftFieldPath.trim()) {
			onChange("fieldPath", "field");
			setDraftFieldPath("field");
		}
	};

	const handleValueChange = (value: string) => {
		setDraftValue(value);
		if (!validateVariableOperationValue(operation, selectedType, value, draftFieldPath)) {
			onChange("value", value);
		}
	};

	const handleFieldPathChange = (value: string) => {
		setDraftFieldPath(value);
		if (!validateObjectFieldPath(value)) {
			onChange("fieldPath", value.trim());
		}
	};

	const handleNameChange = (value: string) => {
		setDraftName(value);
		if (!validateWritableVariableName(value, builtInVariableNames)) {
			onChange("name", value.trim());
		}
	};

	return (
		<div className="space-y-3">
			<ComboboxField
				label="Operation"
				value={operation}
				options={variableOperationOptions}
				onChange={handleOperationChange}
			/>
			<p className="text-xs leading-4 text-baud-muted">{operationDefinition.description}</p>
			<div>
				<TextInput
					label="Variable name"
					value={draftName}
					onChange={handleNameChange}
					hasError={!!nameValidationMessage}
				/>
				{nameValidationMessage && <p className="mt-1 text-xs leading-4 text-baud-danger">{nameValidationMessage}</p>}
			</div>
			<ComboboxField
				label="Scope"
				value={scope}
				options={variableScopeOptions}
				onChange={(value) => onChange("scope", value)}
			/>
			<p className="text-xs leading-4 text-baud-muted">{variableScopeDefinitions[normalizeScope(scope)]}</p>
			{fixedType ? (
				<div>
					<span className="mb-1 block font-mono text-sm text-baud-muted">Variable type</span>
					<div className="rounded border border-baud-border bg-baud-soft px-3 py-2 font-mono text-sm text-baud-text">
						{fixedType}
					</div>
					<p className="mt-1 text-xs leading-4 text-baud-muted">
						This operation only supports {fixedType} variables, so the type is locked.
					</p>
				</div>
			) : (
				<ComboboxField
					label="Variable type"
					value={selectedType}
					options={variableTypeOptions}
					onChange={handleTypeChange}
				/>
			)}
			{typeCompatibilityMessage && <p className="text-xs leading-4 text-baud-danger">{typeCompatibilityMessage}</p>}
			{operation === "set_object_field" && (
				<div>
					<TextInput
						label="Object field path"
						value={draftFieldPath}
						onChange={handleFieldPathChange}
						hasError={!!validateObjectFieldPath(draftFieldPath)}
					/>
					<p className="mt-1 text-xs leading-4 text-baud-muted">
						Use dot paths and indexes, for example profile.name or users[0].score.
					</p>
					{validateObjectFieldPath(draftFieldPath) && (
						<p className="mt-1 text-xs leading-4 text-baud-danger">{validateObjectFieldPath(draftFieldPath)}</p>
					)}
				</div>
			)}
			<div>
				<span className="mb-1 block font-mono text-sm text-baud-muted">{operationDefinition.valueLabel}</span>
				{operation === "clear" ? (
					<div className="rounded border border-baud-border bg-baud-soft p-3 text-sm leading-5 text-baud-muted">
						This operation clears the variable to the empty value for its type. No manual value is required.
					</div>
				) : (
					<VariableCodeInput
						ariaLabel={operationDefinition.valueLabel}
						value={draftValue}
						multiline
						hasError={!!validationMessage}
						variables={variableCompletions}
						onChange={handleValueChange}
					/>
				)}
				<p className="mt-1 text-xs leading-4 text-baud-muted">{definition.description}</p>
				<p className="mt-1 break-all font-mono text-xs leading-4 text-baud-muted">
					Example: {getDefaultVariableOperationValue(operation, selectedType) || definition.example}
				</p>
				{validationMessage && <p className="mt-1 text-xs leading-4 text-baud-danger">{validationMessage}</p>}
			</div>
		</div>
	);
}

function HttpHeadersPanel({
	config,
	variableCompletions,
	onChange,
}: {
	config: Record<string, JsonValue>;
	variableCompletions: VariableCompletion[];
	onChange: (key: string, value: JsonValue) => void;
}) {
	const headers = getHeaderRows(config.headers);

	return (
		<div className="space-y-2">
			<div className="flex items-center justify-between">
				<span className="font-mono text-xs uppercase tracking-[0.14em] text-baud-muted">Headers</span>
				<AddButton compact label="Add header" onClick={() => onChange("headers", [...headers, createHeaderRow()])} />
			</div>
			{headers.map((header) => (
				<div key={header.id} className="grid grid-cols-[1fr_1fr_24px] gap-2">
					<Input
						value={header.name}
						onChange={(event) => updateHeader(headers, header.id, { name: event.target.value }, onChange)}
						placeholder="Header"
						className="min-w-0 bg-baud-panel px-2"
					/>
					<VariableCodeInput
						ariaLabel="Header value"
						value={header.value}
						variables={variableCompletions}
						onChange={(value) => updateHeader(headers, header.id, { value }, onChange)}
						placeholder="Value"
					/>
					<Button
						type="button"
						onClick={() =>
							onChange(
								"headers",
								headers.filter((row) => row.id !== header.id),
							)
						}
						aria-label="Remove header"
						size="icon"
						variant="destructive"
					>
						<X size={13} />
					</Button>
				</div>
			))}
		</div>
	);
}

function PlaySoundConfigPanel({
	assets,
	config,
	variableCompletions,
	onChange,
}: {
	assets: EditorAsset[];
	config: Record<string, JsonValue>;
	variableCompletions: VariableCompletion[];
	onChange: (key: string, value: JsonValue) => void;
}) {
	const source = normalizePlaySoundSource(valueToInputString(config.source));
	const audioAssets = assets.filter((asset) => asset.kind === "audio");
	const assetOptions = audioAssets.map((asset) => ({
		label: `${asset.name} (${asset.packagePath})`,
		value: asset.packagePath,
	}));
	const assetPath = valueToInputString(config.assetPath);

	return (
		<div className="space-y-3">
			<ComboboxField
				label="Source"
				value={source}
				options={playSoundSourceOptions}
				onChange={(value) => onChange("source", value)}
			/>
			{source === "asset" ? (
				<div>
					<ComboboxField
						label="Audio asset"
						value={assetPath}
						options={assetOptions}
						onChange={(value) => onChange("assetPath", value)}
						ariaLabel="Audio asset"
					/>
					{audioAssets.length === 0 && (
						<p className="mt-1 text-xs leading-4 text-baud-danger">
							Add an audio file in the Asset Editor before using asset library playback.
						</p>
					)}
				</div>
			) : (
				<TextInput
					label="File path"
					value={valueToInputString(config.filePath)}
					usesVariables
					variableCompletions={variableCompletions}
					onChange={(value) => onChange("filePath", value)}
				/>
			)}
		</div>
	);
}

function SerialWriteConfigPanel({
	config,
	deviceOptions,
	variableCompletions,
	onChange,
}: {
	config: Record<string, JsonValue>;
	deviceOptions: SelectOption[];
	variableCompletions: VariableCompletion[];
	onChange: (key: string, value: JsonValue) => void;
}) {
	const selectedDeviceId = valueToInputString(config.deviceId);
	const options =
		selectedDeviceId && !deviceOptions.some((option) => option.value === selectedDeviceId)
			? [{ label: `${selectedDeviceId} (not configured)`, value: selectedDeviceId }, ...deviceOptions]
			: deviceOptions;

	return (
		<div className="space-y-3">
			<ComboboxField
				label="Device"
				value={selectedDeviceId}
				options={options}
				onChange={(value) => onChange("deviceId", value)}
				ariaLabel="Serial write device"
			/>
			{deviceOptions.length === 0 && (
				<p className="text-xs leading-4 text-baud-danger">
					Add a Serial Input Trigger first so the write action knows which logical serial device to target.
				</p>
			)}
			<ComboboxField
				label="Line ending"
				value={normalizeLineEnding(valueToInputString(config.lineEnding))}
				options={serialLineEndingOptions}
				onChange={(value) => onChange("lineEnding", value)}
			/>
			<TextInput
				label="Data"
				value={valueToInputString(config.data)}
				usesVariables
				variableCompletions={variableCompletions}
				onChange={(value) => onChange("data", value)}
			/>
		</div>
	);
}

function KeyCaptureConfigPanel({
	config,
	onChange,
}: {
	config: Record<string, JsonValue>;
	onChange: (key: string, value: JsonValue) => void;
}) {
	return <KeyCaptureInput label="Key" value={getConfiguredKey(config)} onChange={(value) => onChange("key", value)} />;
}

function NodeSpecificHelp({ actionType }: { actionType: ActionType }) {
	if (actionType === "trigger.webhook") {
		return (
			<p className="mb-3 rounded border border-baud-border bg-baud-soft px-3 py-2 text-xs leading-4 text-baud-muted">
				Call this trigger on the runner webhook server: http://&lt;runner-ip&gt;:&lt;runner-port&gt;/events/hookname.
				The runner decides the port.
			</p>
		);
	}

	if (actionType === "trigger.websocket") {
		return (
			<p className="mb-3 rounded border border-baud-border bg-baud-soft px-3 py-2 text-xs leading-4 text-baud-muted">
				Connect to this trigger through the runner WebSocket server. The runner decides host and port; this node defines
				the WebSocket path.
			</p>
		);
	}

	if (actionType === "trigger.startup") {
		return (
			<p className="mb-3 rounded border border-baud-border bg-baud-soft px-3 py-2 text-xs leading-4 text-baud-muted">
				Runs when the runner receives the operating system startup/session-start event.
			</p>
		);
	}

	if (actionType === "trigger.process_started") {
		return (
			<p className="mb-3 rounded border border-baud-border bg-baud-soft px-3 py-2 text-xs leading-4 text-baud-muted">
				The runner watches local process events and starts this branch when the configured process match is detected.
			</p>
		);
	}

	if (actionType === "trigger.hotkey" || actionType === "action.keyboard") {
		return null;
	}

	if (actionType === "control.loop") {
		return (
			<p className="mb-3 rounded border border-baud-border bg-baud-soft px-3 py-2 text-xs leading-4 text-baud-muted">
				The loop output runs the repeated body once per iteration. Let the body branch end naturally; do not connect it
				back to the loop input. The done output runs after all iterations complete.
			</p>
		);
	}

	if (actionType === "control.while") {
		return (
			<p className="mb-3 rounded border border-baud-border bg-baud-soft px-3 py-2 text-xs leading-4 text-baud-muted">
				The while node checks its conditions before every iteration. The loop output runs while the conditions pass. Let
				the body branch end naturally; do not connect it back to the while input. The done output runs when the
				conditions fail.
			</p>
		);
	}

	return null;
}

function TextInput({
	label,
	value,
	onChange,
	hasError,
	usesVariables,
	variableCompletions = [],
}: {
	label: string;
	value: string;
	onChange: (value: string) => void;
	hasError?: boolean;
	usesVariables?: boolean;
	variableCompletions?: VariableCompletion[];
}) {
	const inputId = useId();

	return (
		<div>
			<label htmlFor={inputId} className="mb-1 block font-mono text-sm text-baud-muted">
				{label}
			</label>
			{usesVariables ? (
				<VariableCodeInput
					id={inputId}
					ariaLabel={label}
					value={value}
					hasError={hasError}
					variables={variableCompletions}
					onChange={onChange}
				/>
			) : (
				<Input
					id={inputId}
					value={value}
					onChange={(event) => onChange(event.target.value)}
					className={
						hasError
							? "border-baud-danger focus-visible:border-baud-danger"
							: "border-baud-border focus-visible:border-baud-red/75"
					}
				/>
			)}
		</div>
	);
}

function ComboboxField({
	ariaLabel,
	label,
	value,
	options,
	onChange,
	triggerClassName,
}: {
	ariaLabel?: string;
	label?: string;
	value: string;
	options: SelectOption[];
	onChange: (value: string) => void;
	triggerClassName?: string;
}) {
	const combobox = (
		<OptionCombobox
			ariaLabel={ariaLabel ?? label}
			className={triggerClassName ?? "w-full"}
			options={options}
			value={value}
			onChange={onChange}
		/>
	);

	if (!label) {
		return combobox;
	}

	return (
		<div>
			<span className="mb-1 block font-mono text-sm text-baud-muted">{label}</span>
			{combobox}
		</div>
	);
}

function AddButton({ compact, label, onClick }: { compact?: boolean; label: string; onClick: () => void }) {
	return (
		<Button type="button" onClick={onClick} size={compact ? "sm" : "default"}>
			<Plus size={13} />
			{label}
		</Button>
	);
}

function DragHandle({
	label,
	onPointerDown,
}: {
	label: string;
	onPointerDown: (event: ReactPointerEvent<HTMLButtonElement>) => void;
}) {
	return (
		<Button
			type="button"
			onPointerDown={onPointerDown}
			className="cursor-grab active:cursor-grabbing"
			aria-label={label}
			title={label}
			size="xsIcon"
			variant="ghost"
			style={{ touchAction: "none" }}
		>
			<GripVertical size={15} />
		</Button>
	);
}

function ReorderDropSpace({ height }: { height: number }) {
	return <li aria-hidden="true" className="transition-[height] duration-150 ease-out" style={{ height }} />;
}

function FloatingConditionCard({ condition, drag }: { condition: ConditionRow; drag: ActiveReorderDragState }) {
	return (
		<FloatingReorderCard drag={drag}>
			<div className="flex items-center justify-between gap-3">
				<div className="flex items-center gap-2">
					<GripVertical size={15} className="text-baud-muted" />
					<span className="font-mono text-sm text-baud-muted">Condition</span>
				</div>
				{condition.invert === true && <span className="font-mono text-xs uppercase text-baud-red">Inverted</span>}
			</div>
			<GhostField label="Value" value={condition.left} />
			<GhostField label="Expression" value={condition.operator} />
			<GhostField label="Target" value={condition.right} />
		</FloatingReorderCard>
	);
}

function FloatingSwitchCaseCard({ switchCase, drag }: { switchCase: SwitchCaseRow; drag: ActiveReorderDragState }) {
	return (
		<FloatingReorderCard drag={drag}>
			<div className="flex items-center gap-2">
				<GripVertical size={15} className="text-baud-muted" />
				<span className="font-mono text-sm text-baud-muted">Case node</span>
			</div>
			<GhostField label="Name" value={switchCase.name} />
			<GhostField label="Value" value={switchCase.value} />
		</FloatingReorderCard>
	);
}

function FloatingReorderCard({ drag, children }: { drag: ActiveReorderDragState; children: ReactNode }) {
	return (
		<div
			className="pointer-events-none fixed z-9999 space-y-2 rounded border border-baud-purple bg-baud-panel p-2 opacity-95 shadow-[0_18px_42px_rgba(0,0,0,0.38)]"
			style={{
				left: drag.pointerX - drag.pointerOffsetX,
				minHeight: drag.cardHeight,
				top: drag.pointerY - drag.pointerOffsetY,
				transform: "rotate(0.7deg)",
				width: drag.cardWidth,
			}}
		>
			{children}
		</div>
	);
}

function GhostField({ label, value }: { label: string; value: string }) {
	return (
		<div>
			<div className="mb-1 font-mono text-sm text-baud-muted">{label}</div>
			<div className="min-h-8 truncate rounded border border-baud-border bg-baud-soft px-3 py-1.5 font-mono text-sm text-baud-text">
				{value || "-"}
			</div>
		</div>
	);
}

function RemoveRowButton({ label, onClick }: { label: string; onClick: () => void }) {
	return (
		<Button type="button" onClick={onClick} aria-label={label} title={label} size="xsIcon" variant="destructive">
			<X size={13} />
		</Button>
	);
}

function hasCustomConfigPanel(actionType: ActionType) {
	return (
		actionType === "runtime.set_variable" ||
		actionType === "action.text.format" ||
		usesKeyReference(actionType) ||
		usesConditionRows(actionType) ||
		actionType === "control.switch" ||
		actionType === "action.http" ||
		actionType === "action.webhook_response" ||
		actionType === "action.sound.play" ||
		actionType === "action.serial.write"
	);
}

function normalizePlaySoundSource(value: string) {
	return value === "file_path" ? "file_path" : "asset";
}

function normalizeTextTransformOperation(value: string) {
	return textTransformOperationOptions.some((option) => option.value === value) ? value : "template";
}

function usesTextTransformInput(operation: string) {
	return operation !== "template" && operation !== "join";
}

function getTextTransformHelp(operation: string) {
	if (operation === "template") {
		return "Build text from normal content and {{variables}}.";
	}

	if (operation === "replace") {
		return "Replace every exact text match in the input.";
	}

	if (operation === "regex_replace") {
		return "Replace matches using a JavaScript-style regular expression pattern.";
	}

	if (operation === "split") {
		return "Split input text into list output items.";
	}

	if (operation === "join") {
		return "Join a JSON array or list reference into one text value.";
	}

	if (operation === "substring") {
		return "Read part of the input text. Leave length empty to continue to the end.";
	}

	if (operation === "pad_start" || operation === "pad_end") {
		return "Pad the input text until it reaches the target length.";
	}

	if (operation === "json_escape" || operation === "json_unescape") {
		return "Convert text to or from a JSON-safe string literal.";
	}

	if (operation === "base64_encode" || operation === "base64_decode") {
		return "Encode or decode UTF-8 text with Base64.";
	}

	if (operation === "url_encode" || operation === "url_decode") {
		return "Encode or decode text for URL query/path usage.";
	}

	return "Transform the input text and expose the result as runtime data.";
}

function normalizeLineEnding(value: string) {
	return value === "lf" || value === "crlf" ? value : "none";
}

function usesKeyReference(actionType: ActionType) {
	return actionType === "trigger.hotkey" || actionType === "action.keyboard";
}

function usesConditionRows(actionType: ActionType) {
	return actionType === "control.if" || actionType === "control.while";
}

function getConfiguredKey(config: Record<string, JsonValue>) {
	const key = valueToInputString(config.key);
	if (key) {
		return formatKeyWithModifiers(key, valueToInputString(config.modifiers));
	}

	return "";
}

function formatKeyWithModifiers(key: string, modifiers: string) {
	if (!modifiers || modifiers === "none") {
		return key;
	}
	return `${modifiers
		.split("+")
		.filter(Boolean)
		.map((part) => `${part.charAt(0).toUpperCase()}${part.slice(1)}`)
		.join("+")}+${key}`;
}

function valueToInputString(value: JsonValue | undefined) {
	if (typeof value === "string") {
		return value;
	}

	if (typeof value === "number" || typeof value === "boolean") {
		return String(value);
	}

	return "";
}

function createVariableCompletions(
	projectSettings: ProjectSettings,
	nodes: Node<ScriptNodeData>[],
): VariableCompletion[] {
	const completionsByName = new Map<string, VariableCompletion>();

	for (const variable of createEditorVariableRegistry(projectSettings, nodes)) {
		completionsByName.set(variable.name, {
			description: variable.description,
			name: variable.name,
			readOnly: variable.read_only,
			token: variable.token,
			type: variable.type,
		});
	}

	return [...completionsByName.values()].sort((a, b) => a.name.localeCompare(b.name));
}

function getConditionRows(value: JsonValue | undefined, legacyCombinator = "and"): ConditionRow[] {
	if (!Array.isArray(value)) {
		return [createConditionRow()];
	}

	const rows = value
		.filter(isConditionRow)
		.map((condition, index) =>
			createSerializableConditionRow(condition, index === 0 ? undefined : (condition.combinator ?? legacyCombinator)),
		);

	return rows.length > 0 ? rows : [createConditionRow()];
}

function getHeaderRows(value: JsonValue | undefined): HeaderRow[] {
	return Array.isArray(value) ? value.filter(isHeaderRow) : [];
}

function updateCondition(
	conditions: ConditionRow[],
	id: string,
	patch: Partial<ConditionRow>,
	onChange: (key: string, value: JsonValue) => void,
) {
	onChange(
		"conditions",
		normalizeConditionRows(
			conditions.map((condition) => (condition.id === id ? { ...condition, ...patch } : condition)),
		),
	);
}

function normalizeConditionRows(conditions: ConditionRow[]) {
	return conditions.map((condition, index) =>
		createSerializableConditionRow(condition, index === 0 ? undefined : condition.combinator),
	);
}

function createSerializableConditionRow(condition: ConditionRow, combinator: string | undefined): ConditionRow {
	const row: ConditionRow = {
		id: condition.id,
		left: condition.left,
		operator: condition.operator,
		right: condition.right,
	};

	if (condition.invert === true) {
		row.invert = true;
	}

	return combinator === undefined ? row : { ...row, combinator: normalizeCombinator(combinator) };
}

function normalizeCombinator(value: string | undefined) {
	return value === "or" ? "or" : "and";
}

function normalizeVariableType(value: string): VariableType {
	return value in variableTypeDefinitions ? (value as VariableType) : "string";
}

function normalizeScope(value: string) {
	if (value === "local") {
		return "runtime";
	}

	if (value === "runner") {
		return "global";
	}

	return value === "persistent" || value === "global" ? value : "runtime";
}

function updateSwitchCase(
	cases: SwitchCaseRow[],
	id: string,
	patch: Partial<SwitchCaseRow>,
	onChange: (key: string, value: JsonValue) => void,
) {
	onChange(
		"cases",
		cases.map((switchCase) => (switchCase.id === id ? { ...switchCase, ...patch } : switchCase)),
	);
}

function updateHeader(
	headers: HeaderRow[],
	id: string,
	patch: Partial<HeaderRow>,
	onChange: (key: string, value: JsonValue) => void,
) {
	onChange(
		"headers",
		headers.map((header) => (header.id === id ? { ...header, ...patch } : header)),
	);
}
