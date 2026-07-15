import { PanelLeftClose, PanelLeftOpen, PanelRightClose, PanelRightOpen } from "lucide-react";
import { Button } from "@/components/ui/button";

type PanelCollapseButtonProps = {
	collapsed: boolean;
	label: string;
	onToggle: () => void;
	side: "left" | "right";
};

export function PanelCollapseButton({ collapsed, label, onToggle, side }: PanelCollapseButtonProps) {
	const Icon = getPanelIcon(side, collapsed);
	const action = collapsed ? "Expand" : "Collapse";
	const accessibleLabel = `${action} ${label}`;

	return (
		<Button
			type="button"
			aria-label={accessibleLabel}
			aria-expanded={!collapsed}
			title={accessibleLabel}
			onClick={onToggle}
			className="size-9 shrink-0"
			size="icon"
			variant="ghost"
		>
			<Icon size={17} />
		</Button>
	);
}

function getPanelIcon(side: "left" | "right", collapsed: boolean) {
	if (side === "left") {
		return collapsed ? PanelLeftOpen : PanelLeftClose;
	}

	return collapsed ? PanelRightOpen : PanelRightClose;
}
