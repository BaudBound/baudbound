import { Eye, EyeOff, KeyRound, Pencil, Plus, Trash2 } from "lucide-react";
import { useId, useMemo, useState } from "react";
import { Badge } from "@/components/ui/badge";
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
	reservedVariableNames?: ReadonlySet<string>;
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
	reservedVariableNames = new Set(),
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
		() => validateSecretDeclaration(draft, declarations, editingName ?? undefined, reservedVariableNames),
		[draft, declarations, editingName, reservedVariableNames],
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
		<section className="min-w-0 bg-baud-panel px-4 py-3">
			<div className="min-w-0">
				<div className="flex items-center gap-2 text-sm font-semibold text-baud-text">
					<KeyRound size={14} className="text-baud-amber" />
					Secret references
				</div>
				<p className="mt-0.5 text-xs text-baud-muted">
					Simulation values stay in this browser session and are never exported.
				</p>
				<Button
					type="button"
					size="xs"
					variant="toolbar"
					className="mt-2 hover:border-baud-amber/40 hover:bg-baud-amber/10 hover:text-baud-amber [&_svg]:transition-transform hover:[&_svg]:scale-110"
					onClick={openCreate}
				>
					<Plus size={12} /> Add secret
				</Button>
			</div>

			{declarations.length > 0 && (
				<div className="mt-2.5 grid gap-1.5">
					{declarations.map((declaration) => {
						const rawValue = simulationValues[declaration.name] ?? "";
						const valueError = secretSimulationValueError(declaration.type, rawValue);
						const visible = visibleValues.has(declaration.name);
						const inputId = `secret-simulation-${declaration.name}`;
						return (
							<article
								key={declaration.name}
								className="flex flex-col rounded border border-baud-border bg-baud-soft p-2"
							>
								<div className="flex min-w-0 items-start gap-2">
									<div className="min-w-0 flex-1">
										<div className="flex min-w-0 flex-wrap items-center gap-1">
											<div className="min-w-0 break-all font-mono text-sm font-semibold text-baud-text">
												{declaration.name}
											</div>
											<Badge variant="outline" className="font-mono text-baud-muted">
												Type: {declaration.type}
											</Badge>
											<Badge variant={declaration.required ? "medium" : "outline"}>
												{declaration.required ? "Required" : "Optional"}
											</Badge>
										</div>
										{declaration.description && (
											<p className="mt-0.5 text-xs leading-4 text-baud-muted">{declaration.description}</p>
										)}
									</div>
									<div className="flex shrink-0 justify-end gap-1">
										<Button
											type="button"
											aria-label={`Edit ${declaration.name}`}
											size="icon-xs"
											variant="ghost"
											className="text-baud-muted hover:bg-baud-blue/15 hover:text-baud-blue [&_svg]:transition-transform hover:[&_svg]:scale-110"
											onClick={() => openEdit(declaration)}
										>
											<Pencil />
										</Button>
										<Button
											type="button"
											aria-label={`Delete ${declaration.name}`}
											size="icon-xs"
											variant="ghost"
											className="text-baud-muted hover:bg-baud-danger/15 hover:text-baud-danger [&_svg]:transition-transform hover:[&_svg]:scale-110"
											onClick={() => remove(declaration.name)}
										>
											<Trash2 />
										</Button>
									</div>
								</div>
								<div className="mt-2 border-t border-baud-border/80 pt-1.5">
									<label
										htmlFor={inputId}
										className="mb-0.5 block text-[10px] font-semibold tracking-[0.08em] text-baud-muted uppercase"
									>
										Simulation value
									</label>
									<div className="flex gap-1">
										<Input
											id={inputId}
											className="h-7 px-2"
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
											className="text-baud-muted hover:bg-baud-blue/15 hover:text-baud-blue [&_svg]:transition-transform hover:[&_svg]:scale-110"
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
							</article>
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
		</section>
	);
}
