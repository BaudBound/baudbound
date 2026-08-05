export function Details({ rows }: { rows: Array<[string, string]> }) {
  return (
    <dl className="grid min-w-0 max-w-full grid-cols-[max-content_minmax(0,1fr)] gap-x-4 gap-y-2 text-sm">
      {rows.map(([label, value]) => (
        <div className="contents" key={label}>
          <dt className="text-muted-foreground">{label}</dt>
          <dd className="min-w-0 max-w-full break-all">{value}</dd>
        </div>
      ))}
    </dl>
  );
}
