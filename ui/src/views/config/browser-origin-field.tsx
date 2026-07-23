import { X } from "lucide-react";
import {
  type ClipboardEvent,
  type KeyboardEvent,
  useId,
  useState,
} from "react";

import { Badge } from "@/components/ui/badge";
import {
  BROWSER_ORIGIN_MAX_COUNT,
  BROWSER_ORIGIN_MAX_LENGTH,
} from "@/lib/input-limits";

export function BrowserOriginField({
  onChange,
  value,
}: {
  onChange: (value: string[]) => void;
  value: string[];
}) {
  const inputId = useId();
  const [draft, setDraft] = useState("");
  const [error, setError] = useState<string | null>(null);

  const commitDraft = () => {
    const result = appendBrowserOrigins(value, draft);
    if (result.error) {
      setError(result.error);
      return false;
    }

    if (result.origins !== value) {
      onChange(result.origins);
    }
    setDraft("");
    setError(null);
    return true;
  };

  const handleKeyDown = (event: KeyboardEvent<HTMLInputElement>) => {
    if (event.key !== "Enter" && event.key !== ",") return;

    event.preventDefault();
    if (draft.trim()) commitDraft();
  };

  const handlePaste = (event: ClipboardEvent<HTMLInputElement>) => {
    const pastedValue = event.clipboardData.getData("text");
    if (parseBrowserOrigins(pastedValue).length <= 1) return;

    event.preventDefault();
    const result = appendBrowserOrigins(value, pastedValue);
    if (result.error) {
      setDraft(pastedValue);
      setError(result.error);
      return;
    }

    onChange(result.origins);
    setDraft("");
    setError(null);
  };

  return (
    <div className="grid gap-1.5 text-sm">
      <label className="text-xs text-muted-foreground" htmlFor={inputId}>
        Allowed browser origins
      </label>
      <div
        className="flex min-h-9 flex-wrap items-center gap-1 rounded-md border border-border bg-[#080b12] px-2 py-1 transition-colors focus-within:border-ring focus-within:ring-2 focus-within:ring-ring/25"
      >
        {value.map((origin) => (
          <Badge className="h-6 max-w-full gap-1 px-2" key={origin} variant="muted">
            <span className="min-w-0 truncate font-mono" title={origin}>
              {origin}
            </span>
            <button
              aria-label={`Remove ${origin}`}
              className="-mr-1 grid size-4 shrink-0 place-items-center rounded-sm text-muted-foreground transition-colors hover:bg-muted hover:text-foreground focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring"
              onClick={() => onChange(value.filter((current) => current !== origin))}
              type="button"
            >
              <X aria-hidden="true" size={12} />
            </button>
          </Badge>
        ))}
        <input
          aria-describedby={`${inputId}-help${error ? ` ${inputId}-error` : ""}`}
          aria-invalid={Boolean(error)}
          className="min-w-48 flex-1 bg-transparent px-1 py-1 text-sm text-foreground outline-none placeholder:text-muted-foreground"
          id={inputId}
          maxLength={BROWSER_ORIGIN_MAX_LENGTH}
          onBlur={() => {
            if (draft.trim()) commitDraft();
          }}
          onChange={(event) => {
            setDraft(event.target.value);
            if (error) setError(null);
          }}
          onKeyDown={handleKeyDown}
          onPaste={handlePaste}
          placeholder={value.length === 0 ? "https://dashboard.example.com" : "Add another origin"}
          value={draft}
        />
      </div>
      {error ? (
        <span className="text-xs text-destructive" id={`${inputId}-error`}>
          {error}
        </span>
      ) : null}
      <span className="text-xs text-muted-foreground" id={`${inputId}-help`}>
        Enter an exact HTTP or HTTPS origin, then press Enter or comma. Leave empty to block browser
        clients.
      </span>
    </div>
  );
}

export function appendBrowserOrigins(current: string[], input: string) {
  if (input.length > BROWSER_ORIGIN_MAX_COUNT * (BROWSER_ORIGIN_MAX_LENGTH + 1)) {
    return {
      origins: current,
      error: "The pasted browser origin list is too large.",
    };
  }
  const additions = parseBrowserOrigins(input);
  for (const origin of additions) {
    if (!isValidBrowserOrigin(origin)) {
      return {
        origins: current,
        error: `${origin} is not an exact HTTP or HTTPS origin.`,
      };
    }
  }

  const origins = [...current];
  for (const origin of additions) {
    if (!origins.includes(origin)) origins.push(origin);
  }
  if (origins.length > BROWSER_ORIGIN_MAX_COUNT) {
    return {
      origins: current,
      error: `No more than ${BROWSER_ORIGIN_MAX_COUNT} browser origins can be configured.`,
    };
  }

  return {
    origins: origins.length === current.length ? current : origins,
    error: null,
  };
}

export function parseBrowserOrigins(value: string) {
  return value
    .split(/[\s,]+/)
    .map((origin) => origin.trim())
    .filter(Boolean);
}

export function isValidBrowserOrigin(origin: string) {
  const authority = origin.startsWith("http://")
    ? origin.slice("http://".length)
    : origin.startsWith("https://")
      ? origin.slice("https://".length)
      : null;

  return (
    authority !== null &&
    authority.length > 0 &&
    origin.length <= BROWSER_ORIGIN_MAX_LENGTH &&
    !/[/?#@\s]/.test(authority) &&
    origin === origin.trim()
  );
}
