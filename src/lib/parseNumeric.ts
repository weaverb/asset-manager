/** Parses a non-negative integer from digit-only field text; empty → 0. */
export function parseNonNegInt(s: string | undefined): number {
  if (s === undefined || s === "") return 0;
  const n = Number.parseInt(s, 10);
  return Number.isFinite(n) && n >= 0 ? n : 0;
}

/** Digit-only maintenance interval: empty → null; zero → null. */
export function parseOptionalPositiveInt(digits: string): number | null {
  if (digits.trim() === "") return null;
  const n = parseNonNegInt(digits);
  return n > 0 ? n : null;
}

/** Keeps digits and at most one decimal point (for currency-style entry). */
export function sanitizeDecimalInput(raw: string): string {
  let s = raw.replace(/[^\d.]/g, "");
  const firstDot = s.indexOf(".");
  if (firstDot >= 0) {
    s =
      s.slice(0, firstDot + 1) + s.slice(firstDot + 1).replace(/\./g, "");
  }
  return s;
}

/** Parses optional non-negative decimal; empty or lone "." → null. */
export function parseOptionalPrice(s: string): number | null {
  const t = s.trim();
  if (t === "" || t === ".") return null;
  const n = Number.parseFloat(t);
  return Number.isFinite(n) && n >= 0 ? n : null;
}
