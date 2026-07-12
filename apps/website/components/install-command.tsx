import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs";

function TrafficLights() {
	return (
		<div className="flex gap-1.5">
			<span className="size-2.5 rounded-full bg-[#3c4650]" />
			<span className="size-2.5 rounded-full bg-[#3c4650]" />
			<span className="size-2.5 rounded-full bg-[#3c4650]" />
		</div>
	);
}

function CommandRow({ command }: { command: string }) {
	return (
		<div className="flex min-h-6 items-center gap-3 px-6 py-6">
			<span className="font-mono text-base font-semibold text-brand">❯</span>
			<code className="font-mono text-[15px] font-medium text-foreground">{command}</code>
			<span className="bb-blink inline-block h-[17px] w-2 bg-brand" />
		</div>
	);
}

export function InstallCommand() {
	return (
		<div className="relative w-full max-w-150 overflow-hidden rounded-2xl border border-brand/30 bg-card text-left shadow-[0_20px_60px_rgba(230,45,62,0.14),0_20px_50px_rgba(0,0,0,0.4)]">
			<div className="absolute inset-x-0 top-0 h-0.5 bg-gradient-to-r from-transparent via-brand to-transparent" />
			<Tabs defaultValue="linux" className="gap-0">
				<div className="flex items-center justify-between border-b border-border px-4.5 py-3.5">
					<TrafficLights />
					<TabsList className="h-9! gap-1 border border-border/60 bg-background p-1">
						<TabsTrigger
							value="linux"
							className="px-4 py-2 text-[13px] font-semibold data-active:bg-brand data-active:text-white dark:data-active:border-transparent dark:data-active:bg-brand"
						>
							Linux
						</TabsTrigger>
						<TabsTrigger
							value="windows"
							className="px-4 py-2 text-[13px] font-semibold data-active:bg-brand data-active:text-white dark:data-active:border-transparent dark:data-active:bg-brand"
						>
							Windows
						</TabsTrigger>
					</TabsList>
				</div>
				<TabsContent value="linux">
					<CommandRow command="curl -fsSL https://get.baudbound.app/linux | sh" />
				</TabsContent>
				<TabsContent value="windows">
					<CommandRow command="irm https://get.baudbound.app/windows | iex" />
				</TabsContent>
			</Tabs>
		</div>
	);
}
