import { useCallback, useEffect, useId, useRef, useState } from "react";

function dedupeAppend(tags: string[], next: string): string[] {
  const t = next.trim();
  if (!t) return tags;
  const key = t.toLowerCase();
  if (tags.some((x) => x.toLowerCase() === key)) return tags;
  return [...tags, t];
}

export type TagInputProps = {
  label: string;
  tags: string[];
  onChange: (tags: string[]) => void;
  fetchSuggestions: (query: string) => Promise<string[]>;
  placeholder?: string;
  debounceMs?: number;
  /** Single-row control for the assets toolbar (matches search / type height). */
  variant?: "default" | "toolbar";
  className?: string;
};

export function TagInput({
  label,
  tags,
  onChange,
  fetchSuggestions,
  placeholder = "Type and press Enter or pick a suggestion",
  debounceMs = 350,
  variant = "default",
  className,
}: TagInputProps) {
  const labelId = useId();
  const [draft, setDraft] = useState("");
  const [listOpen, setListOpen] = useState(false);
  const [items, setItems] = useState<string[]>([]);
  const debounceRef = useRef<number | null>(null);
  const inputFocusedRef = useRef(false);

  const runFetch = useCallback(
    async (q: string) => {
      try {
        const raw = await fetchSuggestions(q);
        const lower = new Set(tags.map((x) => x.toLowerCase()));
        const filtered = raw.filter((s) => !lower.has(s.toLowerCase()));
        setItems(filtered);
        if (inputFocusedRef.current) {
          setListOpen(filtered.length > 0);
        }
      } catch {
        setItems([]);
        setListOpen(false);
      }
    },
    [fetchSuggestions, tags],
  );

  useEffect(() => {
    if (!inputFocusedRef.current) return;
    if (debounceRef.current) window.clearTimeout(debounceRef.current);
    debounceRef.current = window.setTimeout(() => {
      void runFetch(draft);
    }, debounceMs);
    return () => {
      if (debounceRef.current) window.clearTimeout(debounceRef.current);
    };
  }, [draft, runFetch, debounceMs]);

  const commitDraft = () => {
    const t = draft.trim();
    if (t) {
      const next = dedupeAppend(tags, t);
      if (next.length !== tags.length) onChange(next);
    }
    setDraft("");
    setListOpen(false);
  };

  const removeAt = (i: number) => {
    onChange(tags.filter((_, j) => j !== i));
  };

  const chips = (
    <div
      className={
        variant === "toolbar" ? "tag-chips tag-chips--toolbar" : "tag-chips"
      }
      role="list"
    >
      {tags.map((t, i) => (
        <span key={`${t}-${i}`} className="tag-chip" role="listitem">
          {t}
          <button
            type="button"
            className="tag-chip-remove"
            aria-label={`Remove tag ${t}`}
            onClick={() => removeAt(i)}
          >
            ×
          </button>
        </span>
      ))}
    </div>
  );

  const inputWrap = (
    <div className="autocomplete-wrap tag-input-wrap">
      <input
        value={draft}
        placeholder={placeholder}
        aria-labelledby={variant === "default" ? labelId : undefined}
        onChange={(e) => setDraft(e.target.value)}
        onFocus={() => {
          inputFocusedRef.current = true;
          void runFetch(draft);
        }}
        onBlur={() =>
          window.setTimeout(() => {
            inputFocusedRef.current = false;
            setListOpen(false);
          }, 180)
        }
        onKeyDown={(e) => {
          if (e.key === "Enter") {
            e.preventDefault();
            commitDraft();
          }
        }}
        autoComplete="off"
        spellCheck={false}
      />
      {listOpen && items.length > 0 ? (
        <ul className="autocomplete-list" role="listbox">
          {items.map((s) => (
            <li key={s}>
              <button
                type="button"
                className="autocomplete-item"
                onMouseDown={(e) => e.preventDefault()}
                onClick={() => {
                  onChange(dedupeAppend(tags, s));
                  setDraft("");
                  setListOpen(false);
                }}
              >
                {s}
              </button>
            </li>
          ))}
        </ul>
      ) : null}
    </div>
  );

  const rootClass =
    variant === "toolbar"
      ? ["tag-input", "tag-input--toolbar", className].filter(Boolean).join(" ")
      : ["tag-input-label", className].filter(Boolean).join(" ");

  return (
    <div
      className={rootClass}
      role="group"
      aria-label={variant === "toolbar" ? label : undefined}
      aria-labelledby={variant === "default" ? labelId : undefined}
    >
      {variant === "default" ? (
        <span className="tag-input-label-text" id={labelId}>
          {label}
        </span>
      ) : null}
      {variant === "toolbar" ? (
        <div className="tag-input-toolbar-inner">
          {chips}
          {inputWrap}
        </div>
      ) : (
        <>
          {chips}
          {inputWrap}
        </>
      )}
    </div>
  );
}
