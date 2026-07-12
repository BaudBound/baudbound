import { headers } from "next/headers";
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
		<div className="flex min-h-5 flex-wrap items-start gap-2 px-3.5 py-3.5 sm:min-h-6 sm:flex-nowrap sm:items-center sm:gap-3 sm:px-6 sm:py-6">
			<span className="font-mono text-base font-semibold text-brand">❯</span>
			<code className="font-mono text-xs break-all text-foreground sm:text-[15px] sm:break-normal">{command}</code>
			<span className="bb-blink inline-block h-4.25 w-2 bg-brand" />
		</div>
	);
}

export async function InstallCommand() {
	const userAgent = (await headers()).get("user-agent") ?? "";

	const initialTab: "linux" | "windows" = /windows/i.test(userAgent) ? "windows" : "linux";

	return (
		<div className="relative w-full max-w-150 overflow-hidden rounded-xl border border-brand/30 bg-card text-left shadow-[0_10px_26px_rgba(230,45,62,0.14),0_10px_26px_rgba(0,0,0,0.4)] sm:rounded-[14px] sm:shadow-[0_20px_60px_rgba(230,45,62,0.14),0_20px_50px_rgba(0,0,0,0.4)]">
			<div className="absolute inset-x-0 top-0 h-0.5 bg-linear-to-r from-transparent via-brand to-transparent" />
			<Tabs defaultValue={initialTab} className="gap-0">
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
