"use client";

import { type Edge, useEdgesState, useNodesState, type XYPosition } from "@xyflow/react";
import type { ChangeEvent } from "react";
import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import type { CommentFlowNode } from "@/components/canvas/comment-card";
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
import { createSwitchOutputPorts, getSwitchCaseRowsFromValue } from "@/data/nodes/definitions/rows";
import {
	createDevelopmentEditorNodes,
	createInitialEditorEdges,
	createInitialEditorNodes,
	isDevelopmentGraphEnabled,
} from "@/data/nodes/development-graph";
import { createNodeFromPaletteItem, getFlatPaletteItems } from "@/data/nodes/registry";
import { isDesktopTargetRuntime } from "@/data/project/runtimes";
import { useEditorPanelSizes } from "@/hooks/use-editor-panel-sizes";
import type {
	CommentNodeData,
	EditorAsset,
	EditorComment,
	InspectorTab,
	JsonValue,
	LogEntry,
	PaletteItem,
	ProjectSettings,
	SimulationOverride,
	SimulationOverrideOutcome,
	SimulationRunStatus,
	SimulationSettings,
	SimulationTraceEntry,
	SimulationTriggerPayload,
	SimulationVariableSnapshot,
} from "@/lib/types";
import {
	calculateCapabilities,
	calculatePermissions,
	calculateRiskLevel,
	createConsoleLogs,
	createExportSummary,
} from "@/utils/analysis";
import { exportBbsPackage, importBbsPackage, verifyBbsPackage } from "@/utils/bbs-package";
import {
	cloneGraphValue,
	createEditorVerificationSignature,
	createGraphNodeCopy,
	DUPLICATE_OFFSET,
	getCenteredScriptNodePosition,
	hasManualTrigger,
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
	node: ScriptFlowNode;
	type: "node";
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
const DEFAULT_COMMENT_SIZE = {
	width: 280,
	height: 156,
};
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
	minimumRunnerVersion: "0.1.0",
};

