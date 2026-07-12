import type { LucideIcon } from "lucide-react";
import { Cable, FileClock, FolderCog, Globe, MonitorCog, Webhook } from "lucide-react";
import { SectionHeading } from "@/components/section-heading";
import { Card, CardContent } from "@/components/ui/card";

const FEATURES: { icon: LucideIcon; title: string; description: string }[] = [
	{
		icon: FileClock,
		title: "Schedule & file triggers",
		description: "Kick off a workflow on a timer or the moment a watched file changes.",
	},
	{
		icon: Webhook,
		title: "Local webhooks",
		description: "Receive an HTTP request on your own machine and react to it immediately.",
	},
	{
		icon: Cable,
		title: "Serial devices",
		description: "Talk to serial hardware directly: read, write, and route the data.",
	},
	{
		icon: Globe,
		title: "HTTP requests",
		description: "Call external APIs and use the response anywhere else in the graph.",
	},
	{
		icon: FolderCog,
		title: "Files & processes",
		description: "Read, write, and move files, or launch and manage local processes.",
	},
	{
		icon: MonitorCog,
		title: "Desktop app control",
		description: "Drive supported desktop applications as part of a larger workflow.",
	},
];

export function FeatureGrid() {
	return (
		<section className="px-5.5 pb-16 sm:px-16 sm:pb-32.5">
			<SectionHeading
				eyebrow="WHAT YOU CAN AUTOMATE"
				title="Nodes for real machines, not just APIs"
				className="mb-7 text-center sm:mb-14"
			/>

			<div className="grid grid-cols-1 gap-3.5 sm:grid-cols-3 sm:gap-6">
				{FEATURES.map(({ icon: Icon, title, description }) => (
					<Card
						key={title}
						className="gap-0 border-border p-7.5 transition-all duration-250 hover:-translate-y-1.5 hover:border-brand/40 hover:shadow-[0_16px_40px_rgba(230,45,62,0.16)]"
					>
						<CardContent className="flex flex-col gap-0 p-0">
							<div className="mb-5 flex size-11 items-center justify-center rounded-[10px] bg-brand/12">
								<Icon className="size-5.5 text-brand" strokeWidth={2} />
							</div>
							<div className="mb-2.5 text-[17px] font-bold">{title}</div>
							<p className="text-sm leading-relaxed text-muted-foreground">{description}</p>
						</CardContent>
					</Card>
				))}
			</div>
		</section>
	);
}
