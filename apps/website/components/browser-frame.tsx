import Image from "next/image";

import { Dialog, DialogContent, DialogTitle, DialogTrigger } from "@/components/ui/dialog";
import { cn } from "@/lib/utils";

function TrafficLights() {
	return (
		<div className="flex gap-1.5">
			<span className="size-2.5 rounded-full bg-[#3c4650]" />
			<span className="size-2.5 rounded-full bg-[#3c4650]" />
			<span className="size-2.5 rounded-full bg-[#3c4650]" />
		</div>
	);
}

export function BrowserFrame({
	label,
	variant = "url",
	image,
	alt,
	width,
	height,
	className,
}: {
	label: string;
	variant?: "url" | "label";
	image: string;
	alt: string;
	width: number;
	height: number;
	className?: string;
}) {
	return (
		<div
			className={cn(
				"w-full max-w-150 overflow-hidden rounded-[10px] border border-border bg-[#1c2126] shadow-[0_24px_70px_rgba(0,0,0,0.35)]",
				className,
			)}
		>
			<div className="flex h-11 items-center gap-2.5 border-b border-white/6 bg-[#20262b] px-4">
				<TrafficLights />
				{variant === "url" ? (
					<div className="flex-1 truncate rounded-md bg-background px-3.5 py-1.5 font-mono text-xs text-muted-foreground">
						{label}
					</div>
				) : (
					<span className="truncate font-mono text-xs text-muted-foreground">{label}</span>
				)}
			</div>
			<Dialog>
				<DialogTrigger
					render={
						<button
							type="button"
							className="relative block h-90 w-full cursor-zoom-in overflow-hidden"
							aria-label={`View larger screenshot: ${alt}`}
						/>
					}
				>
					<Image
						src={image}
						alt={alt}
						fill
						quality={100}
						className="object-fill"
						sizes="(min-width: 768px) 600px, 100vw"
					/>
				</DialogTrigger>
				<DialogContent
					showCloseButton
					className="max-w-[min(1400px,92vw)] gap-0 border-0 bg-transparent p-0 shadow-2xl ring-0 sm:max-w-[min(1400px,92vw)]"
				>
					<DialogTitle className="sr-only">{alt}</DialogTitle>
					<Image
						src={image}
						alt={alt}
						width={width}
						height={height}
						quality={100}
						className="h-auto max-h-[85vh] w-full rounded-lg object-contain"
					/>
				</DialogContent>
			</Dialog>
		</div>
	);
}
