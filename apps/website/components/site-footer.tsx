import Image from "next/image";
import Link from "next/link";

import { Separator } from "@/components/ui/separator";

const FOOTER_COLUMNS = [
	{
		heading: "PRODUCT",
		links: [
			{ href: "https://editor.baudbound.app", label: "Editor" },
			{ href: "https://wiki.baudbound.app/runner/installation", label: "Runner install" },
		],
	},
	{
		heading: "RESOURCES",
		links: [
			{ href: "https://wiki.baudbound.app", label: "Wiki" },
			{ href: "https://wiki.baudbound.app/security", label: "Security model" },
		],
	},
	{
		heading: "COMMUNITY",
		links: [
			{ href: "https://github.com/NATroutter/BaudBound", label: "GitHub" },
			{ href: "https://wiki.baudbound.app/licensing", label: "Licensing" },
		],
	},
];

export function SiteFooter() {
	return (
		<footer className="px-5.5 pt-11 pb-7 sm:px-16 sm:pt-18 sm:pb-11">
			<div className="grid grid-cols-1 gap-7 pb-8 sm:grid-cols-[1.4fr_1fr_1fr_1fr] sm:gap-10 sm:pb-12">
				<div>
					<div className="mb-3.5 flex items-center gap-2.5">
						<Image src="/logo-notext.svg" alt="" width={26} height={26} className="size-6.5" />
						<span className="font-heading text-[17px] font-extrabold">
							<span className="text-[#c9cdd2]">Baud</span>
							<span className="text-brand">Bound</span>
						</span>
					</div>
					<p className="max-w-[280px] text-sm leading-relaxed text-muted-foreground/70">
						A visual automation platform for Windows and Linux. Build in the browser, run locally.
					</p>
				</div>

				{FOOTER_COLUMNS.map((column) => (
					<div key={column.heading} className="flex flex-col gap-3.5">
						<div className="mb-1 font-mono text-[13px] font-semibold tracking-[0.08em] text-muted-foreground/70">
							{column.heading}
						</div>
						{column.links.map((link) => (
							<a
								key={link.href}
								href={link.href}
								target="_blank"
								rel="noopener noreferrer"
								className="text-sm text-muted-foreground transition-colors hover:text-foreground"
							>
								{link.label}
							</a>
						))}
					</div>
				))}
			</div>

			<Separator />

			<div className="flex flex-col gap-2 pt-7 text-[13px] text-muted-foreground/70 sm:flex-row sm:justify-between">
				<div className="flex flex-wrap gap-x-2">
					<span>&copy; 2026 NATroutter</span>
					<span aria-hidden>&middot;</span>
					<Link href="/privacy" className="transition-colors hover:text-foreground">
						Privacy Policy
					</Link>
				</div>
				<div>Source available &middot; Windows &amp; Linux</div>
			</div>
		</footer>
	);
}
