export function SectionHeading({ eyebrow, title, className }: { eyebrow: string; title: string; className?: string }) {
	return (
		<div className={className}>
			<div className="mb-3.5 font-mono text-[13px] font-bold tracking-[0.14em] text-brand">{eyebrow}</div>
			<h2 className="text-[25px] leading-tight font-extrabold text-balance sm:text-[36px] sm:leading-[1.2]">{title}</h2>
		</div>
	);
}
