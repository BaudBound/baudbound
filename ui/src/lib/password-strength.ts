export const STORAGE_PASSWORD_MIN_CHARACTERS = 8;

export type PasswordStrength = {
  label: "Too short" | "Weak" | "Fair" | "Good" | "Strong";
  score: 0 | 1 | 2 | 3 | 4;
};

const COMMON_PATTERNS = [
  "12345678",
  "abcdefgh",
  "baudbound",
  "letmein",
  "password",
  "qwerty",
  "welcome",
];

export function passwordCharacterCount(password: string) {
  return Array.from(password).length;
}

export function evaluatePasswordStrength(password: string): PasswordStrength {
  const characters = Array.from(password);
  const length = characters.length;
  if (length < STORAGE_PASSWORD_MIN_CHARACTERS) {
    return { label: "Too short", score: 0 };
  }

  const categories = [
    /[a-z]/u.test(password),
    /[A-Z]/u.test(password),
    /\d/u.test(password),
    /[^\p{L}\p{N}\s]/u.test(password),
    /[^\u0000-\u007f]/u.test(password),
  ].filter(Boolean).length;

  let score: PasswordStrength["score"] = 1;
  if (length >= 10 && categories >= 2) score = 2;
  if (length >= 12 && categories >= 3) score = 3;
  if ((length >= 16 && categories >= 3) || (length >= 12 && categories >= 4)) {
    score = 4;
  }

  const normalized = password.toLocaleLowerCase();
  const repeatedCharacter = characters.every(
    (character) => character === characters[0],
  );
  if (
    repeatedCharacter ||
    COMMON_PATTERNS.some((pattern) => normalized.includes(pattern))
  ) {
    score = 1;
  }

  const labels = ["Too short", "Weak", "Fair", "Good", "Strong"] as const;
  return { label: labels[score], score };
}
