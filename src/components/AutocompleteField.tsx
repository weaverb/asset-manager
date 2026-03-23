import { useCallback, useEffect, useRef, useState } from "react";
import type { FieldSuggestions } from "../types";

function isFieldSuggestions(
  x: string[] | FieldSuggestions,
): x is FieldSuggestions {
  return (
    typeof x === "object" &&
    x !== null &&
    "items" in x &&
    Array.isArray((x as FieldSuggestions).items)
  );
}

export type AutocompleteFieldProps = {
  label: string;
  value: string;
  onChange: (v: string) => void;
  fetchSuggestions: (
    query: string,
  ) => Promise<string[] | FieldSuggestions>;
  refetchKey?: string;
  onRemoteNotice?: (message: string | null) => void;
  debounceMs?: number;
  disabled?: boolean;
};

export function AutocompleteField({
  label,
  value,
  onChange,
  fetchSuggestions,
  refetchKey = "",
  onRemoteNotice,
  debounceMs = 450,
  disabled,
}: AutocompleteFieldProps) {
  const [listOpen, setListOpen] = useState(false);
  const [items, setItems] = useState<string[]>([]);
  const debounceRef = useRef<number | null>(null);
  /** Dropdown should only open while this input is focused (avoids lists opening on mount). */
  const inputFocusedRef = useRef(false);

  const runFetch = useCallback(
    async (q: string) => {
      try {
        const raw = await fetchSuggestions(q);
        if (isFieldSuggestions(raw)) {
          setItems(raw.items);
          if (inputFocusedRef.current) {
            setListOpen(raw.items.length > 0);
          }
          onRemoteNotice?.(raw.gunspecNotice ?? null);
        } else {
          setItems(raw);
          if (inputFocusedRef.current) {
            setListOpen(raw.length > 0);
          }
          onRemoteNotice?.(null);
        }
      } catch {
        setItems([]);
        setListOpen(false);
        onRemoteNotice?.(null);
      }
    },
    [fetchSuggestions, onRemoteNotice],
  );

  useEffect(() => {
    if (!inputFocusedRef.current) return;
    if (debounceRef.current) window.clearTimeout(debounceRef.current);
    debounceRef.current = window.setTimeout(() => {
      void runFetch(value);
    }, debounceMs);
    return () => {
      if (debounceRef.current) window.clearTimeout(debounceRef.current);
    };
  }, [value, runFetch, refetchKey, debounceMs]);

  return (
    <label className="autocomplete">
      {label}
      <div className="autocomplete-wrap">
        <input
          value={value}
          disabled={disabled}
          onChange={(e) => onChange(e.target.value)}
          onFocus={() => {
            inputFocusedRef.current = true;
            void runFetch(value);
          }}
          onBlur={() =>
            window.setTimeout(() => {
              inputFocusedRef.current = false;
              setListOpen(false);
            }, 180)
          }
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
                    onChange(s);
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
    </label>
  );
}
