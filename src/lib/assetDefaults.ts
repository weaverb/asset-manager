import type { Asset, AssetInput, AssetKind } from "../types";

export const KINDS: { value: AssetKind; label: string }[] = [
  { value: "firearm", label: "Firearm" },
  { value: "part", label: "Part" },
  { value: "accessory", label: "Accessory" },
  { value: "ammunition", label: "Ammunition" },
];

export function emptyInput(kind: AssetKind = "firearm"): AssetInput {
  return {
    kind,
    name: "",
    manufacturer: "",
    model: "",
    serialNumber: "",
    caliber: "",
    quantity: 1,
    purchaseDate: "",
    purchasePrice: null,
    notes: "",
    extraJson: "{}",
    tags: [],
  };
}

export function normalizeTagsForSave(tags: string[] | null | undefined): string[] {
  const seen = new Set<string>();
  const out: string[] = [];
  for (const t of tags ?? []) {
    const s = t.trim();
    if (!s) continue;
    const k = s.toLowerCase();
    if (seen.has(k)) continue;
    seen.add(k);
    out.push(s);
  }
  return out;
}

export function assetToInput(a: Asset): AssetInput {
  return {
    kind: a.kind,
    name: a.name,
    manufacturer: a.manufacturer ?? "",
    model: a.model ?? "",
    serialNumber: a.serialNumber ?? "",
    caliber: a.caliber ?? "",
    quantity: a.quantity,
    purchaseDate: a.purchaseDate ?? "",
    purchasePrice: a.purchasePrice,
    notes: a.notes ?? "",
    extraJson: a.extraJson || "{}",
    tags: [...(a.tags ?? [])],
  };
}
