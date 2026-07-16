import { Database, Pencil, Plus, Trash2 } from "lucide-react";
import { useId, useMemo, useState } from "react";
import { VariableCodeInput, type VariableCompletion } from "@/components/inspector/variable-code-input";
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
import {
	createDefaultValue,
	defaultValueError,
	formatDefaultValue,
	parseDefaultValue,
	validateDefaultVariable,
} from "@/data/project/default-variables";
import { type VariableType, variableTypes } from "@/data/project/variables";
import type { DefaultVariable, SecretDeclaration } from "@/lib/types";

type DefaultVariableManagerProps = {
	secrets: SecretDeclaration[];
	variables: DefaultVariable[];
	onChange: (variables: DefaultVariable[]) => void;
};

const defaultValueCompletions: VariableCompletion[] = [];

function emptyVariable(): DefaultVariable {
	return {
		description: "",
		name: "",
		scope: "runtime",
		type: "string",
		value: createDefaultValue("string"),
	};
}

export function DefaultVariableManager({ secrets, variables, onChange }: DefaultVariableManagerProps) {
	const [dialogOpen, setDialogOpen] = useState(false);
	const [editingName, setEditingName] = useState<string | null>(null);
	const [draft, setDraft] = useState<DefaultVariable>(emptyVariable);
	const [rawValue, setRawValue] = useState("");
	const nameId = useId();
	const scopeId = useId();
	const typeId = useId();
	const valueId = useId();
	const descriptionId = useId();
	const declarationError = useMemo(
		() => validateDefaultVariable(draft, variables, secrets, editingName ?? undefined),
		[draft, editingName, secrets, variables],
	);
	const valueError = defaultValueError(draft.type, rawValue);

	const openCreate = () => {
		const variable = emptyVariable();
		setEditingName(null);
		setDraft(variable);
		setRawValue(formatDefaultValue(variable.type, variable.value));
		setDialogOpen(true);
	};
	const openEdit = (variable: DefaultVariable) => {
		setEditingName(variable.name);
		setDraft(structuredClone(variable));
		setRawValue(formatDefaultValue(variable.type, variable.value));
		setDialogOpen(true);
	};
	const changeType = (type: VariableType) => {
		const value = createDefaultValue(type);
		setDraft((current) => ({ ...current, type, value }));
		setRawValue(formatDefaultValue(type, value));
	};
	const save = () => {
		const value = parseDefaultValue(draft.type, rawValue);
		if (declarationError || valueError || value === undefined) return;
		const normalized: DefaultVariable = {
			...draft,
			description: draft.description.trim(),
			name: draft.name.trim(),
			value,
		};
		const next = editingName
			? variables.map((variable) => (variable.name === editingName ? normalized : variable))
			: [...variables, normalized];
		onChange(next.sort((left, right) => left.name.localeCompare(right.name)));
		setDialogOpen(false);
	};

	return (
		<section className="min-w-0 bg-baud-panel px-4 py-3">
			<div className="min-w-0">
				<div className="flex items-center gap-2 text-sm font-semibold text-baud-text">
					<Database size={14} className="text-baud-green" />
					Default variables
				</div>
				<p className="mt-0.5 text-xs text-baud-muted">
					Runtime values reset for every run. Persistent values are used only until the runner stores a value.
				</p>
				<Button
					type="button"
					size="xs"
					variant="toolbar"
					className="mt-2 hover:border-baud-green/40 hover:bg-baud-green/10 hover:text-baud-green [&_svg]:transition-transform hover:[&_svg]:scale-110"
					onClick={openCreate}
				>
					<Plus size={12} /> Add variable
				</Button>
			</div>

			{variables.length > 0 && (
				<div className="mt-2.5 grid gap-1.5">
					{variables.map((variable) => (
						<article key={variable.name} className="flex flex-col rounded border border-baud-border bg-baud-soft p-2">
							<div className="flex min-w-0 items-start gap-2">
								<div className="min-w-0 flex-1">
									<div className="flex min-w-0 flex-wrap items-center gap-1">
										<div className="min-w-0 break-all font-mono text-sm font-semibold text-baud-text">
											{variable.name}
										</div>
										<Badge variant="outline" className="font-mono text-baud-muted">
											Type: {variable.type}
										</Badge>
										<Badge variant={variable.scope === "persistent" ? "medium" : "low"}>Scope: {variable.scope}</Badge>
									</div>
									{variable.description && (
										<p className="mt-0.5 text-xs leading-4 text-baud-muted">{variable.description}</p>
									)}
								</div>
								<div className="flex shrink-0 justify-end gap-1">
									<Button
										type="button"
										aria-label={`Edit ${variable.name}`}
										size="icon-xs"
										variant="ghost"
										className="text-baud-muted hover:bg-baud-blue/15 hover:text-baud-blue [&_svg]:transition-transform hover:[&_svg]:scale-110"
										onClick={() => openEdit(variable)}
									>
										<Pencil />
									</Button>
									<Button
										type="button"
										aria-label={`Delete ${variable.name}`}
										size="icon-xs"
										variant="ghost"
										className="text-baud-muted hover:bg-baud-danger/15 hover:text-baud-danger [&_svg]:transition-transform hover:[&_svg]:scale-110"
										onClick={() => onChange(variables.filter((entry) => entry.name !== variable.name))}
									>
										<Trash2 />
									</Button>
								</div>
							</div>
							<div className="mt-2 border-t border-baud-border/80 pt-1.5">
								<div className="text-[10px] font-semibold tracking-[0.08em] text-baud-muted uppercase">
									Default value
								</div>
								<pre
									className="mt-0.5 min-h-7 min-w-0 whitespace-pre-wrap break-all rounded border border-baud-border bg-baud-panel/60 px-2 py-1.5 font-mono text-xs leading-4 text-baud-text"
									data-selectable-text="true"
								>
									{formatDefaultValue(variable.type, variable.value)}
								</pre>
							</div>
						</article>
					))}
				</div>
			)}

			<Dialog open={dialogOpen} onOpenChange={setDialogOpen}>
				<DialogContent className="sm:max-w-lg">
					<DialogHeader>
						<DialogTitle>{editingName ? "Edit default variable" : "Add default variable"}</DialogTitle>
						<DialogDescription>
							The value is saved in the script package and can be changed by Variable Operation nodes.
						</DialogDescription>
					</DialogHeader>
					<div className="grid gap-3 sm:grid-cols-2">
						<label htmlFor={nameId} className="grid gap-1 text-xs text-baud-muted">
							Name
							<Input
								id={nameId}
								value={draft.name}
								onChange={(event) => setDraft((current) => ({ ...current, name: event.target.value }))}
							/>
						</label>
						<label htmlFor={scopeId} className="grid gap-1 text-xs text-baud-muted">
							Scope
							<Select
								value={draft.scope}
								onValueChange={(scope) =>
									setDraft((current) => ({ ...current, scope: scope as DefaultVariable["scope"] }))
								}
							>
								<SelectTrigger id={scopeId} className="w-full">
									<SelectValue />
								</SelectTrigger>
								<SelectContent>
									<SelectItem value="runtime">runtime</SelectItem>
									<SelectItem value="persistent">persistent</SelectItem>
								</SelectContent>
							</Select>
						</label>
					</div>
					<label htmlFor={typeId} className="grid gap-1 text-xs text-baud-muted">
						Type
						<Select value={draft.type} onValueChange={(type) => changeType(type as VariableType)}>
							<SelectTrigger id={typeId} className="w-full">
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
					<label htmlFor={valueId} className="grid gap-1 text-xs text-baud-muted">
						Default value
						<VariableCodeInput
							id={valueId}
							ariaLabel="Default value"
							className="min-h-28 [&_pre]:min-h-28 [&_textarea]:min-h-28"
							hasError={Boolean(valueError)}
							multiline
							placeholder="Enter a required default value"
							value={rawValue}
							variables={defaultValueCompletions}
							onChange={setRawValue}
						/>
					</label>
					<label htmlFor={descriptionId} className="grid gap-1 text-xs text-baud-muted">
						Description
						<Input
							id={descriptionId}
							value={draft.description}
							onChange={(event) => setDraft((current) => ({ ...current, description: event.target.value }))}
						/>
					</label>
					{(declarationError || valueError) && (
						<div className="text-xs text-baud-danger">{declarationError || valueError}</div>
					)}
					<DialogFooter>
						<Button type="button" variant="outline" onClick={() => setDialogOpen(false)}>
							Cancel
						</Button>
						<Button type="button" variant="primary" disabled={Boolean(declarationError || valueError)} onClick={save}>
							Save
						</Button>
					</DialogFooter>
				</DialogContent>
			</Dialog>
		</section>
	);
}
