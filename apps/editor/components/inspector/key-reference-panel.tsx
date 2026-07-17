import { ChevronDown } from "lucide-react";
import { Button } from "@/components/ui/button";
import { windowsKeyboardKeyReference } from "@/data/editor/key-reference";

type KeyReferencePanelProps = {
	value: string;
	onChange: (value: string) => void;
};

export function KeyReferencePanel({ value, onChange }: KeyReferencePanelProps) {
	const appendKey = (key: string) => {
		const currentValue = value.trim();
		onChange(currentValue ? `${currentValue}+${key}` : key);
	};

	return (
		<section>
			<h3 className="mb-2 text-xs font-bold uppercase tracking-[0.18em] text-baud-muted">Windows keys</h3>
			<details className="group rounded border border-baud-border bg-baud-soft px-3 py-2 text-xs text-baud-muted">
				<summary className="flex cursor-pointer list-none items-center justify-between gap-2 font-semibold text-baud-text">
					Supported key reference
					<ChevronDown size={14} className="transition-transform group-open:rotate-180" />
				</summary>
				<div className="mt-2 space-y-2 leading-4">
					<p>
						Press and hold the keys together while the Key field is focused, or build a combination with the buttons
						below. Browser-reserved shortcuts may need to be added with the buttons.
					</p>
					<p>
						Examples: <span className="font-mono text-baud-text">Ctrl+Alt+B</span>,{" "}
						<span className="font-mono text-baud-text">K+L</span>,{" "}
						<span className="font-mono text-baud-text">F1+T</span>,{" "}
						<span className="font-mono text-baud-text">Windows+Space</span>,{" "}
						<span className="font-mono text-baud-text">VolumeUp</span>.
					</p>
					<p>
						Use a key button when the browser cannot capture the physical key, such as{" "}
						<span className="font-mono text-baud-text">BrowserBack</span>,{" "}
						<span className="font-mono text-baud-text">NumpadAdd</span>, or{" "}
						<span className="font-mono text-baud-text">MediaPlayPause</span>.
					</p>
					<div className="max-h-72 space-y-3 overflow-y-auto rounded border border-baud-border bg-baud-panel p-2">
						{windowsKeyboardKeyReference.map((group) => (
							<div key={group.label}>
								<div className="mb-1 font-semibold uppercase tracking-[0.12em] text-baud-muted">{group.label}</div>
								<div className="flex flex-wrap gap-1">
									{group.keys.map((key) => (
										<Button
											key={`${group.label}-${key}`}
											type="button"
											variant="outline"
											size="sm"
											aria-label={`Add ${key} to key expression`}
											className="h-7 rounded border-baud-border bg-baud-soft px-2 font-mono text-xs text-baud-text hover:border-baud-line hover:bg-baud-line"
											onClick={() => appendKey(key)}
										>
											{key}
										</Button>
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
