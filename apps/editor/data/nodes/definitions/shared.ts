import { defaultInputPort, triggerOutputPort } from "../node-definition";

export const triggerPorts = () => ({ inputs: [], outputs: [triggerOutputPort] });

export const loopPorts = () => ({
	inputs: [defaultInputPort],
	outputs: [
		{ id: "done", label: "done" },
		{ id: "loop", label: "loop" },
	],
});

export const actionAudio = ["action.audio"];
export const actionFile = ["action.file"];
export const actionKeyboard = ["action.keyboard"];
export const actionMouse = ["action.mouse"];
export const actionProcess = ["action.process"];
export const actionWindow = ["action.window"];
