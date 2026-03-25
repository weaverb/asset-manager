export function DigitsOnlyInput(props: {
  id?: string;
  className?: string;
  "aria-label"?: string;
  value: string;
  onChange: (digits: string) => void;
}) {
  return (
    <input
      id={props.id}
      className={["digits-only-input", props.className]
        .filter(Boolean)
        .join(" ")}
      type="text"
      inputMode="numeric"
      autoComplete="off"
      spellCheck={false}
      aria-label={props["aria-label"]}
      value={props.value}
      onChange={(e) => {
        const digits = e.target.value.replace(/\D/g, "");
        props.onChange(digits);
      }}
    />
  );
}
