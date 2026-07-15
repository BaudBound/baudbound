export type GraphElementIdPrefix = "c" | "n";

let lastTimestamp = 0;
let sequence = 0;

export function createGraphElementId(prefix: GraphElementIdPrefix) {
	const timestamp = Math.max(Date.now(), lastTimestamp);
	if (timestamp === lastTimestamp) {
		sequence += 1;
	} else {
		lastTimestamp = timestamp;
		sequence = 0;
	}

	const sequenceSuffix = sequence === 0 ? "" : `-${sequence.toString(36)}`;
	return `${prefix}-${timestamp.toString(36)}${sequenceSuffix}`;
}
