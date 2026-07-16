"use client";

import { ArrowLeft, MonitorUp } from "lucide-react";
import { Button } from "@/components/ui/button";

export function ProjectAccessGate({
	checking,
	error,
	onReturn,
	onTakeControl,
}: {
	checking: boolean;
	error: string | null;
	onReturn: () => void;
	onTakeControl: () => void;
}) {
	return (
		<div className="grid min-h-dvh place-items-center bg-baud-bg px-4 text-baud-text">
			<div className="max-w-md text-center">
				{checking ? (
					<p className="text-sm text-baud-muted">Checking project availability...</p>
				) : (
					<>
						<h1 className="text-xl font-semibold text-white">Project already open</h1>
						<p className="mt-2 text-sm leading-6 text-baud-muted">
							Another tab currently controls this project. Taking control closes editing in that tab and is only allowed
							after its changes are saved.
						</p>
						{error && <p className="mt-3 text-sm leading-6 text-baud-red">{error}</p>}
						<div className="mt-5 flex justify-center gap-2">
							<Button type="button" variant="toolbar" onClick={onReturn}>
								<ArrowLeft /> Projects
							</Button>
							<Button type="button" variant="primary" onClick={onTakeControl}>
								<MonitorUp /> Take control
							</Button>
						</div>
					</>
				)}
			</div>
		</div>
	);
}
