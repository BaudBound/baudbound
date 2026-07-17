export function SerialDeviceFact({ label, value }: { label: string; value: string }) {
  return (
    <div className="min-w-0">
      <div className="text-xs text-muted-foreground">{label}</div>
      <div className="mt-0.5 break-words font-medium">{value}</div>
    </div>
  );
}
