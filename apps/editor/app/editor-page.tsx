"use client";

import {
	applyEdgeChanges,
	type Edge,
	type EdgeChange,
	useEdgesState,
	useNodesState,
	type XYPosition,
} from "@xyflow/react";
import type { ChangeEvent } from "react";
import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import {
	areCommentNodeDataEqual,
	createCommentFlowNode,
	DEFAULT_COMMENT_SIZE,
	isCommentFlowNode,
	toCommentFlowNode,
	toEditorComment,
} from "@/components/canvas/comment-card";
import { type EditorFlowNode, FlowCanvas, type ScriptFlowNode } from "@/components/canvas/flow-canvas";
import { Inspector } from "@/components/inspector/inspector";
import { AssetEditorModal } from "@/components/modals/asset-editor-modal";
import { ExportWizardModal } from "@/components/modals/export-wizard-modal";
import { HelpModal } from "@/components/modals/help-modal";
import { ProjectSettingsModal } from "@/components/modals/project-settings-modal";
import { SimulationMessageBoxDialog } from "@/components/modals/simulation-message-box-dialog";
import { VerificationErrorModal } from "@/components/modals/verification-error-modal";
import { VerificationModal } from "@/components/modals/verification-modal";
import { BlockLibrary } from "@/components/shell/block-library";
import { type BottomPanelTab, OutputConsole } from "@/components/shell/output-console";
import { ResizeHandle } from "@/components/shell/resize-handle";
import { StatusBar } from "@/components/shell/status-bar";
import { TopBar } from "@/components/shell/top-bar";
import { Toaster } from "@/components/ui/sonner";
import { defaultEditorEdgeStyle, type EditorEdgeStyle, toReactFlowEdgeType } from "@/data/editor/flow-canvas";
import { createSwitchOutputPorts, getSwitchCaseRowsFromValue } from "@/data/nodes/definitions/rows";
import {
	createDevelopmentEditorNodes,
	createInitialEditorEdges,
	createInitialEditorNodes,
	isDevelopmentGraphEnabled,
} from "@/data/nodes/development-graph";
import { createNodeFromPaletteItem, getFlatPaletteItems } from "@/data/nodes/registry";
import { createSimulationSecretValues, getSecretSimulationProblems } from "@/data/project/secrets";
import { useEditorPanelSizes } from "@/hooks/use-editor-panel-sizes";
import type {
	CommentNodeData,
	DefaultVariable,
	EditorAsset,
	InspectorTab,
	JsonValue,
	LogEntry,
	PaletteItem,
	ProjectSettings,
	SecretDeclaration,
	SimulationOverride,
	SimulationOverrideOutcome,
	SimulationRunStatus,
	SimulationSettings,
	SimulationTraceEntry,
	SimulationTriggerPayload,
	SimulationVariableSnapshot,
} from "@/lib/types";
import { DEFAULT_MINIMUM_RUNNER_VERSION } from "@/lib/version";
import {
	calculateCapabilities,
	calculatePermissions,
	calculateRiskLevel,
	createConsoleLogs,
	createExportSummary,
} from "@/utils/analysis";
import { exportBbsPackage, importBbsPackage, verifyBbsPackage } from "@/utils/bbs-package";
import {
	createEditorVerificationSignature,
	createGraphFragment,
	createGraphFragmentCopy,
	createGraphNodeCopy,
	DUPLICATE_OFFSET,
	type GraphFragment,
	getCenteredScriptNodePosition,
	hasManualTrigger,
	normalizeEdgeExecutionOrders,
	reorderEdgeExecutionGroup,
} from "@/utils/editor-graph";
import { truncateLogEntry, truncateSimulationTrace } from "@/utils/editor-log";
import { hasBrowserTextSelection, isEditableShortcutTarget } from "@/utils/editor-shortcuts";
import { createVariablePanelEntries } from "@/utils/editor-variables";
import {
	createSimulationRun,
	type SimulationSideEffect,
	type SimulationSideEffectResult,
	type SimulationStep,
} from "@/utils/simulation";
import { getSimulationStepDelay, getSimulationTriggers } from "@/utils/simulation-settings";
import { executeSimulationSideEffects } from "@/utils/simulation-side-effects";
import {
	createVerificationChecks,
	summarizeVerification,
	type VerificationCheck,
	type VerificationStatus,
} from "@/utils/verification";

type EditorClipboard = {
	fragment: GraphFragment<EditorFlowNode, Edge>;
	type: "graph";
};

type VerificationRecord = {
	signature: string | null;
	status: VerificationStatus;
};

type VerificationErrorDialog = {
	checks: VerificationCheck[];
	description: string;
	open: boolean;
	title: string;
};

type SimulationMessageBoxState = Extract<SimulationSideEffect, { type: "message_box" }> | null;

type SimulationLifecycle = {
	abortController: AbortController | null;
	active: boolean;
	runId: number;
};

const MAX_OUTPUT_LOG_ENTRIES = 800;
const MAX_SIMULATION_LOG_ENTRIES = 800;
const paletteItemByActionType: ReadonlyMap<string, PaletteItem> = new Map(
	getFlatPaletteItems().map((item) => [item.actionType, item]),
);
const DEFAULT_PROJECT_SETTINGS: ProjectSettings = {
	name: "untitled-script",
	description: "",
	author: "",
	website: "",
	repository: "",
	tags: [],
	targetRuntime: "Generic Desktop",
	minimumRunnerVersion: DEFAULT_MINIMUM_RUNNER_VERSION,
};

