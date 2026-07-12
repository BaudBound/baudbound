import { BrowserFrame } from "@/components/browser-frame";
import { SectionHeading } from "@/components/section-heading";

export function ShowcaseSection() {
	return (
		<section className="px-5.5 pb-16 sm:px-16 sm:pb-32.5">
			<SectionHeading eyebrow="SEE IT IN ACTION" title="From canvas to machine" className="mb-7 text-center sm:mb-13" />

			<div className="flex flex-col items-center gap-5 sm:flex-row sm:flex-wrap sm:items-start sm:justify-center sm:gap-8">
				<BrowserFrame
					label="editor.baudbound.app"
					image="/editor_screenshot.png"
					alt="BaudBound editor node graph canvas"
					width={2550}
					height={1276}
				/>
				<BrowserFrame
					label="BaudBound Runner"
					variant="label"
					image="/runner_screenshot.png"
					alt="BaudBound runner package approval view"
					width={1120}
					height={760}
				/>
			</div>
		</section>
	);
}
