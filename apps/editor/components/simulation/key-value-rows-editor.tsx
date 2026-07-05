import { Plus, X } from "lucide-react";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { createHeaderRow, type HeaderRow } from "@/data/nodes/definitions/rows";

type KeyValueRowsEditorProps = {
	addLabel: string;
	emptyText: string;
	namePlaceholder: string;
	rows: HeaderRow[];
	title: string;
	valuePlaceholder: string;
	onChange: (rows: HeaderRow[]) => void;
};

export function KeyValueRowsEditor({
	addLabel,
	emptyText,
	namePlaceholder,
	rows,
	title,
	valuePlaceholder,
	onChange,
}: KeyValueRowsEditorProps) {
	return (
		<div className="space-y-2">
			<div className="flex items-center justify-between">
				<span className="font-mono text-xs uppercase tracking-[0.14em] text-baud-muted">{title}</span>
				<Button type="button" onClick={() => onChange([...rows, createHeaderRow()])} size="xs" variant="ghost">
					<Plus size={12} />
					{addLabel}
				</Button>
			</div>
			{rows.length === 0 ? (
				<div className="rounded border border-baud-border bg-baud-soft px-3 py-2 text-xs text-baud-muted">
					{emptyText}
				</div>
			) : (
				<div className="space-y-2">
					{rows.map((row) => (
						<div key={row.id} className="grid grid-cols-[minmax(0,1fr)_minmax(0,1fr)_24px] gap-2">
							<Input
								value={row.name}
								onChange={(event) => updateKeyValueRow(rows, row.id, { name: event.target.value }, onChange)}
								placeholder={namePlaceholder}
								className="min-w-0 bg-baud-soft px-2"
							/>
							<Input
								value={row.value}
								onChange={(event) => updateKeyValueRow(rows, row.id, { value: event.target.value }, onChange)}
								placeholder={valuePlaceholder}
								className="min-w-0 bg-baud-soft px-2"
							/>
							<Button
								type="button"
								onClick={() => onChange(rows.filter((currentRow) => currentRow.id !== row.id))}
								aria-label={`Remove ${title.toLowerCase()} row`}
								size="icon"
								variant="destructive"
							>
								<X size={13} />
							</Button>
						</div>
					))}
				</div>
			)}
		</div>
	);
}

function updateKeyValueRow(
	rows: HeaderRow[],
	id: string,
	patch: Partial<HeaderRow>,
	onChange: (rows: HeaderRow[]) => void,
) {
	onChange(rows.map((row) => (row.id === id ? { ...row, ...patch } : row)));
}
