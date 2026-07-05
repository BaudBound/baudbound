import { toast } from "sonner";
import type { EditorAsset, SimulationTraceEntry } from "@/lib/types";
import type { SimulationSideEffect, SimulationSideEffectResult } from "@/utils/simulation";

type AudioContextWindow = Window &
	typeof globalThis & {
		webkitAudioContext?: typeof AudioContext;
	};

export type SimulationSideEffectHandlers = {
	showMessageBox: (
		sideEffect: Extract<SimulationSideEffect, { type: "message_box" }>,
		signal: AbortSignal,
	) => Promise<string>;
};

export async function executeSimulationSideEffects(
	sideEffects: SimulationSideEffect[],
	assets: EditorAsset[],
	signal: AbortSignal,
	handlers: SimulationSideEffectHandlers,
): Promise<{ results: SimulationSideEffectResult[]; traces: SimulationTraceEntry[] }> {
	const results: SimulationSideEffectResult[] = [];
	const traces: SimulationTraceEntry[] = [];
	const assetsByPath = new Map(assets.map((asset) => [asset.packagePath.toLowerCase(), asset]));

	for (const sideEffect of sideEffects) {
		if (signal.aborted) {
			break;
		}

		if (sideEffect.type === "notification_toast") {
			showSimulationNotificationToast(sideEffect);
			continue;
		}

		if (sideEffect.type === "message_box") {
			const button = await handlers.showMessageBox(sideEffect, signal);
			if (button !== "aborted" && button !== "replaced") {
				results.push({ type: "message_box", nodeId: sideEffect.nodeId, button });
			}
			continue;
		}

		if (sideEffect.type === "system_beep") {
			try {
				await playSimulationBeep(sideEffect, signal);
			} catch (error) {
				if (signal.aborted) {
					break;
				}

				traces.push({
					level: "error",
					message: `[Simulation] Beep (${sideEffect.nodeId}) audio playback failed: ${getAudioPlaybackErrorMessage(error)}`,
				});
			}
			continue;
		}

		if (sideEffect.type !== "play_audio_asset") {
			continue;
		}

		const asset = assetsByPath.get(sideEffect.assetPath.toLowerCase());
		if (!asset || asset.kind !== "audio") {
			traces.push({
				level: "error",
				message: `[Simulation] Play Sound (${sideEffect.nodeId}) could not play missing audio asset ${sideEffect.assetPath}.`,
			});
			continue;
		}

		try {
			await playSimulationAudioAsset(asset, signal);
		} catch (error) {
			if (signal.aborted) {
				break;
			}

			traces.push({
				level: "error",
				message: `[Simulation] Play Sound (${sideEffect.nodeId}) audio playback failed: ${getAudioPlaybackErrorMessage(error)}`,
			});
		}
	}

	return { results, traces };
}

function showSimulationNotificationToast(sideEffect: Extract<SimulationSideEffect, { type: "notification_toast" }>) {
	toast.info(sideEffect.title || "Notification", {
		description: sideEffect.message,
	});
}

async function playSimulationBeep(
	sideEffect: Extract<SimulationSideEffect, { type: "system_beep" }>,
	signal: AbortSignal,
) {
	if (signal.aborted) {
		throw new DOMException("Simulation stopped before beep playback started.", "AbortError");
	}

	const AudioContextConstructor = window.AudioContext ?? (window as AudioContextWindow).webkitAudioContext;
	if (!AudioContextConstructor) {
		throw new DOMException("Browser does not support Web Audio playback.", "NotSupportedError");
	}

	const audioContext = new AudioContextConstructor();
	const oscillator = audioContext.createOscillator();
	const gain = audioContext.createGain();
	const durationSeconds = sideEffect.durationMs / 1000;

	oscillator.type = "sine";
	oscillator.connect(gain);
	gain.connect(audioContext.destination);

	let timeoutId: number | null = null;
	let cleanedUp = false;

	return new Promise<void>((resolve, reject) => {
		const cleanup = () => {
			if (cleanedUp) {
				return;
			}

			cleanedUp = true;
			if (timeoutId !== null) {
				window.clearTimeout(timeoutId);
				timeoutId = null;
			}
			signal.removeEventListener("abort", handleAbort);
			oscillator.removeEventListener("ended", handleEnded);
			oscillator.disconnect();
			gain.disconnect();
			void audioContext.close();
		};

		const finish = () => {
			cleanup();
			resolve();
		};
		const fail = (error: unknown) => {
			cleanup();
			reject(error);
		};
		const handleAbort = () => fail(new DOMException("Simulation stopped during beep playback.", "AbortError"));
		const handleEnded = () => finish();

		signal.addEventListener("abort", handleAbort, { once: true });
		oscillator.addEventListener("ended", handleEnded, { once: true });
		timeoutId = window.setTimeout(finish, sideEffect.durationMs + 150);

		audioContext
			.resume()
			.then(() => {
				if (signal.aborted) {
					handleAbort();
					return;
				}

				const now = audioContext.currentTime;
				oscillator.frequency.setValueAtTime(sideEffect.frequencyHz, now);
				gain.gain.setValueAtTime(0.0001, now);
				gain.gain.exponentialRampToValueAtTime(0.18, now + 0.01);
				gain.gain.exponentialRampToValueAtTime(0.0001, now + Math.max(0.02, durationSeconds));
				oscillator.start(now);
				oscillator.stop(now + durationSeconds);
			})
			.catch(fail);
	});
}

async function playSimulationAudioAsset(asset: EditorAsset, signal: AbortSignal) {
	if (signal.aborted) {
		throw new DOMException("Simulation stopped before audio playback started.", "AbortError");
	}

	const audioUrl = URL.createObjectURL(asset.file);
	const audio = new Audio(audioUrl);
	audio.preload = "auto";

	let cleanupDone = false;
	let cleanupTimeoutId: number | null = null;
	const cleanup = () => {
		if (cleanupDone) {
			return;
		}

		cleanupDone = true;
		if (cleanupTimeoutId !== null) {
			window.clearTimeout(cleanupTimeoutId);
			cleanupTimeoutId = null;
		}

		signal.removeEventListener("abort", handleAbort);
		audio.removeEventListener("ended", cleanup);
		audio.removeEventListener("error", cleanup);
		audio.pause();
		audio.removeAttribute("src");
		audio.load();
		URL.revokeObjectURL(audioUrl);
	};
	const handleAbort = () => cleanup();

	signal.addEventListener("abort", handleAbort, { once: true });
	audio.addEventListener("ended", cleanup, { once: true });
	audio.addEventListener("error", cleanup, { once: true });
	cleanupTimeoutId = window.setTimeout(cleanup, 10 * 60 * 1000);

	try {
		await audio.play();
	} catch (error) {
		cleanup();
		throw error;
	}

	if (signal.aborted) {
		cleanup();
		throw new DOMException("Simulation stopped while audio playback was starting.", "AbortError");
	}
}

function getAudioPlaybackErrorMessage(error: unknown) {
	if (error instanceof DOMException && error.name === "NotAllowedError") {
		return "Browser blocked audio playback. Try starting simulation again after interacting with the editor.";
	}

	if (error instanceof DOMException && error.name === "NotSupportedError") {
		return "Browser could not decode or play this audio file.";
	}

	return error instanceof Error ? error.message : "Browser audio playback failed.";
}
