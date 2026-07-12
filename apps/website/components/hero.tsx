import { InstallCommand } from "@/components/install-command";

export function Hero() {
	return (
		<section className="flex flex-col items-center gap-6 px-6 pt-20 pb-20 text-center md:px-16 md:pt-28 md:pb-24">
			<div className="bb-fade-up font-mono text-[13px] font-bold tracking-[0.14em] text-brand">
				LOCAL-FIRST AUTOMATION
			</div>

			<h1 className="bb-fade-up max-w-3xl text-[42px] leading-[1.1] font-extrabold tracking-tight text-balance md:text-[62px] [animation-delay:0.08s]">
				Automate your machine, visually.
			</h1>

			<p className="bb-fade-up max-w-xl text-lg leading-relaxed text-muted-foreground md:text-[19px] [animation-delay:0.16s]">
				Build a workflow by connecting nodes in the browser editor, export it as a portable package, and run it natively
				on your own machine. No cloud. No account.
			</p>

			<div className="bb-fade-up mt-3.5 w-full max-w-[600px] [animation-delay:0.24s]">
				<InstallCommand />
			</div>

			<div className="bb-fade-up mt-1.5 flex flex-wrap justify-center gap-8 [animation-delay:0.32s]">
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
