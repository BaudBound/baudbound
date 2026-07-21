import { ExternalLink as ExternalLinkIcon } from "lucide-react";
import type { ReactNode } from "react";
import { toast } from "sonner";

import { openExternalUrl } from "@/lib/external-url";
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
  return (
    <a
      className={cn(
        "inline-flex w-fit max-w-full min-w-0 self-start justify-self-start items-center gap-1 text-baud-blue underline-offset-4 hover:underline",
        className,
      )}
      href={href}
      onClick={(event) => {
        event.preventDefault();
        void openExternalUrl(href).catch((error) =>
          toast.error(`Could not open link: ${String(error)}`),
        );
      }}
    >
      <span className="min-w-0 break-all">{children}</span>
      {showIcon ? <ExternalLinkIcon className="size-3.5 shrink-0" /> : null}
    </a>
  );
}
