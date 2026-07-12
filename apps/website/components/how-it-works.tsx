import { SectionHeading } from "@/components/section-heading";

const STEPS = [
	{
		number: "01",
		title: "Build",
		description: "Add triggers, actions, and control-flow nodes on the canvas in the browser editor.",
	},
	{
		number: "02",
		title: "Verify",
		description: "The editor checks graph structure, node configuration, and target compatibility.",
	},
	{
		number: "03",
		title: "Export",
		description: "Download a portable package containing the workflow, metadata, and declared assets.",
	},
	{
		number: "04",
		title: "Run",
		description: "Import it into the runner, approve the exact revision, then run it locally.",
	},
];

export function HowItWorks() {
	return (
		<section className="px-6 pb-24 md:px-16 md:pb-32">
			<SectionHeading
				eyebrow="HOW IT WORKS"
				title="Build once, run with explicit trust"
				className="mb-14 text-center md:mb-16"
			/>

			<div className="relative grid grid-cols-1 gap-10 sm:grid-cols-2 lg:grid-cols-4 lg:gap-7">
				<div className="absolute top-[19px] right-[60px] left-[60px] hidden h-px bg-border lg:block" />

				{STEPS.map((step) => (
					<div key={step.number} className="relative flex flex-col gap-4">
						<div className="flex size-9.5 items-center justify-center rounded-full border border-white/15 bg-background font-mono text-sm font-bold text-brand">
							{step.number}
						</div>
						<div className="text-lg font-bold">{step.title}</div>
						<p className="text-[15px] leading-relaxed text-muted-foreground">{step.description}</p>
					</div>
				))}
			</div>
		</section>
	);
}
