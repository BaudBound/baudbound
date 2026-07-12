import type { Metadata } from "next";

import { SectionHeading } from "@/components/section-heading";
import { SiteFooter } from "@/components/site-footer";
import { SiteHeader } from "@/components/site-header";

export const metadata: Metadata = {
	title: "Privacy Policy · BaudBound",
	description: "What data this website collects and how it's used.",
};

export default function PrivacyPage() {
	return (
		<>
			<SiteHeader />
			<main className="px-5.5 py-13 sm:px-16 sm:py-28">
				<div className="mx-auto max-w-2xl">
					<SectionHeading eyebrow="LEGAL" title="Privacy Policy" className="mb-7 sm:mb-12" />

					<div className="flex flex-col gap-10 text-[15px] leading-relaxed text-muted-foreground">
						<p>
							This page describes what data <span className="text-foreground">baudbound.app</span> collects about
							visitors and why. It does not cover the BaudBound editor, runner, or any workflow you build and run
							yourself. Those run locally on your machine, and this website has no visibility into them.
						</p>

						<section className="flex flex-col gap-3">
							<h2 className="text-lg font-bold text-foreground">Analytics</h2>
							<p>
								This site uses{" "}
								<a
									href="https://umami.is"
									target="_blank"
									rel="noopener noreferrer"
									className="text-foreground underline underline-offset-4 hover:text-brand"
								>
									Umami
								</a>
								, a self-hosted, open-source analytics tool. The instance is operated by the site owner directly, and no
								data is sent to a third-party analytics company. By default, Umami collects aggregate, cookieless usage
								data: the page you visited, the referring site, and coarse browser/device/location information derived
								from your request. It does not use cookies and does not build a cross-site profile of you.
							</p>
						</section>

						<section className="flex flex-col gap-3">
							<h2 className="text-lg font-bold text-foreground">Session replay &amp; heatmaps</h2>
							<p>
								On a sample of visits, this site may also record a replay of on-page interactions (mouse movement,
								clicks, and scrolling) used to generate heatmaps and diagnose usability issues. Text typed into form
								fields is masked before it's ever recorded, so field contents aren't captured. Replay recordings are
								automatically deleted after 30 days.
							</p>
						</section>

						<section className="flex flex-col gap-3">
							<h2 className="text-lg font-bold text-foreground">What we don't do</h2>
							<p>
								No cookies, no advertising trackers, no cross-site tracking, and no data is sold or shared with third
								parties. Nothing collected here is linked to an account, since this site doesn't have one.
							</p>
						</section>

						<section className="flex flex-col gap-3">
							<h2 className="text-lg font-bold text-foreground">Questions</h2>
							<p>
								If you have questions about this policy or want to raise a concern, open an issue on{" "}
								<a
									href="https://github.com/NATroutter/BaudBound"
									target="_blank"
									rel="noopener noreferrer"
									className="text-foreground underline underline-offset-4 hover:text-brand"
								>
									GitHub
								</a>
								.
							</p>
						</section>
					</div>
				</div>
			</main>
			<SiteFooter />
		</>
	);
}
