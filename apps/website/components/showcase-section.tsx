import { BrowserFrame } from "@/components/browser-frame";
import { SectionHeading } from "@/components/section-heading";

export function ShowcaseSection() {
	return (
		<section className="px-6 pb-24 md:px-16 md:pb-32">
			<SectionHeading
				eyebrow="SEE IT IN ACTION"
				title="From canvas to machine"
				className="mb-12 text-center md:mb-16"
			/>

			<div className="flex flex-wrap items-start justify-center gap-8">
				<BrowserFrame
					label="editor.baudbound.app"
					image="/editor_screenshot.png"
					alt="BaudBound editor — node graph canvas"
					width={2550}
					height={1276}
				/>
				<BrowserFrame
					label="BaudBound Runner"
					variant="label"
					image="/runner_screenshot.png"
					alt="BaudBound runner — package approval view"
					width={1120}
					height={760}
				/>
			</div>
		</section>
	);
}
