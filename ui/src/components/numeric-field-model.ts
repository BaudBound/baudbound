const SIGNED_INTEGER_PATTERN = /^-?(?:0|[1-9][0-9]*)$/;
const UNSIGNED_INTEGER_PATTERN = /^(?:0|[1-9][0-9]*)$/;
const SIGNED_FLOAT_PATTERN = /^-?(?:0|[1-9][0-9]*)(?:\.[0-9]+)?(?:[eE][+-]?[0-9]+)?$/;
const UNSIGNED_FLOAT_PATTERN = /^(?:0|[1-9][0-9]*)(?:\.[0-9]+)?(?:[eE][+-]?[0-9]+)?$/;

export const runtimeFloatMinimum = "-1.7976931348623157e308";
export const runtimeFloatMaximum = "1.7976931348623157e308";

export type NumericFieldContract = {
  kind: "float" | "integer";
  maximum: string;
  maximumInclusive?: boolean;
  minimum: string;
  minimumInclusive?: boolean;
  signed: boolean;
};

export type NumericStepDirection = -1 | 1;

export function getNumericDraftError(
  value: string,
  contract: NumericFieldContract,
  required: boolean,
) {
  const trimmed = value.trim();
  if (!trimmed) {
    return required ? "Enter a number." : "";
  }

  if (contract.kind === "integer") {
    const pattern = contract.signed ? SIGNED_INTEGER_PATTERN : UNSIGNED_INTEGER_PATTERN;
    if (!pattern.test(trimmed)) {
      return contract.signed ? "Enter a whole signed integer." : "Enter a whole non-negative integer.";
    }
    const parsed = BigInt(trimmed);
    const minimum = BigInt(contract.minimum);
    const maximum = BigInt(contract.maximum);
    return withinBounds(parsed, minimum, maximum, contract) ? "" : rangeMessage(contract);
  }

  const pattern = contract.signed ? SIGNED_FLOAT_PATTERN : UNSIGNED_FLOAT_PATTERN;
  if (!pattern.test(trimmed)) {
    return contract.signed ? "Enter a signed decimal number." : "Enter a non-negative decimal number.";
  }
  const parsed = Number(trimmed);
  if (!Number.isFinite(parsed)) {
    return "Enter a finite number.";
  }
  const minimum = Number(contract.minimum);
  const maximum = Number(contract.maximum);
  return withinBounds(parsed, minimum, maximum, contract) ? "" : rangeMessage(contract);
}

export function stepNumericDraft(
  value: string,
  contract: NumericFieldContract,
  direction: NumericStepDirection,
  step = "1",
  multiplier = 1,
) {
  const trimmed = value.trim();
  if (!Number.isInteger(multiplier) || multiplier < 1) {
    return null;
  }
  return contract.kind === "integer"
    ? stepIntegerDraft(trimmed, contract, direction, step, multiplier)
    : stepFloatDraft(trimmed, contract, direction, step, multiplier);
}

export function numericAriaValue(value: string) {
  const parsed = Number(value);
  return value.trim() && Number.isFinite(parsed) ? parsed : undefined;
}

function stepIntegerDraft(
  value: string,
  contract: NumericFieldContract,
  direction: NumericStepDirection,
  step: string,
  multiplier: number,
) {
  const zero = BigInt(0);
  const one = BigInt(1);
  if (!UNSIGNED_INTEGER_PATTERN.test(step) || BigInt(step) <= zero) {
    return null;
  }
  const minimum = BigInt(contract.minimum) + (contract.minimumInclusive === false ? one : zero);
  const maximum = BigInt(contract.maximum) - (contract.maximumInclusive === false ? one : zero);
  if (minimum > maximum) {
    return null;
  }
  if (!value) {
    return initialIntegerValue(minimum, maximum).toString();
  }
  if (!SIGNED_INTEGER_PATTERN.test(value)) {
    return null;
  }

  const current = BigInt(value);
  const delta = BigInt(step) * BigInt(multiplier) * BigInt(direction);
  const next = clampBigInt(current + delta, minimum, maximum);
  return next === current ? null : next.toString();
}

function initialIntegerValue(minimum: bigint, maximum: bigint) {
  const zero = BigInt(0);
  if (minimum > zero) {
    return minimum;
  }
  if (maximum < zero) {
    return maximum;
  }
  return zero;
}

