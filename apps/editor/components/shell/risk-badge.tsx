import { Badge } from "@/components/ui/badge";
import type { RiskLevel } from "@/lib/types";

type RiskBadgeProps = {
	risk: RiskLevel;
};

export function RiskBadge({ risk }: RiskBadgeProps) {
	return <Badge variant={risk}>{risk}</Badge>;
}
