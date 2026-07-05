import { ChevronDown } from "lucide-react";
import { standardKeyboardKeyReference } from "@/data/editor/key-reference";

export function KeyReferencePanel() {
	return (
		<section>
			<h3 className="mb-2 text-xs font-bold uppercase tracking-[0.18em] text-baud-muted">Keys</h3>
			<details className="group rounded border border-baud-border bg-baud-soft px-3 py-2 text-xs text-baud-muted">
				<summary className="flex cursor-pointer list-none items-center justify-between gap-2 font-semibold text-baud-text">
					Key reference
					<ChevronDown size={14} className="transition-transform group-open:rotate-180" />
				</summary>
				<div className="mt-2 space-y-2 leading-4">
					<p>
						Press a key or shortcut while the Key field is focused to record it, or type the key expression manually.
						Use <span className="font-mono text-baud-text">+</span> between parts.
					</p>
					<p>
						Examples: <span className="font-mono text-baud-text">Ctrl+Alt+B</span>,{" "}
						<span className="font-mono text-baud-text">Shift+Enter</span>,{" "}
						<span className="font-mono text-baud-text">Meta+Space</span>,{" "}
						<span className="font-mono text-baud-text">AudioVolumeUp</span>.
					</p>
					<p>
						Use runner key names for keys that are platform-specific or not produced by the browser, such as{" "}
						<span className="font-mono text-baud-text">BrowserBack</span>,{" "}
						<span className="font-mono text-baud-text">NumpadEnter</span>, or{" "}
						<span className="font-mono text-baud-text">MediaPlayPause</span>.
					</p>
					<div className="max-h-72 space-y-3 overflow-y-auto rounded border border-baud-border bg-baud-panel p-2">
						{standardKeyboardKeyReference.map((group) => (
							<div key={group.label}>
								<div className="mb-1 font-semibold uppercase tracking-[0.12em] text-baud-muted">{group.label}</div>
								<div className="flex flex-wrap gap-1">
									{group.keys.map((key) => (
										<span
											key={`${group.label}-${key}`}
											className="rounded border border-baud-border bg-baud-soft px-1.5 py-0.5 font-mono text-sm text-baud-text"
										>
											{key}
										</span>
									))}
								</div>
							</div>
						))}
					</div>
				</div>
			</details>
		</section>
	);
}
