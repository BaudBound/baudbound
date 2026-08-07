import { describe, expect, it } from "vitest";

import {
  getNumericDraftError,
  type NumericFieldContract,
  runtimeFloatMinimum,
  runtimeIntegerMaximum,
  runtimeIntegerMinimum,
  stepNumericDraft,
} from "@/components/numeric-field-model";

const unsignedInteger: NumericFieldContract = {
  kind: "integer",
  maximum: "18446744073709551615",
  minimum: "0",
  signed: false,
};

const signedDecimal: NumericFieldContract = {
  kind: "float",
  maximum: "100",
  minimum: "-100",
  signed: true,
};

describe("numeric field model", () => {
  it("steps integers beyond JavaScript's safe-number range without losing precision", () => {
    expect(stepNumericDraft("9007199254740992", unsignedInteger, 1)).toBe(
      "9007199254740993",
    );
    expect(stepNumericDraft("18446744073709551615", unsignedInteger, 1)).toBeNull();
  });

  it("steps decimals without floating-point display artifacts", () => {
    expect(stepNumericDraft("0.2", signedDecimal, 1, "0.1")).toBe("0.3");
    expect(stepNumericDraft("0.3", signedDecimal, -1, "0.1")).toBe("0.2");
  });

  it("uses the nearest valid starting value and enforces boundaries", () => {
    const positive: NumericFieldContract = {
      kind: "integer",
      maximum: "10",
      minimum: "2",
      signed: false,
    };

    expect(stepNumericDraft("", positive, 1)).toBe("2");
    expect(stepNumericDraft("2", positive, -1)).toBeNull();
    expect(stepNumericDraft("9", positive, 1, "5")).toBe("10");
  });

  it("starts inside exclusive decimal bounds and never reverses a requested step", () => {
    const positiveDecimal: NumericFieldContract = {
      kind: "float",
      maximum: "10",
      maximumInclusive: false,
      minimum: "0",
      minimumInclusive: false,
      signed: false,
    };

    expect(stepNumericDraft("", positiveDecimal, 1, "0.5")).toBe("0.5");
    expect(stepNumericDraft("0.25", positiveDecimal, -1, "0.5")).toBeNull();
    expect(stepNumericDraft("9.75", positiveDecimal, 1, "0.5")).toBeNull();
  });

  it("validates integer, decimal, empty, and range errors", () => {
    expect(getNumericDraftError("", signedDecimal, true)).toBe("Enter a number.");
    expect(getNumericDraftError("1.", signedDecimal, true)).toContain("decimal");
    expect(getNumericDraftError("101", signedDecimal, true)).toContain("at most 100");
    expect(getNumericDraftError("42.5", signedDecimal, true)).toBe("");
    expect(getNumericDraftError("-1", unsignedInteger, true)).toContain("non-negative");
  });
});

describe("runtime contracts", () => {
  it("gives an integer contract bounds that BigInt can read", () => {
    // An integer contract is read with BigInt, which rejects exponent
    // notation. Borrowing the float bounds threw while rendering the field and
    // took the whole window blank.
    expect(() => BigInt(runtimeIntegerMinimum)).not.toThrow();
    expect(() => BigInt(runtimeIntegerMaximum)).not.toThrow();
    expect(() => BigInt(runtimeFloatMinimum)).toThrow();

    const integerContract: NumericFieldContract = {
      kind: "integer",
      maximum: runtimeIntegerMaximum,
      minimum: runtimeIntegerMinimum,
      signed: true,
    };
    expect(getNumericDraftError("42", integerContract, true)).toBe("");
    expect(getNumericDraftError("42.5", integerContract, true)).not.toBe("");
  });
});
