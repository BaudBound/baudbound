import { Eye, EyeOff, KeyRound, Pencil, Plus, Trash2 } from "lucide-react";
import { useId, useMemo, useState } from "react";
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
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from "@/components/ui/select";
import { Switch } from "@/components/ui/switch";
import { secretSimulationValueError, validateSecretDeclaration } from "@/data/project/secrets";
import { type VariableType, variableTypes } from "@/data/project/variables";
import type { SecretDeclaration } from "@/lib/types";

type SecretReferenceManagerProps = {
	declarations: SecretDeclaration[];
	simulationValues: Record<string, string>;
	onDeclarationsChange: (declarations: SecretDeclaration[]) => void;
	onSimulationValueChange: (name: string, value: string) => void;
};

const emptyDeclaration: SecretDeclaration = {
	description: "",
	name: "",
	required: true,
	type: "string",
};

export function SecretReferenceManager({
	declarations,
	simulationValues,
	onDeclarationsChange,
	onSimulationValueChange,
}: SecretReferenceManagerProps) {
	const [editingName, setEditingName] = useState<string | null>(null);
	const [draft, setDraft] = useState<SecretDeclaration>(emptyDeclaration);
	const [dialogOpen, setDialogOpen] = useState(false);
	const [visibleValues, setVisibleValues] = useState<Set<string>>(new Set());
	const nameInputId = useId();
	const typeInputId = useId();
	const descriptionInputId = useId();
	const declarationError = useMemo(
		() => validateSecretDeclaration(draft, declarations, editingName ?? undefined),
		[draft, declarations, editingName],
	);

	const openCreate = () => {
		setEditingName(null);
		setDraft(emptyDeclaration);
		setDialogOpen(true);
	};
	const openEdit = (declaration: SecretDeclaration) => {
		setEditingName(declaration.name);
		setDraft(declaration);
		setDialogOpen(true);
	};
	const save = () => {
		if (declarationError) return;
		const normalized = { ...draft, name: draft.name.trim(), description: draft.description.trim() };
		if (editingName) {
			onDeclarationsChange(declarations.map((secret) => (secret.name === editingName ? normalized : secret)));
			if (editingName !== normalized.name && simulationValues[editingName] !== undefined) {
				onSimulationValueChange(normalized.name, simulationValues[editingName]);
				onSimulationValueChange(editingName, "");
			}
		} else {
			onDeclarationsChange([...declarations, normalized].sort((a, b) => a.name.localeCompare(b.name)));
		}
		setDialogOpen(false);
	};
	const remove = (name: string) => {
		onDeclarationsChange(declarations.filter((secret) => secret.name !== name));
		onSimulationValueChange(name, "");
		setVisibleValues((current) => {
			const next = new Set(current);
			next.delete(name);
			return next;
		});
	};

	return (
		<div className="border-b border-baud-border bg-baud-panel px-4 py-3">
			<div className="flex items-center justify-between gap-3">
				<div className="min-w-0">
					<div className="flex items-center gap-2 text-sm font-semibold text-baud-text">
						<KeyRound size={14} className="text-baud-amber" />
						Secret references
					</div>
					<p className="mt-0.5 text-xs text-baud-muted">
						Simulation values stay in this browser session and are never exported.
					</p>
				</div>
				<Button type="button" size="xs" variant="toolbar" onClick={openCreate}>
					<Plus size={12} /> Add secret
				</Button>
			</div>

			{declarations.length > 0 && (
				<div className="mt-3 grid gap-2">
					{declarations.map((declaration) => {
						const rawValue = simulationValues[declaration.name] ?? "";
						const valueError = secretSimulationValueError(declaration.type, rawValue);
						const visible = visibleValues.has(declaration.name);
						return (
							<div
								key={declaration.name}
								className="grid grid-cols-[minmax(150px,0.7fr)_100px_minmax(180px,1fr)_64px] items-start gap-2 rounded border border-baud-border bg-baud-soft p-2"
							>
								<div className="min-w-0">
									<div className="break-all font-mono text-sm text-baud-text">{declaration.name}</div>
									<div className="text-xs text-baud-muted">{declaration.required ? "Required" : "Optional"}</div>
								</div>
								<div className="font-mono text-xs text-baud-muted">{declaration.type}</div>
								<div className="min-w-0">
									<div className="flex gap-1">
										<Input
											type={visible ? "text" : "password"}
											value={rawValue}
											aria-invalid={Boolean(valueError)}
											placeholder="Simulation value"
											onChange={(event) => onSimulationValueChange(declaration.name, event.target.value)}
										/>
										<Button
											type="button"
											aria-label={visible ? "Hide simulation secret" : "Show simulation secret"}
											size="icon-sm"
											variant="ghost"
											onClick={() =>
												setVisibleValues((current) => {
													const next = new Set(current);
													visible ? next.delete(declaration.name) : next.add(declaration.name);
													return next;
												})
											}
										>
											{visible ? <EyeOff size={13} /> : <Eye size={13} />}
										</Button>
									</div>
									{valueError && <div className="mt-1 text-xs text-baud-danger">{valueError}</div>}
								</div>
								<div className="flex justify-end gap-1">
									<Button
										type="button"
										aria-label={`Edit ${declaration.name}`}
										size="icon-xs"
										variant="ghost"
										onClick={() => openEdit(declaration)}
									>
										<Pencil />
									</Button>
									<Button
										type="button"
										aria-label={`Delete ${declaration.name}`}
										size="icon-xs"
										variant="ghost"
										onClick={() => remove(declaration.name)}
									>
										<Trash2 />
									</Button>
								</div>
							</div>
						);
					})}
				</div>
			)}

			<Dialog open={dialogOpen} onOpenChange={setDialogOpen}>
				<DialogContent>
					<DialogHeader>
						<DialogTitle>{editingName ? "Edit secret reference" : "Add secret reference"}</DialogTitle>
						<DialogDescription>
							Only the declaration is saved in the package. Values are configured in the runner.
						</DialogDescription>
					</DialogHeader>
					<label htmlFor={nameInputId} className="grid gap-1 text-xs text-baud-muted">
						Name
						<Input
							id={nameInputId}
							value={draft.name}
							onChange={(event) => setDraft((current) => ({ ...current, name: event.target.value }))}
						/>
					</label>
					<label htmlFor={typeInputId} className="grid gap-1 text-xs text-baud-muted">
						Type
						<Select
							value={draft.type}
							onValueChange={(value) => setDraft((current) => ({ ...current, type: value as VariableType }))}
						>
							<SelectTrigger id={typeInputId} className="w-full">
								<SelectValue />
							</SelectTrigger>
							<SelectContent>
								{variableTypes.map((type) => (
									<SelectItem key={type} value={type}>
										{type}
									</SelectItem>
								))}
							</SelectContent>
						</Select>
					</label>
					<label htmlFor={descriptionInputId} className="grid gap-1 text-xs text-baud-muted">
						Description
						<Input
							id={descriptionInputId}
							value={draft.description}
							onChange={(event) => setDraft((current) => ({ ...current, description: event.target.value }))}
						/>
					</label>
					<div className="flex items-center justify-between gap-3 rounded border border-baud-border p-2 text-sm text-baud-text">
						<span>Required before a run starts</span>
						<Switch
							checked={draft.required}
							onCheckedChange={(required) => setDraft((current) => ({ ...current, required }))}
						/>
					</div>
					{declarationError && <div className="text-xs text-baud-danger">{declarationError}</div>}
					<DialogFooter>
						<Button type="button" variant="outline" onClick={() => setDialogOpen(false)}>
							Cancel
						</Button>
						<Button type="button" variant="primary" disabled={Boolean(declarationError)} onClick={save}>
							Save
						</Button>
					</DialogFooter>
				</DialogContent>
			</Dialog>
		</div>
	);
}
