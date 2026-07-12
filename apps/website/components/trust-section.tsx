import { SectionHeading } from "@/components/section-heading";
import { cn } from "@/lib/utils";

const POINTS: { shape: "square" | "circle" | "diamond"; title: string; description: string }[] = [
	{
		shape: "square",
		title: "Local execution",
		description: "Workflows run on a runner you control. There's no hosted execution service in between.",
	},
	{
		shape: "circle",
		title: "Explicit approval",
		description: "Every package revision is reviewed and approved by hash before the runner will touch it.",
	},
	{
		shape: "diamond",
		title: "Nothing trusted by default",
		description: "A package isn't trusted just because the editor created it.",
	},
];

export function TrustSection() {
	return (
		<section className="border-y border-border bg-[#0f1113] bg-[radial-gradient(900px_380px_at_50%_0%,rgba(230,45,62,0.1),transparent_65%)] bg-no-repeat px-5.5 py-14 sm:px-16 sm:py-25">
			<SectionHeading
				eyebrow="LOCAL BY DESIGN"
				title="Nothing runs until you approve it"
				className="mb-7 text-center sm:mb-14"
			/>

			<div className="mx-auto grid max-w-5xl grid-cols-1 gap-7 sm:grid-cols-3 sm:gap-10">
				{POINTS.map(({ shape, title, description }) => (
					<div key={title}>
						<div
							className={cn(
								"mb-4.5 size-8.5 border-2 border-brand",
								shape === "circle" && "rounded-full",
								shape === "square" && "rounded-lg",
								shape === "diamond" && "size-6 rotate-45 rounded-[3px]",
							)}
						/>
						<div className="mb-2.5 text-[17px] font-bold">{title}</div>
						<p className="text-sm leading-relaxed text-muted-foreground">{description}</p>
					</div>
				))}
			</div>
		</section>
	);
}
