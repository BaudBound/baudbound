import { ExternalLink as ExternalLinkIcon } from "lucide-react";
import type { ReactNode } from "react";

import { useSystemLog } from "@/components/system-log-provider";
import { openExternalUrl, tryNormalizeExternalUrl } from "@/lib/external-url";
import { cn } from "@/lib/utils";

export function ExternalLink({
  children,
  className,
  href,
  showIcon = true,
}: {
  children: ReactNode;
  className?: string;
  href: string;
  showIcon?: boolean;
}) {
  const { notify } = useSystemLog();
  const normalizedHref = tryNormalizeExternalUrl(href);
  if (!normalizedHref) {
    return (
      <span
        className={cn(
          "inline-flex w-fit max-w-full min-w-0 self-start justify-self-start text-muted-foreground",
          className,
        )}
      >
        <span className="min-w-0 break-all">{children}</span>
      </span>
    );
  }

  return (
    <a
      className={cn(
        "inline-flex w-fit max-w-full min-w-0 self-start justify-self-start items-center gap-1 text-baud-blue underline-offset-4 hover:underline",
        className,
      )}
      href={normalizedHref}
      onClick={(event) => {
        event.preventDefault();
        void openExternalUrl(normalizedHref).catch((error) => {
          notify.error("The external link could not be opened.", {
            details: [{ label: "URL", value: normalizedHref }],
            error,
            source: "External links",
            title: "Could not open link",
          });
        });
      }}
    >
      <span className="min-w-0 break-all">{children}</span>
      {showIcon ? <ExternalLinkIcon className="size-3.5 shrink-0" /> : null}
    </a>
  );
}
