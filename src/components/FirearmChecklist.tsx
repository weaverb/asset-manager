import type { Asset } from "../types";

export type FirearmChecklistProps = {
  firearms: Asset[];
  selectedIds: string[];
  onChange: (ids: string[]) => void;
  disabled?: boolean;
};

export function FirearmChecklist({
  firearms,
  selectedIds,
  onChange,
  disabled = false,
}: FirearmChecklistProps) {
  const set = new Set(selectedIds);
  const toggle = (id: string) => {
    if (disabled) return;
    if (set.has(id)) {
      onChange(selectedIds.filter((x) => x !== id));
    } else {
      onChange([...selectedIds, id]);
    }
  };

  if (firearms.length === 0) {
    return <p className="muted">No firearms in inventory yet.</p>;
  }

  return (
    <ul className="firearm-checklist" role="list">
      {firearms.map((a) => (
        <li key={a.id}>
          <label className="firearm-check-label">
            <input
              type="checkbox"
              checked={set.has(a.id)}
              disabled={disabled}
              onChange={() => toggle(a.id)}
            />
            <span>
              {a.name}
              {a.manufacturer || a.model
                ? ` — ${[a.manufacturer, a.model].filter(Boolean).join(" ")}`
                : null}
            </span>
          </label>
        </li>
      ))}
    </ul>
  );
}
