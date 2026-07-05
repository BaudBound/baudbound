import { Plus } from "lucide-react";
import { useEffect, useState } from "react";
import { Button } from "@/components/ui/button";
import { OptionCombobox } from "@/components/ui/option-combobox";
import type { NodeOption } from "./simulator-panel-types";

type AddOverrideControlProps = {
	defaultNodeId: string;
	nodeOptions: NodeOption[];
	onAddOverride: (nodeId: string) => void;
};

export function AddOverrideControl({ defaultNodeId, nodeOptions, onAddOverride }: AddOverrideControlProps) {
	const [nodeId, setNodeId] = useState(defaultNodeId);

	useEffect(() => {
		setNodeId(defaultNodeId);
	}, [defaultNodeId]);

	return (
		<div className="grid grid-cols-[minmax(0,1fr)_auto] gap-2">
			<OptionCombobox ariaLabel="Node to override" value={nodeId} options={nodeOptions} onChange={setNodeId} />
			<Button type="button" onClick={() => onAddOverride(nodeId)} disabled={!nodeId} size="icon" variant="toolbar">
				<Plus size={14} />
			</Button>
		</div>
	);
}
