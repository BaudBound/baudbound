import { ExternalLink } from "lucide-react";
import { toast } from "sonner";

import { Button } from "@/components/ui/button";
import { openExternalUrl } from "@/lib/external-url";
import { LATEST_RELEASE_URL } from "@/lib/update-policy";

export function LatestReleaseButton() {
  return (
    <Button
      onClick={() => {
        void openExternalUrl(LATEST_RELEASE_URL).catch((error) =>
          toast.error(`Could not open the latest release: ${String(error)}`),
        );
      }}
      variant="outline"
    >
      <ExternalLink />
      View latest release
    </Button>
  );
}
