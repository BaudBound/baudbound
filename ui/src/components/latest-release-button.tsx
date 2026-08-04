import { ExternalLink } from "lucide-react";

import { useSystemLog } from "@/components/system-log-provider";
import { Button } from "@/components/ui/button";
import { openExternalUrl } from "@/lib/external-url";
import { LATEST_RELEASE_URL } from "@/lib/update-policy";

export function LatestReleaseButton() {
  const { notify } = useSystemLog();
  return (
    <Button
      onClick={() => {
        void openExternalUrl(LATEST_RELEASE_URL).catch((error) => {
          notify.error("The latest release page could not be opened.", {
            details: [{ label: "URL", value: LATEST_RELEASE_URL }],
            error,
            source: "Updates",
            title: "Could not open latest release",
          });
        });
      }}
      variant="outline"
    >
      <ExternalLink />
      View latest release
    </Button>
  );
}
