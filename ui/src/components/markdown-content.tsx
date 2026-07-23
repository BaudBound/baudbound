import type { ComponentPropsWithoutRef } from "react";
import ReactMarkdown from "react-markdown";
import remarkGfm from "remark-gfm";

import { ExternalLink } from "@/components/external-link";
import { cn } from "@/lib/utils";

export function MarkdownContent({ className, source }: { className?: string; source: string }) {
  return (
    <div className={cn("select-text text-sm leading-6 text-muted-foreground", className)}>
      <ReactMarkdown
        components={{
          a: MarkdownLink,
          blockquote: ({ children }) => (
            <blockquote className="my-3 border-l-2 border-border pl-3">{children}</blockquote>
          ),
          code: ({ children, className: codeClassName, ...props }) => (
            <code
              className={cn("rounded-sm bg-muted px-1 py-0.5 font-mono text-xs", codeClassName)}
              {...props}
            >
              {children}
            </code>
          ),
          h1: ({ children }) => <h3 className="mb-2 mt-4 text-base font-semibold text-foreground">{children}</h3>,
          h2: ({ children }) => <h3 className="mb-2 mt-4 text-base font-semibold text-foreground">{children}</h3>,
          h3: ({ children }) => <h4 className="mb-2 mt-4 text-sm font-semibold text-foreground">{children}</h4>,
          li: ({ children }) => <li className="ml-5 list-disc pl-1">{children}</li>,
          ol: ({ children }) => <ol className="my-2 grid gap-1">{children}</ol>,
          p: ({ children }) => <p className="my-2 first:mt-0 last:mb-0">{children}</p>,
          pre: ({ children }) => (
            <pre className="my-3 overflow-x-auto rounded-md border border-border bg-background p-3 font-mono text-xs text-foreground">
              {children}
            </pre>
          ),
          ul: ({ children }) => <ul className="my-2 grid gap-1">{children}</ul>,
        }}
        remarkPlugins={[remarkGfm]}
        skipHtml
      >
        {source}
      </ReactMarkdown>
    </div>
  );
}

function MarkdownLink({ children, href }: ComponentPropsWithoutRef<"a">) {
  if (!href) return <span>{children}</span>;
  return <ExternalLink href={href}>{children}</ExternalLink>;
}