export function EditorPage() {
	const handleCopyNodeRef = useRef<(nodeId: string) => void>(() => undefined);
	const handlePasteClipboardRef = useRef<(position: XYPosition) => void>(() => undefined);
	const clipboardRef = useRef<EditorClipboard | null>(null);
	const importInputRef = useRef<HTMLInputElement>(null);
	const nodesRef = useRef<ScriptFlowNode[]>([]);
	const selectedNodeIdRef = useRef<string | null>(null);
	const simulationLifecycleRef = useRef<SimulationLifecycle>({ abortController: null, active: false, runId: 0 });
	const simulationMessageBoxResolveRef = useRef<((button: string) => void) | null>(null);
	const viewportCenterRef = useRef<XYPosition | null>(null);
	const initialNodes = useMemo<EditorFlowNode[]>(() => createInitialEditorNodes() as ScriptFlowNode[], []);
	const initialEdges = useMemo(() => createInitialEditorEdges(), []);
	const [projectSettings, setProjectSettings] = useState<ProjectSettings>(DEFAULT_PROJECT_SETTINGS);
	const [activeTab, setActiveTab] = useState<InspectorTab>("properties");
	const [selectedNodeId, setSelectedNodeId] = useState<string | null>(null);
	const [selectedEdgeId, setSelectedEdgeId] = useState<string | null>(null);
	const [bottomPanelTab, setBottomPanelTab] = useState<BottomPanelTab>("system");
	const [bottomPanelOpen, setBottomPanelOpen] = useState(true);
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
	const [edges, setEdges, onEdgesChange] = useEdgesState<Edge>(initialEdges);
	const { sizes, startResize } = useEditorPanelSizes();

	const scriptNodes = useMemo(() => nodes.filter(isScriptFlowNode), [nodes]);
	const comments = useMemo(() => nodes.filter(isCommentFlowNode).map(toEditorComment), [nodes]);

	nodesRef.current = scriptNodes;
	selectedNodeIdRef.current = selectedNodeId ?? scriptNodes.find((node) => node.selected)?.id ?? null;
	clipboardRef.current = clipboard;
	viewportCenterRef.current = viewportCenter;

	const selectedNode = useMemo(
		() => scriptNodes.find((node) => node.id === selectedNodeId) ?? null,
		[scriptNodes, selectedNodeId],
	);
	const permissions = useMemo(() => calculatePermissions(scriptNodes), [scriptNodes]);
	const capabilities = useMemo(() => calculateCapabilities(scriptNodes), [scriptNodes]);
	const riskLevel = useMemo(() => calculateRiskLevel(permissions), [permissions]);
	const exportSummary = useMemo(
		() => createExportSummary(projectSettings.name, projectSettings.targetRuntime, assets),
		[assets, projectSettings.name, projectSettings.targetRuntime],
	);
	const verificationChecks = useMemo(
		() =>
			createVerificationChecks({
				assets,
				edges,
				nodes: scriptNodes,
				permissions,
				scriptName: projectSettings.name,
				targetRuntime: projectSettings.targetRuntime,
			}),
		[assets, edges, scriptNodes, permissions, projectSettings.name, projectSettings.targetRuntime],
	);
	const verificationSignature = useMemo(
		() => createEditorVerificationSignature(projectSettings, scriptNodes, edges, assets),
		[projectSettings, scriptNodes, edges, assets],
	);
	const variableEntries = useMemo(
		() => createVariablePanelEntries(projectSettings, scriptNodes, simulationVariables),
		[projectSettings, scriptNodes, simulationVariables],
	);
	const isDesktopTarget = isDesktopTargetRuntime(projectSettings.targetRuntime);

	const normalizedProjectSettings = {
		...projectSettings,
		name: projectSettings.name.trim() || "untitled-script",
		minimumRunnerVersion: projectSettings.minimumRunnerVersion.trim() || "0.1.0",
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
			setBottomPanelOpen(true);
		},
		[appendSystemLogs, verificationSignature],
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
		setBottomPanelOpen(true);
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
			setSimulationStatus("running");

			try {
				const run = await createSimulationRun({
					assets,
					nodes: scriptNodes,
					edges,
					overrides: simulationOverrides,
					projectSettings,
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
			scriptNodes,
			simulationOverrides,
			simulationSettings.speed,
		],
	);

	const handleSimulate = async () => {
		if (simulationLifecycleRef.current.active) {
			handleStopSimulation();
			return;
		}

		const summary = summarizeVerification(verificationChecks);
		setVerificationRecord({ signature: verificationSignature, status: summary.status });
		if (summary.status === "failed") {
			setVerificationErrorDialog({
				open: true,
				title: "Simulation Blocked",
				description: "The current script failed verification and cannot be simulated.",
				checks: verificationChecks,
			});
			appendSimulationLogs([{ level: "error", message: "[Simulation] Simulation blocked: verification failed." }]);
			appendSystemLogs([
				{
					level: "error",
					message: `Simulation blocked: verification failed with ${summary.failed} failed check${summary.failed === 1 ? "" : "s"}.`,
				},
			]);
			setBottomPanelOpen(true);
			return;
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
			return;
		}

		const abortController = new AbortController();
		startSimulationLifecycle(abortController);
		setActiveTab("simulator");
		setBottomPanelOpen(true);
		setSimulationStatus("waiting");
		setSimulationLogs([
			{
				level: "info",
				message: `[Simulation] Waiting for input from ${triggerNodes.length} trigger${triggerNodes.length === 1 ? "" : "s"}. Schedule triggers run automatically while the simulation is active.`,
			},
		]);
		setSimulationVariables([]);
	};

	const handleTriggerSimulation = useCallback(
		(triggerNodeId: string, payload: SimulationTriggerPayload) => {
			const lifecycle = simulationLifecycleRef.current;
			if (simulationStatus !== "waiting" || !lifecycle.abortController) {
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
		[runSimulationTrigger, simulationStatus],
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
				setBottomPanelOpen(true);
				return;
			}

			const importedPackage = await importBbsPackage(file);
			const importedPermissions = calculatePermissions(importedPackage.nodes);
			const importedVerificationChecks = createVerificationChecks({
				assets: importedPackage.assets,
				edges: importedPackage.edges,
				nodes: importedPackage.nodes,
				permissions: importedPermissions,
				scriptName: importedPackage.projectSettings.name,
				targetRuntime: importedPackage.projectSettings.targetRuntime,
			});
			const importedSummary = summarizeVerification(importedVerificationChecks);
			const importedSignature = createEditorVerificationSignature(
				importedPackage.projectSettings,
				importedPackage.nodes,
				importedPackage.edges,
				importedPackage.assets,
			);

			abortSimulationLifecycle("imported package loaded");
			setProjectSettings(importedPackage.projectSettings);
			setAssets(importedPackage.assets);
			setNodes([
				...(importedPackage.nodes as ScriptFlowNode[]),
				...importedPackage.comments.map((comment) => toCommentFlowNode(comment)),
			]);
			setEdges(importedPackage.edges);
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
			setBottomPanelOpen(true);
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
		setBottomPanelOpen(true);
	};

	const handleAssetsChange = (nextAssets: EditorAsset[]) => {
		setAssets(nextAssets);
		appendSystemLogs([
			{
				level: "info",
				message: `Assets updated: ${nextAssets.length} file${nextAssets.length === 1 ? "" : "s"} attached.`,
			},
		]);
		setBottomPanelOpen(true);
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
		setBottomPanelOpen(true);
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
		setBottomPanelOpen(true);
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

				return remainingEdges;
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

			return remainingEdges;
		});

		if (selectedNodeId === nodeId) {
			setSelectedNodeId(null);
		}
	};

	const handleDeleteEdge = (edgeId: string) => {
		setEdges((currentEdges) => currentEdges.filter((edge) => edge.id !== edgeId));
		if (selectedEdgeId === edgeId) {
			setSelectedEdgeId(null);
		}
	};

	const handleCopyNode = (nodeId: string) => {
		const node = scriptNodes.find((currentNode) => currentNode.id === nodeId);
		if (!node) {
			return;
		}

		setClipboard({ type: "node", node: cloneGraphValue(node) });
	};

	const handleDuplicateNode = (nodeId: string) => {
		const node = scriptNodes.find((currentNode) => currentNode.id === nodeId);
		if (!node) {
			return;
		}

		if (node.data.actionType === "trigger.manual" && hasManualTrigger(scriptNodes)) {
			showManualTriggerLimitError();
			return;
		}

		const duplicatedNode = createGraphNodeCopy(node, {
			x: node.position.x + DUPLICATE_OFFSET,
			y: node.position.y + DUPLICATE_OFFSET,
		}) as ScriptFlowNode;

		setNodes((currentNodes) => [...currentNodes, duplicatedNode]);
		setSelectedNodeId(duplicatedNode.id);
		setSelectedEdgeId(null);
		setActiveTab("properties");
	};

	const handlePasteClipboard = (position: XYPosition) => {
		if (!clipboard) {
			return;
		}

		if (clipboard.node.data.actionType === "trigger.manual" && hasManualTrigger(scriptNodes)) {
			showManualTriggerLimitError();
			return;
		}

		const pastedNode = createGraphNodeCopy(clipboard.node, position) as ScriptFlowNode;
		setNodes((currentNodes) => [...currentNodes, pastedNode]);
		setSelectedNodeId(pastedNode.id);
		setSelectedEdgeId(null);
		setActiveTab("properties");
	};

	handleCopyNodeRef.current = handleCopyNode;
	handlePasteClipboardRef.current = handlePasteClipboard;

	const handleNodesDelete = (deletedNodes: EditorFlowNode[]) => {
		const deletedNodeIds = new Set(deletedNodes.filter(isScriptFlowNode).map((node) => node.id));
		setEdges((currentEdges) =>
			currentEdges.filter((edge) => {
				const shouldDelete = deletedNodeIds.has(edge.source) || deletedNodeIds.has(edge.target);
				if (shouldDelete && edge.id === selectedEdgeId) {
					setSelectedEdgeId(null);
				}

				return !shouldDelete;
			}),
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

				const selectedNodeId =
					selectedNodeIdRef.current ?? nodesRef.current.find((currentNode) => currentNode.selected)?.id ?? null;

				if (selectedNodeId) {
					event.preventDefault();
					handleCopyNodeRef.current(selectedNodeId);
				}
				return;
			}

			if (key === "v" && clipboardRef.current) {
				event.preventDefault();
				handlePasteClipboardRef.current(
					viewportCenterRef.current ? getCenteredScriptNodePosition(viewportCenterRef.current) : { x: 0, y: 0 },
				);
			}
		};

		window.addEventListener("keydown", handleEditorKeyDown);

		return () => window.removeEventListener("keydown", handleEditorKeyDown);
	}, []);

	return (
		<div className="flex h-dvh min-h-0 select-none flex-col overflow-hidden bg-baud-bg text-baud-text">
			<TopBar
				importInputRef={importInputRef}
				isSimulationRunning={simulationStatus === "running" || simulationStatus === "waiting"}
				leftWidth={sizes.left}
				rightWidth={sizes.right}
				targetRuntime={projectSettings.targetRuntime}
				verificationStatus={verificationRecord.status}
				onAssetEditorClick={() => setAssetEditorOpen(true)}
				onImportClick={() => importInputRef.current?.click()}
				onImportFileChange={handleImportFileChange}
				onExportClick={handleExport}
				onHelpClick={() => setHelpOpen(true)}
				onProjectSettingsClick={() => setProjectSettingsOpen(true)}
				onSimulateClick={handleSimulate}
				onStopSimulationClick={handleStopSimulation}
				onVerifyClick={handleVerify}
			/>

			<div className="flex min-h-0 flex-1">
				<BlockLibrary width={sizes.left} isDesktopTarget={isDesktopTarget} onAddBlock={handleAddBlock} />
				<ResizeHandle
					axis="horizontal"
					label="Resize block library"
					onPointerDown={(event) => startResize("left", event)}
				/>

				<main className="flex min-h-0 min-w-0 flex-1 flex-col overflow-hidden">
					<FlowCanvas
						nodes={nodes}
						edges={edges}
						selectedEdgeId={selectedEdgeId}
						onNodesChange={onNodesChange}
						onEdgesChange={onEdgesChange}
						onEdgesCommit={setEdges}
						onNodesDelete={handleNodesDelete}
						onEdgesDelete={handleEdgesDelete}
						onSelectNode={(nodeId) => {
							setSelectedNodeId(nodeId);
							if (nodeId && activeTab === "simulator") {
								setActiveTab("properties");
							}
						}}
						onSelectEdge={setSelectedEdgeId}
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
						onSpawnDevelopmentNodes={handleSpawnDevelopmentNodes}
						showDevelopmentNodeSpawner={isDevelopmentGraphEnabled}
						onViewportCenterChange={setViewportCenter}
					/>
					<ResizeHandle
						axis="vertical"
						label="Resize output console"
						onPointerDown={(event) => startResize("bottom", event)}
					/>
					<OutputConsole
						activeTab={bottomPanelTab}
						follow={bottomPanelFollow}
						logs={logs}
						open={bottomPanelOpen}
						systemLogs={systemLogs}
						simulationLogs={simulationLogs}
						variables={variableEntries}
						height={sizes.bottom}
						onClearTab={handleClearBottomPanelTab}
						onFollowChange={handleFollowBottomPanelTab}
						onTabChange={setBottomPanelTab}
						onToggle={() => setBottomPanelOpen((open) => !open)}
					/>
				</main>

				<ResizeHandle
					axis="horizontal"
					label="Resize inspector"
					onPointerDown={(event) => startResize("right", event)}
				/>
				<Inspector
					activeTab={activeTab}
					assets={assets}
					nodes={scriptNodes}
					projectSettings={projectSettings}
					selectedNode={selectedNode}
					simulationOverrides={simulationOverrides}
					simulationSettings={simulationSettings}
					simulationStatus={simulationStatus}
					width={sizes.right}
					onAddSimulationOverride={handleAddSimulationOverride}
					onRemoveSimulationOverride={handleRemoveSimulationOverride}
					onSimulationSettingsChange={setSimulationSettings}
					onTriggerSimulation={handleTriggerSimulation}
					onTabChange={setActiveTab}
					onUpdateNodeConfig={handleUpdateNodeConfig}
					onUpdateSimulationOverride={handleUpdateSimulationOverride}
					onDeleteNode={handleDeleteNode}
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

function createCommentFlowNode(position: XYPosition): CommentFlowNode {
	return toCommentFlowNode(
		{
			id: `c-${crypto.randomUUID()}`,
			text: "",
			position,
			size: DEFAULT_COMMENT_SIZE,
			color: "amber",
		},
		true,
	);
}

function toCommentFlowNode(comment: EditorComment, selected = false): CommentFlowNode {
	return {
		id: comment.id,
		type: "commentNode",
		position: comment.position,
		data: {
			editorOnly: true,
			text: comment.text,
			size: comment.size,
			color: comment.color,
		},
		style: {
			width: comment.size.width,
			height: comment.size.height,
		},
		className: "baud-comment-flow-node",
		connectable: false,
		deletable: true,
		draggable: true,
		dragHandle: ".baud-comment-drag-handle",
		selectable: true,
		selected,
		zIndex: 5,
	};
}

function toEditorComment(node: CommentFlowNode): EditorComment {
	return {
		id: node.id,
		text: node.data.text,
		position: node.position,
		size: node.data.size,
		color: node.data.color,
	};
}

function isCommentFlowNode(node: EditorFlowNode): node is CommentFlowNode {
	return node.type === "commentNode";
}

function isScriptFlowNode(node: EditorFlowNode): node is ScriptFlowNode {
	return node.type !== "commentNode";
}

function areCommentNodeDataEqual(first: CommentNodeData, second: CommentNodeData) {
	return (
		first.text === second.text &&
		first.color === second.color &&
		first.size.width === second.size.width &&
		first.size.height === second.size.height
	);
}
