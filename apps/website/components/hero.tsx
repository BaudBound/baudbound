import { InstallCommand } from "@/components/install-command";

export function Hero() {
	return (
		<section className="flex flex-col items-center gap-4.5 px-5.5 pt-13 pb-15 text-center sm:gap-6.5 sm:px-16 sm:pt-27.5 sm:pb-24">
			<div className="bb-fade-up font-mono text-[13px] font-bold tracking-[0.14em] text-brand">
				LOCAL-FIRST AUTOMATION
			</div>

			<h1 className="bb-fade-up max-w-full text-[32px] leading-[1.2] font-extrabold tracking-tight text-balance sm:max-w-205 sm:text-[62px] sm:leading-[1.1] [animation-delay:0.08s]">
				Automate your machine, visually.
			</h1>

			<p className="bb-fade-up max-w-full text-[15.5px] leading-relaxed text-muted-foreground sm:max-w-150 sm:text-[19px] [animation-delay:0.16s]">
				Build a workflow by connecting nodes in the browser editor, export it as a portable package, and run it natively
				on your own machine. No cloud. No account.
			</p>

			<div className="bb-fade-up mt-2 w-full max-w-150 sm:mt-3.5 [animation-delay:0.24s]">
				<InstallCommand />
			</div>

			<div className="bb-fade-up mt-1 flex w-full flex-col items-center gap-3.5 sm:mt-1.5 sm:w-auto sm:flex-row sm:flex-wrap sm:justify-center sm:gap-8 [animation-delay:0.32s]">
				<a
					href="https://editor.baudbound.app"
					target="_blank"
					rel="noopener noreferrer"
					className="text-[15px] font-semibold text-foreground transition-colors hover:text-brand"
				>
					Open the Editor →
				</a>
				<a
					href="https://wiki.baudbound.app"
					target="_blank"
					rel="noopener noreferrer"
					className="text-[15px] font-semibold text-muted-foreground transition-colors hover:text-foreground"
				>
					Read the Wiki →
				</a>
			</div>
		</section>
	);
}
