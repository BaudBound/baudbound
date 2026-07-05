import type { RuntimeDataOutput } from "@/lib/types";
import { withFailureErrorOutput } from "../node-definition";

export function fileTransferRuntimeOutputs(verb: string): RuntimeDataOutput[] {
	return [
		{
			name: "source_path",
			type: "file_path",
			description: `Source file path ${verb} by the runner.`,
			example: "n-mr3zyt6f-21.source_path",
		},
		{
			name: "destination_path",
			type: "file_path",
			description: `Destination file path ${verb === "moved" ? "after the move" : "created by the runner"}.`,
			example: "n-mr3zyt6f-21.destination_path",
		},
	];
}

export function processStatusRuntimeOutputs(): RuntimeDataOutput[] {
	return [
		{
			name: "running",
			type: "boolean",
			description: "Whether the target process is currently running.",
			example: "n-mr3zyt6f-16.running",
		},
		{ name: "state", type: "string", description: "Runner-specific process state.", example: "n-mr3zyt6f-16.state" },
		{
			name: "process_id",
			type: "process_id",
			description: "Matching process identifier when found.",
			example: "n-mr3zyt6f-16.process_id",
		},
		{
			name: "process_name",
			type: "string",
			description: "Matching process name.",
			example: "n-mr3zyt6f-16.process_name",
		},
	];
}

export function fallible(outputs: RuntimeDataOutput[]) {
	return withFailureErrorOutput(outputs);
}