export function EditorPage() {
	const handleCopyGraphRef = useRef<(nodeId?: string) => boolean>(() => false);
	const importInputRef = useRef<HTMLInputElement>(null);
	const simulationLifecycleRef = useRef<SimulationLifecycle>({ abortController: null, active: false, runId: 0 });
	const simulationMessageBoxResolveRef = useRef<((button: string) => void) | null>(null);
	const initialNodes = useMemo<EditorFlowNode[]>(() => createInitialEditorNodes() as ScriptFlowNode[], []);
	const initialEdges = useMemo(() => createInitialEditorEdges(), []);
	const [projectSettings, setProjectSettings] = useState<ProjectSettings>(DEFAULT_PROJECT_SETTINGS);
	const [activeTab, setActiveTab] = useState<InspectorTab>("properties");
	const [selectedNodeId, setSelectedNodeId] = useState<string | null>(null);
	const [selectedEdgeId, setSelectedEdgeId] = useState<string | null>(null);
	const [bottomPanelTab, setBottomPanelTab] = useState<BottomPanelTab>("system");
	const [verificationOpen, setVerificationOpen] = useState(false);
	const [verificationRecord, setVerificationRecord] = useState<VerificationRecord>({
		signature: null,
		status: "unverified",
	});
	const [verificationErrorDialog, setVerificationErrorDialog] = useState<VerificationErrorDialog>({
		checks: [],
		description: "",
		open: false,
		title: "",
	});
	const [projectSettingsOpen, setProjectSettingsOpen] = useState(false);
	const [assetEditorOpen, setAssetEditorOpen] = useState(false);
	const [helpOpen, setHelpOpen] = useState(false);
	const [exportOpen, setExportOpen] = useState(false);
	const [clipboard, setClipboard] = useState<EditorClipboard | null>(null);
	const [assets, setAssets] = useState<EditorAsset[]>([]);
	const [defaultVariables, setDefaultVariables] = useState<DefaultVariable[]>([]);
	const [secretDeclarations, setSecretDeclarations] = useState<SecretDeclaration[]>([]);
	const [simulationSecretValues, setSimulationSecretValues] = useState<Record<string, string>>({});
	const [edgeStyle, setEdgeStyle] = useState<EditorEdgeStyle>(defaultEditorEdgeStyle);
	const [viewportCenter, setViewportCenter] = useState<XYPosition | null>(null);
	const [bottomPanelFollow, setBottomPanelFollow] = useState({
		system: true,
		output: true,
		simulation: true,
	});
	const [simulationSettings, setSimulationSettings] = useState<SimulationSettings>({
		speed: "instant",
	});
	const [simulationOverrides, setSimulationOverrides] = useState<SimulationOverride[]>([]);
	const [simulationStatus, setSimulationStatus] = useState<SimulationRunStatus>("idle");
	const [simulationLogs, setSimulationLogs] = useState<SimulationTraceEntry[]>([]);
	const [simulationVariables, setSimulationVariables] = useState<SimulationVariableSnapshot[]>([]);
	const [simulationMessageBox, setSimulationMessageBox] = useState<SimulationMessageBoxState>(null);
	const [systemLogs, setSystemLogs] = useState<LogEntry[]>(() =>
		createConsoleLogs(
			DEFAULT_PROJECT_SETTINGS.name,
			DEFAULT_PROJECT_SETTINGS.targetRuntime,
			calculatePermissions(initialNodes.filter(isScriptFlowNode)),
		),
	);
	const [logs, setLogs] = useState<LogEntry[]>([]);
	const [nodes, setNodes, onNodesChange] = useNodesState<EditorFlowNode>(initialNodes);
	const [edges, setEdges] = useEdgesState<Edge>(initialEdges);
	const { collapsed, expandPanel, sizes, startResize, togglePanel } = useEditorPanelSizes();

	const scriptNodes = useMemo(() => nodes.filter(isScriptFlowNode), [nodes]);
	const comments = useMemo(() => nodes.filter(isCommentFlowNode).map(toEditorComment), [nodes]);

	const selectedNode = useMemo(
		() => scriptNodes.find((node) => node.id === selectedNodeId) ?? null,
		[scriptNodes, selectedNodeId],
	);
	const selectedEdge = useMemo(() => edges.find((edge) => edge.id === selectedEdgeId) ?? null, [edges, selectedEdgeId]);
	const handleEdgesChange = useCallback(
		(changes: EdgeChange<Edge>[]) => {
			setEdges((currentEdges) => normalizeEdgeExecutionOrders(applyEdgeChanges(changes, currentEdges)));
		},
		[setEdges],
	);
	const permissions = useMemo(
		() => calculatePermissions(scriptNodes, secretDeclarations, defaultVariables),
		[scriptNodes, secretDeclarations, defaultVariables],
	);
	const capabilities = useMemo(
		() => calculateCapabilities(scriptNodes, secretDeclarations, defaultVariables),
		[scriptNodes, secretDeclarations, defaultVariables],
	);
	const riskLevel = useMemo(() => calculateRiskLevel(permissions), [permissions]);
	const exportSummary = useMemo(
		() =>
			createExportSummary(
				projectSettings.name,
				projectSettings.targetRuntime,
				projectSettings.minimumRunnerVersion,
				assets,
			),
		[assets, projectSettings.minimumRunnerVersion, projectSettings.name, projectSettings.targetRuntime],
	);
	const verificationChecks = useMemo(
		() =>
			createVerificationChecks({
				assets,
				edges,
				nodes: scriptNodes,
				permissions,
				defaultVariables,
				secretDeclarations,
				scriptName: projectSettings.name,
				targetRuntime: projectSettings.targetRuntime,
			}),
		[
			assets,
			defaultVariables,
			edges,
			scriptNodes,
			permissions,
			projectSettings.name,
			projectSettings.targetRuntime,
			secretDeclarations,
		],
	);
	const verificationSignature = useMemo(
		() =>
			createEditorVerificationSignature(
				projectSettings,
				scriptNodes,
				edges,
				assets,
				secretDeclarations,
				defaultVariables,
			),
		[projectSettings, scriptNodes, edges, assets, secretDeclarations, defaultVariables],
	);
	const variableEntries = useMemo(
		() =>
			createVariablePanelEntries(
				projectSettings,
				scriptNodes,
				simulationVariables,
				secretDeclarations,
				defaultVariables,
			),
		[projectSettings, scriptNodes, simulationVariables, secretDeclarations, defaultVariables],
	);
	const normalizedProjectSettings = {
		...projectSettings,
		name: projectSettings.name.trim() || "untitled-script",
		minimumRunnerVersion: projectSettings.minimumRunnerVersion.trim() || DEFAULT_MINIMUM_RUNNER_VERSION,
	};

	const appendOutputLogs = useCallback((entries: LogEntry[]) => {
		setLogs((currentLogs) => [...currentLogs, ...entries.map(truncateLogEntry)].slice(-MAX_OUTPUT_LOG_ENTRIES));
	}, []);

	const appendSystemLogs = useCallback((entries: LogEntry[]) => {
		setSystemLogs((currentLogs) => [...currentLogs, ...entries.map(truncateLogEntry)].slice(-MAX_OUTPUT_LOG_ENTRIES));
	}, []);

	const appendSimulationLogs = useCallback((entries: SimulationTraceEntry[]) => {
		setSimulationLogs((currentLogs) =>
			[...currentLogs, ...entries.map(truncateSimulationTrace)].slice(-MAX_SIMULATION_LOG_ENTRIES),
		);
	}, []);

	const abortSimulationLifecycle = useCallback((reason: string) => {
		const lifecycle = simulationLifecycleRef.current;
		lifecycle.abortController?.abort(reason);
		lifecycle.abortController = null;
		lifecycle.active = false;
		lifecycle.runId += 1;
	}, []);

	const startSimulationLifecycle = useCallback((abortController: AbortController) => {
		const lifecycle = simulationLifecycleRef.current;
		lifecycle.abortController = abortController;
		lifecycle.active = true;
		lifecycle.runId += 1;
		return lifecycle.runId;
	}, []);

	const completeSimulationLifecycle = useCallback((runId: number) => {
		const lifecycle = simulationLifecycleRef.current;
		if (lifecycle.runId !== runId) {
			return;
		}

		lifecycle.abortController = null;
		lifecycle.active = false;
	}, []);

	useEffect(() => {
		const disableNativeContextMenu = (event: MouseEvent) => event.preventDefault();

		document.addEventListener("contextmenu", disableNativeContextMenu);

		return () => document.removeEventListener("contextmenu", disableNativeContextMenu);
	}, []);

	useEffect(() => {
		return () => {
			abortSimulationLifecycle("editor unmounted");
		};
	}, [abortSimulationLifecycle]);

	useEffect(() => {
		setVerificationRecord((currentRecord) => {
			if (!currentRecord.signature || currentRecord.signature === verificationSignature) {
				return currentRecord;
			}

			return { signature: null, status: "unverified" };
		});
		setSimulationStatus("idle");
		setSimulationVariables([]);
		abortSimulationLifecycle("graph changed");
	}, [abortSimulationLifecycle, verificationSignature]);

	useEffect(() => {
		const nodeIds = new Set(scriptNodes.map((node) => node.id));
		setSimulationOverrides((currentOverrides) => currentOverrides.filter((override) => nodeIds.has(override.nodeId)));
	}, [scriptNodes]);

	const handleExport = () => {
		setExportOpen(true);
	};

	const handleDownloadExport = async () => {
		await exportBbsPackage({
			projectSettings: normalizedProjectSettings,
			nodes: scriptNodes,
			edges,
			assets,
			comments,
			edgeStyle,
			secretDeclarations,
			defaultVariables,
		});
	};

	const handleExportVerificationComplete = useCallback(
		(summary: ReturnType<typeof summarizeVerification>) => {
			setVerificationRecord({ signature: verificationSignature, status: summary.status });
			appendSystemLogs([
				{
					level: summary.status === "failed" ? "error" : summary.status === "warning" ? "warn" : "info",
					message: `Export verification ${summary.status}: ${summary.passed} passed, ${summary.warnings} warning${summary.warnings === 1 ? "" : "s"}, ${summary.failed} failed.`,
				},
			]);
			expandPanel("bottom");
		},
		[appendSystemLogs, expandPanel, verificationSignature],
	);

	const handleVerify = () => {
		const summary = summarizeVerification(verificationChecks);
		setVerificationRecord({ signature: verificationSignature, status: summary.status });
		setVerificationOpen(true);
		appendSystemLogs([
			{
				level: summary.status === "failed" ? "error" : summary.status === "warning" ? "warn" : "info",
				message: `Verification ${summary.status}: ${summary.passed} passed, ${summary.warnings} warning${summary.warnings === 1 ? "" : "s"}, ${summary.failed} failed.`,
			},
		]);
		expandPanel("bottom");
	};

	const showSimulationMessageBox = useCallback(
		(sideEffect: Extract<SimulationSideEffect, { type: "message_box" }>, signal: AbortSignal) =>
			new Promise<string>((resolve) => {
				if (signal.aborted) {
					resolve("aborted");
					return;
				}

				simulationMessageBoxResolveRef.current?.("replaced");
				const finish = (button: string) => {
					signal.removeEventListener("abort", handleAbort);
					if (simulationMessageBoxResolveRef.current === finish) {
						simulationMessageBoxResolveRef.current = null;
					}
					setSimulationMessageBox(null);
					resolve(button);
				};
				const handleAbort = () => finish("aborted");

				simulationMessageBoxResolveRef.current = finish;
				setSimulationMessageBox(sideEffect);
				signal.addEventListener("abort", handleAbort, { once: true });
			}),
		[],
	);

	const handleSimulationMessageBoxSelect = useCallback((button: string) => {
		const resolve = simulationMessageBoxResolveRef.current;
		simulationMessageBoxResolveRef.current = null;
		setSimulationMessageBox(null);
		resolve?.(button);
	}, []);

	const handleSimulationStep = useCallback(
		async (step: SimulationStep, runId: number, signal: AbortSignal): Promise<SimulationSideEffectResult[]> => {
			if (simulationLifecycleRef.current.runId !== runId || signal.aborted) {
				return [];
			}

			let sideEffectResults: SimulationSideEffectResult[] = [];
			if (step.sideEffects.length > 0) {
				const sideEffectResult = await executeSimulationSideEffects(step.sideEffects, assets, signal, {
					showMessageBox: showSimulationMessageBox,
				});
				sideEffectResults = sideEffectResult.results;
				const sideEffectErrors = sideEffectResult.traces;
				if (sideEffectErrors.length > 0) {
					appendSimulationLogs(sideEffectErrors);
				}
			}
			if (step.outputLogs.length > 0) {
				appendOutputLogs(step.outputLogs);
			}
			if (step.traces.length > 0) {
				appendSimulationLogs(step.traces);
			}
			setSimulationVariables(step.variables);
			return sideEffectResults;
		},
		[appendOutputLogs, appendSimulationLogs, assets, showSimulationMessageBox],
	);

	const runSimulationTrigger = useCallback(
		async ({
			abortController,
			keepWaiting,
			payload,
			runId,
			triggerNodeId,
		}: {
			abortController: AbortController;
			keepWaiting: boolean;
			payload: SimulationTriggerPayload;
			runId: number;
			triggerNodeId: string;
		}) => {
			const secretProblems = getSecretSimulationProblems(secretDeclarations, simulationSecretValues);
			if (secretProblems.length > 0) {
				setSimulationStatus("failed");
				appendSimulationLogs(secretProblems.map((message) => ({ level: "error", message: `[Simulation] ${message}` })));
				completeSimulationLifecycle(runId);
				return;
			}
			setSimulationStatus("running");

			try {
				const run = await createSimulationRun({
					assets,
					nodes: scriptNodes,
					edges,
					overrides: simulationOverrides,
					projectSettings,
					defaultVariables,
					secretValues: createSimulationSecretValues(secretDeclarations, simulationSecretValues),
					signal: abortController.signal,
					stepDelayMs: getSimulationStepDelay(simulationSettings.speed),
					triggerNodeId,
					triggerPayload: payload,
					onStep: (step) => handleSimulationStep(step, runId, abortController.signal),
				});
				if (simulationLifecycleRef.current.runId !== runId) {
					return;
				}

				setSimulationStatus(abortController.signal.aborted ? "stopped" : keepWaiting ? "waiting" : run.status);
				if (keepWaiting && !abortController.signal.aborted) {
					appendSimulationLogs([{ level: "info", message: "[Simulation] Waiting for the next trigger input." }]);
				}
			} finally {
				if (simulationLifecycleRef.current.runId === runId && !keepWaiting) {
					completeSimulationLifecycle(runId);
				}
			}
		},
		[
			appendSimulationLogs,
			assets,
			completeSimulationLifecycle,
			edges,
			handleSimulationStep,
			projectSettings,
			defaultVariables,
			secretDeclarations,
			scriptNodes,
			simulationSecretValues,
			simulationOverrides,
			simulationSettings.speed,
		],
	);

	const startSimulationSession = useCallback(
		(initialLogs: SimulationTraceEntry[] = []) => {
			const currentLifecycle = simulationLifecycleRef.current;
			if (currentLifecycle.active && currentLifecycle.abortController) {
				return currentLifecycle;
			}

			const triggerNodes = getSimulationTriggers(scriptNodes);
			if (triggerNodes.length === 0) {
				setVerificationErrorDialog({
					open: true,
					title: "Simulation Blocked",
					description: "Add at least one trigger node before starting simulation.",
					checks: [
						{
							id: "simulation-trigger",
							title: "Simulation Trigger",
							description: "Checking selected simulation triggers.",
							outcome: "failed",
							message: "No trigger nodes are available.",
						},
					],
				});
				appendSystemLogs([{ level: "error", message: "Simulation blocked: no trigger nodes are available." }]);
				appendSimulationLogs([
					{ level: "error", message: "[Simulation] Simulation blocked: no trigger nodes are available." },
				]);
				expandPanel("bottom");
				return null;
			}

			const abortController = new AbortController();
			const runId = startSimulationLifecycle(abortController);
			expandPanel("bottom");
			setSimulationStatus("waiting");
			setSimulationLogs([
				...initialLogs,
				{
					level: "info",
					message: `[Simulation] Waiting for input from ${triggerNodes.length} trigger${triggerNodes.length === 1 ? "" : "s"}. Schedule triggers run automatically while the simulation is active.`,
				},
			]);
			setSimulationVariables([]);
			return { abortController, active: true, runId };
		},
		[appendSimulationLogs, appendSystemLogs, expandPanel, scriptNodes, startSimulationLifecycle],
	);

	const handleTriggerSimulation = useCallback(
		(triggerNodeId: string, payload: SimulationTriggerPayload) => {
			if (simulationStatus === "running") {
				appendSimulationLogs([
					{
						level: "warn",
						message: "[Simulation] A trigger is already running. Stop it before firing another trigger.",
					},
				]);
				expandPanel("bottom");
				return;
			}

			const summary = summarizeVerification(verificationChecks);
			const verificationLog: SimulationTraceEntry = {
				level: summary.status === "failed" ? "error" : summary.status === "warning" ? "warn" : "info",
				message: `[Simulation] Verification ${summary.status}: ${summary.passed} passed, ${summary.warnings} warning${summary.warnings === 1 ? "" : "s"}, ${summary.failed} failed.`,
			};

			setVerificationRecord({ signature: verificationSignature, status: summary.status });
			appendSystemLogs([
				{
					level: verificationLog.level,
					message: `Simulation verification ${summary.status}: ${summary.passed} passed, ${summary.warnings} warning${summary.warnings === 1 ? "" : "s"}, ${summary.failed} failed.`,
				},
			]);

			if (summary.status === "failed") {
				setVerificationErrorDialog({
					open: true,
					title: "Simulation Blocked",
					description: "The current script failed verification and cannot be simulated.",
					checks: verificationChecks,
				});
				appendSimulationLogs([
					verificationLog,
					{ level: "error", message: "[Simulation] Simulation blocked: verification failed." },
				]);
				expandPanel("bottom");
				return;
			}

			appendSimulationLogs([verificationLog]);
			const lifecycle = startSimulationSession([verificationLog]);
			if (!lifecycle?.abortController) {
				return;
			}

			void runSimulationTrigger({
				abortController: lifecycle.abortController,
				keepWaiting: true,
				payload,
				runId: lifecycle.runId,
				triggerNodeId,
			});
		},
		[
			appendSimulationLogs,
			appendSystemLogs,
			expandPanel,
			runSimulationTrigger,
			simulationStatus,
			startSimulationSession,
			verificationChecks,
			verificationSignature,
		],
	);

	const handleStopSimulation = () => {
		if (!simulationLifecycleRef.current.active) {
			return;
		}

		abortSimulationLifecycle("stopped by user");
		setSimulationStatus("stopped");
		appendSimulationLogs([{ level: "warn", message: "[Simulation] Stop requested by user." }]);
	};

	const handleImportFileChange = async (event: ChangeEvent<HTMLInputElement>) => {
		const file = event.target.files?.[0];
		if (!file) {
			return;
		}

		try {
			const verification = await verifyBbsPackage(file);
			if (verification.summary.status !== "verified") {
				setVerificationErrorDialog({
					open: true,
					title: "Import Rejected",
					description: "The imported package did not pass verification cleanly and was not loaded.",
					checks: verification.checks,
				});
				appendSystemLogs([
					{ level: "error", message: `Import rejected: ${file.name} did not pass package verification.` },
				]);
				expandPanel("bottom");
				return;
			}

			const importedPackage = await importBbsPackage(file);
			const importedPermissions = calculatePermissions(
				importedPackage.nodes,
				importedPackage.secretDeclarations,
				importedPackage.defaultVariables,
			);
			const importedVerificationChecks = createVerificationChecks({
				assets: importedPackage.assets,
				edges: importedPackage.edges,
				nodes: importedPackage.nodes,
				permissions: importedPermissions,
				defaultVariables: importedPackage.defaultVariables,
				secretDeclarations: importedPackage.secretDeclarations,
				scriptName: importedPackage.projectSettings.name,
				targetRuntime: importedPackage.projectSettings.targetRuntime,
			});
			const importedSummary = summarizeVerification(importedVerificationChecks);
			const importedSignature = createEditorVerificationSignature(
				importedPackage.projectSettings,
				importedPackage.nodes,
				importedPackage.edges,
				importedPackage.assets,
				importedPackage.secretDeclarations,
				importedPackage.defaultVariables,
			);

			abortSimulationLifecycle("imported package loaded");
			setProjectSettings(importedPackage.projectSettings);
			setAssets(importedPackage.assets);
			setDefaultVariables(importedPackage.defaultVariables);
			setSecretDeclarations(importedPackage.secretDeclarations);
			setSimulationSecretValues({});
			setEdgeStyle(importedPackage.edgeStyle);
			setNodes([
				...(importedPackage.nodes as ScriptFlowNode[]),
				...importedPackage.comments.map((comment) => toCommentFlowNode(comment)),
			]);
			setEdges(
				importedPackage.edges.map((edge) => ({
					...edge,
					type: toReactFlowEdgeType(importedPackage.edgeStyle),
					style: undefined,
				})),
			);
			setSelectedNodeId(null);
			setSelectedEdgeId(null);
			setSimulationOverrides([]);
			setSimulationStatus("idle");
			setSimulationLogs([]);
			setSimulationVariables([]);
			setLogs([]);
			setSystemLogs([
				...createConsoleLogs(
					importedPackage.projectSettings.name,
					importedPackage.projectSettings.targetRuntime,
					importedPermissions,
				),
				{
					level: importedSummary.status === "failed" ? "error" : importedSummary.status === "warning" ? "warn" : "info",
					message: `Imported ${file.name}: ${importedPackage.nodes.length} node${importedPackage.nodes.length === 1 ? "" : "s"}, ${importedPackage.edges.length} connection${importedPackage.edges.length === 1 ? "" : "s"}, ${importedPackage.assets.length} asset${importedPackage.assets.length === 1 ? "" : "s"}.`,
				},
			]);
			setVerificationRecord({ signature: importedSignature, status: importedSummary.status });
			expandPanel("bottom");
			setBottomPanelTab("system");
			appendSystemLogs([{ level: "info", message: `Import verified: ${file.name}` }]);
			setActiveTab("properties");
		} catch (error) {
			setVerificationErrorDialog({
				open: true,
				title: "Import Rejected",
				description: error instanceof Error ? error.message : "The imported package could not be read.",
				checks: [
					{
						id: "package-read",
						title: "Package Read",
						description: "Checking that the package can be opened.",
						outcome: "failed",
						message: error instanceof Error ? error.message : "The package could not be read.",
					},
				],
			});
		} finally {
			event.target.value = "";
		}
	};

	const handleSaveProjectSettings = (settings: ProjectSettings) => {
		setProjectSettings(settings);
		appendSystemLogs([
			{
				level: "info",
				message: `Project settings saved: ${settings.name} (${settings.targetRuntime})`,
			},
		]);
		expandPanel("bottom");
	};

	const handleAssetsChange = (nextAssets: EditorAsset[]) => {
		setAssets(nextAssets);
		appendSystemLogs([
			{
				level: "info",
				message: `Assets updated: ${nextAssets.length} file${nextAssets.length === 1 ? "" : "s"} attached.`,
			},
		]);
		expandPanel("bottom");
	};

	const handleAddSimulationOverride = (nodeId: string) => {
		if (!nodeId) {
			return;
		}

		setSimulationOverrides((currentOverrides) => {
			if (currentOverrides.some((override) => override.nodeId === nodeId)) {
				return currentOverrides;
			}

			return [...currentOverrides, { nodeId, outcome: "failed" }];
		});
	};

	const handleUpdateSimulationOverride = (nodeId: string, outcome: SimulationOverrideOutcome) => {
		setSimulationOverrides((currentOverrides) =>
			currentOverrides.map((override) => (override.nodeId === nodeId ? { ...override, outcome } : override)),
		);
	};

	const handleRemoveSimulationOverride = (nodeId: string) => {
		setSimulationOverrides((currentOverrides) => currentOverrides.filter((override) => override.nodeId !== nodeId));
	};

	const showManualTriggerLimitError = () => {
		setVerificationErrorDialog({
			open: true,
			title: "Manual Trigger Already Exists",
			description: "A script can only contain one Manual Trigger node.",
			checks: [
				{
					id: "manual-trigger-limit",
					title: "Manual Trigger Limit",
					description: "Checking manual trigger count before creating the node.",
					outcome: "failed",
					message: "Remove the existing Manual Trigger before adding another one.",
				},
			],
		});
		appendSystemLogs([
			{ level: "error", message: "Manual Trigger was not added: only one Manual Trigger is allowed." },
		]);
		expandPanel("bottom");
	};

	const handleClearBottomPanelTab = (tab: Exclude<BottomPanelTab, "variables">) => {
		if (tab === "system") {
			setSystemLogs([]);
			return;
		}

		if (tab === "output") {
			setLogs([]);
			return;
		}

		if (tab === "simulation") {
			setSimulationLogs([]);
			setSimulationVariables([]);
		}
	};

	const handleFollowBottomPanelTab = (tab: Exclude<BottomPanelTab, "variables">, enabled: boolean) => {
		setBottomPanelFollow((currentFollow) => ({ ...currentFollow, [tab]: enabled }));
	};

	const handleEdgeStyleChange = (nextEdgeStyle: EditorEdgeStyle) => {
		setEdgeStyle(nextEdgeStyle);
		setEdges((currentEdges) =>
			currentEdges.map((edge) => ({
				...edge,
				type: toReactFlowEdgeType(nextEdgeStyle),
				style: undefined,
			})),
		);
	};

	const handleAddBlock = (item: PaletteItem, centerPosition = viewportCenter) => {
		if (item.actionType === "trigger.manual" && hasManualTrigger(scriptNodes)) {
			showManualTriggerLimitError();
			return;
		}

		const node = createNodeFromPaletteItem(item, scriptNodes.length, {
			position: centerPosition ? getCenteredScriptNodePosition(centerPosition) : undefined,
		}) as ScriptFlowNode;
		setNodes((currentNodes) => [
			...currentNodes.map((currentNode) => ({ ...currentNode, selected: false })),
			{ ...node, selected: true },
		]);
		setSelectedNodeId((currentNodeId) => (currentNodeId === node.id ? currentNodeId : node.id));
		setSelectedEdgeId((currentEdgeId) => (currentEdgeId === null ? currentEdgeId : null));
		setActiveTab("properties");
	};

	const handleDropPaletteNode = (actionType: string, position: XYPosition) => {
		const item = paletteItemByActionType.get(actionType);
		if (!item) {
			return;
		}

		handleAddBlock(item, position);
	};

	const handleCreateComment = (position: XYPosition) => {
		const commentNode = createCommentFlowNode({
			x: position.x - DEFAULT_COMMENT_SIZE.width / 2,
			y: position.y - 20,
		});

		setNodes((currentNodes) => [...currentNodes.map((node) => ({ ...node, selected: false })), commentNode]);
		setSelectedNodeId(null);
		setSelectedEdgeId(null);
	};

	const handleUpdateComment = (commentId: string, patch: Partial<CommentNodeData>) => {
		setNodes((currentNodes) => {
			let changed = false;
			const nextNodes = currentNodes.map((node) => {
				if (!isCommentFlowNode(node) || node.id !== commentId) {
					return node;
				}

				const nextData = { ...node.data, ...patch };
				if (areCommentNodeDataEqual(node.data, nextData)) {
					return node;
				}

				changed = true;
				return {
					...node,
					data: nextData,
					style: {
						...node.style,
						width: nextData.size.width,
						height: nextData.size.height,
					},
				};
			});

			return changed ? nextNodes : currentNodes;
		});
	};

	const handleDeleteComment = (commentId: string) => {
		setNodes((currentNodes) => currentNodes.filter((node) => node.id !== commentId));
	};

	const handleSpawnDevelopmentNodes = () => {
		const developmentNodes = createDevelopmentEditorNodes(viewportCenter ?? undefined);
		setNodes(developmentNodes as ScriptFlowNode[]);
		setEdges([]);
		setSelectedNodeId(null);
		setSelectedEdgeId(null);
		setActiveTab("properties");
		appendSystemLogs([
			{
				level: "info",
				message: `Development node grid spawned: ${developmentNodes.length} nodes.`,
			},
		]);
		expandPanel("bottom");
	};

	const handleUpdateNodeConfig = (nodeId: string, key: string, value: JsonValue) => {
		const nextSwitchOutputs = key === "cases" ? createSwitchOutputPorts(getSwitchCaseRowsFromValue(value)) : null;

		setNodes((currentNodes) =>
			currentNodes.map((node) => {
				if (!isScriptFlowNode(node) || node.id !== nodeId) {
					return node;
				}

				const outputs =
					node.data.actionType === "control.switch" && nextSwitchOutputs ? nextSwitchOutputs : node.data.outputs;

				return {
					...node,
					data: {
						...node.data,
						config: {
							...node.data.config,
							[key]: value,
						},
						outputs,
					},
				};
			}),
		);

		if (nextSwitchOutputs) {
			const validOutputIds = new Set(nextSwitchOutputs.map((output) => output.id));
			setEdges((currentEdges) => {
				const remainingEdges = currentEdges.filter(
					(edge) => edge.source !== nodeId || validOutputIds.has(edge.sourceHandle ?? ""),
				);
				if (selectedEdgeId && !remainingEdges.some((edge) => edge.id === selectedEdgeId)) {
					setSelectedEdgeId(null);
				}

				return normalizeEdgeExecutionOrders(remainingEdges);
			});
		}
	};

	const handleDeleteNode = (nodeId: string) => {
		setNodes((currentNodes) => currentNodes.filter((node) => node.id !== nodeId));
		setEdges((currentEdges) => {
			const remainingEdges = currentEdges.filter((edge) => edge.source !== nodeId && edge.target !== nodeId);
			if (selectedEdgeId && !remainingEdges.some((edge) => edge.id === selectedEdgeId)) {
				setSelectedEdgeId(null);
			}

			return normalizeEdgeExecutionOrders(remainingEdges);
		});

		if (selectedNodeId === nodeId) {
			setSelectedNodeId(null);
		}
	};

	const handleDeleteEdge = (edgeId: string) => {
		setEdges((currentEdges) => normalizeEdgeExecutionOrders(currentEdges.filter((edge) => edge.id !== edgeId)));
		if (selectedEdgeId === edgeId) {
			setSelectedEdgeId(null);
		}
	};

	const handleCopyGraph = (nodeId?: string) => {
		const selectedNodes = nodes.filter((node) => node.selected);
		const targetNode = nodeId ? nodes.find((node) => node.id === nodeId) : undefined;
		const nodesToCopy = targetNode && !targetNode.selected ? [targetNode] : selectedNodes;
		if (nodesToCopy.length === 0) {
			return false;
		}

		setClipboard({ type: "graph", fragment: createGraphFragment(nodesToCopy, edges) });
		return true;
	};
	const handleCopyNode = (nodeId: string) => {
		handleCopyGraph(nodeId);
	};

	const handleDuplicateNode = (nodeId: string) => {
		const node = nodes.find((currentNode) => currentNode.id === nodeId);
		if (!node) {
			return;
		}

		if (isScriptFlowNode(node) && node.data.actionType === "trigger.manual" && hasManualTrigger(scriptNodes)) {
			showManualTriggerLimitError();
			return;
		}

		const duplicatedNode = createGraphNodeCopy(node, {
			x: node.position.x + DUPLICATE_OFFSET,
			y: node.position.y + DUPLICATE_OFFSET,
		}) as EditorFlowNode;

		setNodes((currentNodes) => [...currentNodes, duplicatedNode]);
		setSelectedNodeId(isScriptFlowNode(duplicatedNode) ? duplicatedNode.id : null);
		setSelectedEdgeId(null);
		if (isScriptFlowNode(duplicatedNode)) {
			setActiveTab("properties");
		}
	};

	const handlePasteClipboard = (centerPosition: XYPosition) => {
		if (!clipboard) {
			return;
		}

		const clipboardScriptNodes = clipboard.fragment.nodes.filter(isScriptFlowNode);
		if (hasManualTrigger(clipboardScriptNodes) && hasManualTrigger(scriptNodes)) {
			showManualTriggerLimitError();
			return;
		}

		const pastedFragment = createGraphFragmentCopy(clipboard.fragment, centerPosition);
		setNodes((currentNodes) => [
			...currentNodes.map((node) => ({ ...node, selected: false })),
			...pastedFragment.nodes,
		]);
		setEdges((currentEdges) =>
			normalizeEdgeExecutionOrders([
				...currentEdges.map((edge) => ({ ...edge, selected: false })),
				...pastedFragment.edges,
			]),
		);
		setSelectedNodeId(pastedFragment.nodes.find(isScriptFlowNode)?.id ?? null);
		setSelectedEdgeId(null);
		if (pastedFragment.nodes.some(isScriptFlowNode)) {
			setActiveTab("properties");
		}
	};

	handleCopyGraphRef.current = handleCopyGraph;

	const handleNodesDelete = (deletedNodes: EditorFlowNode[]) => {
		const deletedNodeIds = new Set(deletedNodes.filter(isScriptFlowNode).map((node) => node.id));
		setEdges((currentEdges) =>
			normalizeEdgeExecutionOrders(
				currentEdges.filter((edge) => {
					const shouldDelete = deletedNodeIds.has(edge.source) || deletedNodeIds.has(edge.target);
					if (shouldDelete && edge.id === selectedEdgeId) {
						setSelectedEdgeId(null);
					}

					return !shouldDelete;
				}),
			),
		);

		if (selectedNodeId && deletedNodeIds.has(selectedNodeId)) {
			setSelectedNodeId(null);
		}
	};

	const handleEdgesDelete = (deletedEdges: Array<{ id: string }>) => {
		if (selectedEdgeId && deletedEdges.some((edge) => edge.id === selectedEdgeId)) {
			setSelectedEdgeId(null);
		}
	};

	const handleReorderEdges = (orderedEdgeIds: string[]) => {
		setEdges((currentEdges) => reorderEdgeExecutionGroup(currentEdges, orderedEdgeIds));
	};

	const handleSelectEdge = (edgeId: string | null) => {
		setSelectedEdgeId(edgeId);
		if (edgeId) {
			setActiveTab("properties");
		}
	};

	useEffect(() => {
		const handleEditorKeyDown = (event: KeyboardEvent) => {
			if (isEditableShortcutTarget(event.target) || !(event.ctrlKey || event.metaKey)) {
				return;
			}

			const key = event.key.toLowerCase();
			if (key === "c") {
				if (hasBrowserTextSelection()) {
					return;
				}

				if (handleCopyGraphRef.current()) {
					event.preventDefault();
				}
				return;
			}
		};

		window.addEventListener("keydown", handleEditorKeyDown);

		return () => window.removeEventListener("keydown", handleEditorKeyDown);
	}, []);

	return (
		<div className="flex h-dvh min-h-0 select-none flex-col overflow-hidden bg-baud-bg text-baud-text">
			<TopBar
				importInputRef={importInputRef}
				leftCollapsed={collapsed.left}
				leftWidth={sizes.left}
				rightCollapsed={collapsed.right}
				rightWidth={sizes.right}
				targetRuntime={projectSettings.targetRuntime}
				verificationStatus={verificationRecord.status}
				onAssetEditorClick={() => setAssetEditorOpen(true)}
				onImportClick={() => importInputRef.current?.click()}
				onImportFileChange={handleImportFileChange}
				onExportClick={handleExport}
				onHelpClick={() => setHelpOpen(true)}
				onProjectSettingsClick={() => setProjectSettingsOpen(true)}
				onVerifyClick={handleVerify}
			/>

			<div className="flex min-h-0 flex-1">
				<BlockLibrary
					collapsed={collapsed.left}
					width={sizes.left}
					targetRuntime={projectSettings.targetRuntime}
					onAddBlock={handleAddBlock}
					onToggleCollapsed={() => togglePanel("left")}
				/>
				{!collapsed.left && (
					<ResizeHandle
						axis="horizontal"
						label="Resize block library"
						onPointerDown={(event) => startResize("left", event)}
					/>
				)}

				<main className="flex min-h-0 min-w-0 flex-1 flex-col overflow-hidden">
					<FlowCanvas
						nodes={nodes}
						edges={edges}
						selectedEdgeId={selectedEdgeId}
						onNodesChange={onNodesChange}
						onEdgesChange={handleEdgesChange}
						onEdgesCommit={setEdges}
						onNodesDelete={handleNodesDelete}
						onEdgesDelete={handleEdgesDelete}
						onSelectNode={(nodeId) => {
							setSelectedNodeId(nodeId);
							if (nodeId && activeTab === "simulator") {
								setActiveTab("properties");
							}
						}}
						onSelectEdge={handleSelectEdge}
						canPaste={clipboard !== null}
						onCopyNode={handleCopyNode}
						onDeleteNode={handleDeleteNode}
						onDeleteEdge={handleDeleteEdge}
						onCreateComment={handleCreateComment}
						onDeleteComment={handleDeleteComment}
						onUpdateComment={handleUpdateComment}
						onDuplicateNode={handleDuplicateNode}
						onPaste={handlePasteClipboard}
						onDropPaletteNode={handleDropPaletteNode}
						edgeStyle={edgeStyle}
						onEdgeStyleChange={handleEdgeStyleChange}
						onSpawnDevelopmentNodes={handleSpawnDevelopmentNodes}
						showDevelopmentNodeSpawner={isDevelopmentGraphEnabled}
						onViewportCenterChange={setViewportCenter}
						targetRuntime={projectSettings.targetRuntime}
					/>
					{!collapsed.bottom && (
						<ResizeHandle
							axis="vertical"
							label="Resize output console"
							onPointerDown={(event) => startResize("bottom", event)}
						/>
					)}
					<OutputConsole
						activeTab={bottomPanelTab}
						follow={bottomPanelFollow}
						logs={logs}
						open={!collapsed.bottom}
						systemLogs={systemLogs}
						simulationLogs={simulationLogs}
						variables={variableEntries}
						defaultVariables={defaultVariables}
						secretDeclarations={secretDeclarations}
						simulationSecretValues={simulationSecretValues}
						height={sizes.bottom}
						onClearTab={handleClearBottomPanelTab}
						onFollowChange={handleFollowBottomPanelTab}
						onTabChange={setBottomPanelTab}
						onToggle={() => togglePanel("bottom")}
						onDefaultVariablesChange={setDefaultVariables}
						onSecretDeclarationsChange={setSecretDeclarations}
						onSimulationSecretValueChange={(name, value) =>
							setSimulationSecretValues((current) => {
								if (value === "") {
									const next = { ...current };
									delete next[name];
									return next;
								}
								return { ...current, [name]: value };
							})
						}
					/>
				</main>

				{!collapsed.right && (
					<ResizeHandle
						axis="horizontal"
						label="Resize inspector"
						onPointerDown={(event) => startResize("right", event)}
					/>
				)}
				<Inspector
					activeTab={activeTab}
					assets={assets}
					edges={edges}
					nodes={scriptNodes}
					selectedEdge={selectedEdge}
					selectedNode={selectedNode}
					simulationOverrides={simulationOverrides}
					simulationSettings={simulationSettings}
					simulationStatus={simulationStatus}
					variables={variableEntries}
					width={sizes.right}
					collapsed={collapsed.right}
					onAddSimulationOverride={handleAddSimulationOverride}
					onRemoveSimulationOverride={handleRemoveSimulationOverride}
					onSimulationSettingsChange={setSimulationSettings}
					onStopSimulation={handleStopSimulation}
					onTriggerSimulation={handleTriggerSimulation}
					onTabChange={setActiveTab}
					onUpdateNodeConfig={handleUpdateNodeConfig}
					onUpdateSimulationOverride={handleUpdateSimulationOverride}
					onDeleteEdge={handleDeleteEdge}
					onDeleteNode={handleDeleteNode}
					onReorderEdges={handleReorderEdges}
					onSelectEdge={handleSelectEdge}
					onToggleCollapsed={() => togglePanel("right")}
				/>
			</div>

			<StatusBar nodes={scriptNodes} edges={edges} riskLevel={riskLevel} />
			<VerificationModal
				checks={verificationChecks}
				open={verificationOpen}
				onClose={() => setVerificationOpen(false)}
			/>
			<VerificationErrorModal
				checks={verificationErrorDialog.checks}
				description={verificationErrorDialog.description}
				open={verificationErrorDialog.open}
				title={verificationErrorDialog.title}
				onClose={() => setVerificationErrorDialog((currentDialog) => ({ ...currentDialog, open: false }))}
			/>
			<ProjectSettingsModal
				open={projectSettingsOpen}
				settings={projectSettings}
				onClose={() => setProjectSettingsOpen(false)}
				onSave={handleSaveProjectSettings}
			/>
			<AssetEditorModal
				assets={assets}
				open={assetEditorOpen}
				onAssetsChange={handleAssetsChange}
				onClose={() => setAssetEditorOpen(false)}
			/>
			<ExportWizardModal
				capabilities={capabilities}
				checks={verificationChecks}
				exportSummary={exportSummary}
				open={exportOpen}
				permissions={permissions}
				projectSettings={normalizedProjectSettings}
				riskLevel={riskLevel}
				onClose={() => setExportOpen(false)}
				onDownload={handleDownloadExport}
				onVerificationComplete={handleExportVerificationComplete}
			/>
			<HelpModal open={helpOpen} onClose={() => setHelpOpen(false)} />
			<SimulationMessageBoxDialog messageBox={simulationMessageBox} onSelect={handleSimulationMessageBoxSelect} />
			<Toaster position="bottom-right" closeButton richColors />
		</div>
	);
}

function isScriptFlowNode(node: EditorFlowNode): node is ScriptFlowNode {
	return node.type !== "commentNode";
}
