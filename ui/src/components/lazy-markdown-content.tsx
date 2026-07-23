import { lazy, Suspense } from "react";

const MarkdownContent = lazy(() =>
  import("@/components/markdown-content").then((module) => ({
    default: module.MarkdownContent,
  })),
);

export function LazyMarkdownContent({ source }: { source: string }) {
  return (
    <Suspense fallback={<p className="text-sm text-muted-foreground">Formatting release notes...</p>}>
      <MarkdownContent source={source} />
    </Suspense>
  );
}
