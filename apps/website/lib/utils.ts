import { type ClassValue, clsx } from "clsx";
import { twMerge } from "tailwind-merge";

export function cn(...inputs: ClassValue[]) {
	return twMerge(clsx(inputs));
}

export type System = "windows" | "linux" | "other";

export function detectOS(): System {
	if (typeof navigator === "undefined") {
		return "other";
	}

	const nav = navigator as Navigator & {
		userAgentData?: {
			platform?: string;
		};
	};

	const platform = (nav.userAgentData?.platform ?? navigator.platform ?? navigator.userAgent).toLowerCase();

	if (platform.includes("windows") || platform.includes("win32") || platform.includes("win64")) {
		return "windows";
	}

	if (platform.includes("linux")) {
		return "linux";
	}

	return "other";
}
