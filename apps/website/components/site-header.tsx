import { Menu } from "lucide-react";
import Image from "next/image";
import Link from "next/link";

import { Button, buttonVariants } from "@/components/ui/button";
import { Sheet, SheetContent, SheetHeader, SheetTitle, SheetTrigger } from "@/components/ui/sheet";

const NAV_LINKS = [
	{ href: "https://wiki.baudbound.app", label: "Wiki" },
	{ href: "https://editor.baudbound.app", label: "Editor" },
	{ href: "https://github.com/NATroutter/BaudBound", label: "GitHub" },
];

function Wordmark() {
	return (
		<Link href="/" className="flex items-center gap-3">
			<Image src="/logo-notext.svg" alt="" width={32} height={32} className="size-8" />
			<span className="font-heading text-xl font-extrabold tracking-tight">
				<span className="text-[#c9cdd2]">Baud</span>
				<span className="text-brand">Bound</span>
			</span>
		</Link>
	);
}

export function SiteHeader() {
	return (
		<header className="flex h-[84px] items-center justify-between border-b border-border px-6 md:px-16">
			<Wordmark />

			<nav className="hidden items-center gap-10 md:flex">
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
				className={buttonVariants({ className: "hidden h-auto px-5.5 py-2.75 text-sm font-semibold md:inline-flex" })}
			>
				Open Editor
			</a>

			<Sheet>
				<SheetTrigger render={<Button variant="ghost" size="icon" className="md:hidden" aria-label="Open menu" />}>
					<Menu />
				</SheetTrigger>
				<SheetContent side="right" className="p-0">
					<SheetHeader className="border-b border-border">
						<SheetTitle className="sr-only">BaudBound navigation</SheetTitle>
						<Wordmark />
					</SheetHeader>
					<nav className="flex flex-col gap-1 p-4">
						{NAV_LINKS.map((link) => (
							<a
								key={link.href}
								href={link.href}
								target="_blank"
								rel="noopener noreferrer"
								className="rounded-lg px-3 py-2.5 text-sm font-medium text-muted-foreground transition-colors hover:bg-muted hover:text-foreground"
							>
								{link.label}
							</a>
						))}
					</nav>
					<div className="mt-auto p-4">
						<a
							href="https://editor.baudbound.app"
							target="_blank"
							rel="noopener noreferrer"
							className={buttonVariants({ className: "h-auto w-full px-5.5 py-2.75 text-sm font-semibold" })}
						>
							Open Editor
						</a>
					</div>
				</SheetContent>
			</Sheet>
		</header>
	);
}