function stepFloatDraft(
  value: string,
  contract: NumericFieldContract,
  direction: NumericStepDirection,
  step: string,
  multiplier: number,
) {
  const parsedStep = Number(step);
  const minimum = Number(contract.minimum);
  const maximum = Number(contract.maximum);
  if (!Number.isFinite(parsedStep) || parsedStep <= 0 || !Number.isFinite(minimum) || !Number.isFinite(maximum)) {
    return null;
  }
  if (!value) {
    const initial = initialFloatValue(contract, minimum, maximum, parsedStep);
    return initial === null ? null : formatFloat(initial, decimalPlaces(step));
  }
  const pattern = contract.signed ? SIGNED_FLOAT_PATTERN : UNSIGNED_FLOAT_PATTERN;
  if (!pattern.test(value) || !Number.isFinite(Number(value))) {
    return null;
  }

  const current = Number(value);
  const precision = Math.min(12, Math.max(decimalPlaces(value), decimalPlaces(step)));
  const delta = parsedStep * multiplier * direction;
  const stepped = addDecimal(current, delta, precision);
  if (
    (contract.minimumInclusive === false && stepped <= minimum) ||
    (contract.maximumInclusive === false && stepped >= maximum)
  ) {
    return null;
  }
  const next = clampNumber(stepped, minimum, maximum);
  if (!Number.isFinite(next) || Object.is(next, current)) {
    return null;
  }
  return formatFloat(next, precision);
}

function initialFloatValue(
  contract: NumericFieldContract,
  minimum: number,
  maximum: number,
  step: number,
) {
  let candidate = 0;
  if (candidate < minimum || (contract.minimumInclusive === false && candidate <= minimum)) {
    candidate = contract.minimumInclusive === false ? minimum + step : minimum;
  }
  if (candidate > maximum || (contract.maximumInclusive === false && candidate >= maximum)) {
    candidate = contract.maximumInclusive === false ? maximum - step : maximum;
  }
  if (!withinBounds(candidate, minimum, maximum, contract)) {
    candidate = minimum / 2 + maximum / 2;
  }
  return Number.isFinite(candidate) && withinBounds(candidate, minimum, maximum, contract)
    ? candidate
    : null;
}

function withinBounds<T extends bigint | number>(
  value: T,
  minimum: T,
  maximum: T,
  contract: NumericFieldContract,
) {
  const aboveMinimum = contract.minimumInclusive === false ? value > minimum : value >= minimum;
  const belowMaximum = contract.maximumInclusive === false ? value < maximum : value <= maximum;
  return aboveMinimum && belowMaximum;
}

function rangeMessage(contract: NumericFieldContract) {
  const minimumOperator = contract.minimumInclusive === false ? "greater than" : "at least";
  const maximumOperator = contract.maximumInclusive === false ? "less than" : "at most";
  return `Enter a value ${minimumOperator} ${contract.minimum} and ${maximumOperator} ${contract.maximum}.`;
}

function addDecimal(value: number, delta: number, precision: number) {
  const scale = 10 ** precision;
  if (Number.isSafeInteger(value * scale) && Number.isSafeInteger(delta * scale)) {
    return (Math.round(value * scale) + Math.round(delta * scale)) / scale;
  }
  return value + delta;
}

function decimalPlaces(value: string) {
  const normalized = value.toLowerCase();
  const [coefficient, exponentText] = normalized.split("e");
  const fractionLength = coefficient?.split(".")[1]?.length ?? 0;
  const exponent = exponentText ? Number(exponentText) : 0;
  return Number.isFinite(exponent) ? Math.max(0, fractionLength - exponent) : fractionLength;
}

function formatFloat(value: number, precision: number) {
  if (!Number.isFinite(value)) {
    return "";
  }
  if (precision > 0 && Math.abs(value) < 1e21) {
    return value.toFixed(precision).replace(/(?:\.0+|(\.\d+?)0+)$/, "$1");
  }
  return String(value);
}

function clampBigInt(value: bigint, minimum: bigint, maximum: bigint) {
  if (value < minimum) {
    return minimum;
  }
  if (value > maximum) {
    return maximum;
  }
  return value;
}

function clampNumber(value: number, minimum: number, maximum: number) {
  return Math.min(Math.max(value, minimum), maximum);
}
