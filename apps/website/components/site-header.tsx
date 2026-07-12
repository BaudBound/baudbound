import Image from "next/image";
import Link from "next/link";

import { buttonVariants } from "@/components/ui/button";

const NAV_LINKS = [
	{ href: "https://wiki.baudbound.app", label: "Wiki" },
	{ href: "https://editor.baudbound.app", label: "Editor" },
	{ href: "https://github.com/NATroutter/BaudBound", label: "GitHub" },
];

function Wordmark() {
	return (
		<Link href="/" className="flex items-center gap-3">
			<Image src="/logo-notext.svg" alt="" width={32} height={32} className="size-8" />
			<span className="font-heading text-[17px] font-extrabold tracking-tight sm:text-xl">
				<span className="text-[#c9cdd2]">Baud</span>
				<span className="text-brand">Bound</span>
			</span>
		</Link>
	);
}

export function SiteHeader() {
	return (
		<header className="flex h-16 items-center justify-between border-b border-border px-5 sm:h-21 sm:px-16">
			<Wordmark />

			<nav className="hidden items-center gap-10 sm:flex">
				{NAV_LINKS.map((link) => (
					<a
						key={link.href}
						href={link.href}
						target="_blank"
						rel="noopener noreferrer"
						className="text-[15px] font-medium text-muted-foreground transition-colors hover:text-foreground"
					>
						{link.label}
					</a>
				))}
			</nav>

			<a
				href="https://editor.baudbound.app"
				target="_blank"
				rel="noopener noreferrer"
				className={buttonVariants({
					className: "h-auto px-4 py-2.25 text-[13px] font-semibold sm:px-5.5 sm:py-2.75 sm:text-sm",
				})}
			>
				Open Editor
			</a>
		</header>
	);
}
