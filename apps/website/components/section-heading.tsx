export function SectionHeading({ eyebrow, title, className }: { eyebrow: string; title: string; className?: string }) {
	return (
		<div className={className}>
			<div className="mb-3.5 font-mono text-[13px] font-bold tracking-[0.14em] text-brand">{eyebrow}</div>
			<h2 className="text-3xl leading-tight font-extrabold text-balance md:text-4xl">{title}</h2>
		</div>
	);
}
