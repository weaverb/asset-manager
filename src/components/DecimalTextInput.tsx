import { sanitizeDecimalInput } from "../lib/parseNumeric";

/** Plain text decimal entry (avoids native number input typing issues). */
export function DecimalTextInput(props: {
  id?: string;
  className?: string;
  "aria-label"?: string;
  value: string;
  onChange: (sanitized: string) => void;
}) {
  return (
    <input
      id={props.id}
      className={["decimal-text-input", props.className]
        .filter(Boolean)
        .join(" ")}
      type="text"
      inputMode="decimal"
      autoComplete="off"
      spellCheck={false}
      aria-label={props["aria-label"]}
      value={props.value}
      onChange={(e) => props.onChange(sanitizeDecimalInput(e.target.value))}
    />
  );
}
