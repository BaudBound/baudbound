import { Minus, Plus } from "lucide-react";
import {
  type FocusEventHandler,
  type KeyboardEvent,
  type MouseEvent,
  type PointerEvent,
  type ReactNode,
  useEffect,
  useId,
  useRef,
  useState,
} from "react";

import { cn } from "@/lib/utils";

import {
  getNumericDraftError,
  numericAriaValue,
  type NumericFieldContract,
  type NumericStepDirection,
  stepNumericDraft,
} from "./numeric-field-model";

export function NumericField({
  ariaLabel = "Numeric value",
  className,
  compact = false,
  contract,
  disabled = false,
  id,
  onBlur,
  onChange,
  onFocus,
  placeholder,
  readOnly = false,
  required = true,
  showError = true,
  step = "1",
  value,
}: {
  ariaLabel?: string;
  className?: string;
  compact?: boolean;
  contract: NumericFieldContract;
  disabled?: boolean;
  id?: string;
  onBlur?: FocusEventHandler<HTMLInputElement>;
  onChange: (value: string) => void;
  onFocus?: FocusEventHandler<HTMLInputElement>;
  placeholder?: string;
  readOnly?: boolean;
  required?: boolean;
  showError?: boolean;
  step?: string;
  value: number | string;
}) {
  const generatedId = useId();
  const inputId = id ?? `${generatedId}-input`;
  const errorId = `${inputId}-error`;
  const externalValue = valueToDraft(value);
  const [draft, setDraft] = useState(externalValue);
  const draftRef = useRef(draft);

  useEffect(() => {
    draftRef.current = externalValue;
    setDraft(externalValue);
  }, [externalValue]);

  const error = getNumericDraftError(draft, contract, required);
  const currentAriaValue = numericAriaValue(draft);
  const minimum = finiteNumber(contract.minimum);
  const maximum = finiteNumber(contract.maximum);

  const updateDraft = (next: string) => {
    draftRef.current = next;
    setDraft(next);
    onChange(next);
  };

  const applyStep = (direction: NumericStepDirection, multiplier = 1) => {
    const next = stepNumericDraft(draftRef.current, contract, direction, step, multiplier);
    if (next !== null) {
      updateDraft(next);
    }
  };

  const handleKeyDown = (event: KeyboardEvent<HTMLInputElement>) => {
    if (event.key === "Enter") {
      event.preventDefault();
      event.currentTarget.blur();
      return;
    }
    if (event.key === "Escape") {
      event.preventDefault();
      updateDraft(externalValue);
      event.currentTarget.blur();
      return;
    }
    if (event.key !== "ArrowDown" && event.key !== "ArrowUp") {
      return;
    }
    event.preventDefault();
    applyStep(event.key === "ArrowUp" ? 1 : -1, event.shiftKey ? 10 : 1);
  };

  const decreasePress = useRepeatingPress(() => applyStep(-1));
  const increasePress = useRepeatingPress(() => applyStep(1));
  const canDecrease = stepNumericDraft(draft, contract, -1, step) !== null;
  const canIncrease = stepNumericDraft(draft, contract, 1, step) !== null;

  return (
    <div className={cn("grid gap-1", className)}>
      <div
        className={cn(
          "grid h-9 min-w-0 overflow-hidden rounded-md border bg-[#080b12] transition-[border-color,box-shadow]",
          compact
            ? "grid-cols-[minmax(2.5rem,1fr)_1.375rem_1.375rem]"
            : "grid-cols-[minmax(0,1fr)_1.5rem_1.5rem]",
          error
            ? "border-destructive ring-2 ring-destructive/20"
            : "border-border focus-within:border-ring focus-within:ring-2 focus-within:ring-ring/25",
          disabled && "opacity-60",
        )}
      >
        <input
          aria-describedby={error && showError ? errorId : undefined}
          aria-invalid={!!error || undefined}
          aria-label={ariaLabel}
          aria-valuemax={maximum}
          aria-valuemin={minimum}
          aria-valuenow={currentAriaValue}
          aria-valuetext={draft || undefined}
          className={cn(
            "h-9 min-w-0 border-0 bg-transparent px-3 text-left font-mono text-sm tabular-nums text-foreground outline-none placeholder:text-muted-foreground disabled:cursor-not-allowed disabled:opacity-60",
            compact && "px-2 text-xs",
          )}
          disabled={disabled}
          id={inputId}
          inputMode={contract.kind === "integer" ? "numeric" : "decimal"}
          onBlur={onBlur}
          onChange={(event) => updateDraft(event.target.value)}
          onFocus={onFocus}
          onKeyDown={handleKeyDown}
          placeholder={placeholder}
          readOnly={readOnly}
          role="spinbutton"
          type="text"
          value={draft}
        />
        <StepButton
          ariaLabel={`Decrease ${ariaLabel}`}
          disabled={disabled || readOnly || !canDecrease}
          pressHandlers={decreasePress}
        >
          <Minus />
        </StepButton>
        <StepButton
          ariaLabel={`Increase ${ariaLabel}`}
          disabled={disabled || readOnly || !canIncrease}
          pressHandlers={increasePress}
        >
          <Plus />
        </StepButton>
      </div>
      {error && showError && (
        <p className="text-xs leading-4 text-destructive" id={errorId}>
          {error}
        </p>
      )}
    </div>
  );
}

function StepButton({
  ariaLabel,
  children,
  disabled,
  pressHandlers,
}: {
  ariaLabel: string;
  children: ReactNode;
  disabled: boolean;
  pressHandlers: ReturnType<typeof useRepeatingPress>;
}) {
  return (
    <button
      aria-label={ariaLabel}
      className="grid h-full place-items-center border-l border-border bg-secondary text-muted-foreground outline-none transition-colors hover:bg-muted hover:text-foreground focus-visible:bg-muted focus-visible:text-foreground disabled:cursor-not-allowed disabled:opacity-35 [&_svg]:size-3"
      disabled={disabled}
      title={ariaLabel}
      type="button"
      {...pressHandlers}
    >
      {children}
    </button>
  );
}

function useRepeatingPress(action: () => void) {
  const actionRef = useRef(action);
  const delayRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  const intervalRef = useRef<ReturnType<typeof setInterval> | null>(null);
  actionRef.current = action;

  const stop = () => {
    if (delayRef.current) {
      clearTimeout(delayRef.current);
      delayRef.current = null;
    }
    if (intervalRef.current) {
      clearInterval(intervalRef.current);
      intervalRef.current = null;
    }
  };

  useEffect(() => stop, []);

  return {
    onClick: (event: MouseEvent<HTMLButtonElement>) => {
      if (event.detail === 0) {
        actionRef.current();
      }
    },
    onPointerCancel: stop,
    onPointerDown: (event: PointerEvent<HTMLButtonElement>) => {
      if (event.button !== 0) {
        return;
      }
      event.preventDefault();
      actionRef.current();
      delayRef.current = setTimeout(() => {
        intervalRef.current = setInterval(() => actionRef.current(), 75);
      }, 400);
    },
    onPointerLeave: stop,
    onPointerUp: stop,
  };
}

function valueToDraft(value: number | string) {
  return typeof value === "number" && Number.isFinite(value)
    ? String(value)
    : typeof value === "string"
      ? value
      : "";
}

function finiteNumber(value: string) {
  const parsed = Number(value);
  return Number.isFinite(parsed) ? parsed : undefined;
}
