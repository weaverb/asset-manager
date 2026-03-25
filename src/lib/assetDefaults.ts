import type {
  AccessorySubtype,
  AmmunitionSubtype,
  Asset,
  AssetInput,
  AssetKind,
  FirearmSubtype,
} from "../types";

export const KINDS: { value: AssetKind; label: string }[] = [
  { value: "firearm", label: "Firearm" },
  { value: "part", label: "Part" },
  { value: "accessory", label: "Accessory" },
  { value: "ammunition", label: "Ammunition" },
];

export const FIREARM_SUBTYPES: { value: FirearmSubtype; label: string }[] = [
  { value: "pistol", label: "Pistol" },
  { value: "semi_auto", label: "Semi-auto rifle" },
  { value: "bolt_action", label: "Bolt-action rifle" },
  { value: "revolver", label: "Revolver" },
  { value: "shotgun", label: "Shotgun" },
  { value: "pcc_sub", label: "PCC / Sub" },
  { value: "other", label: "Other" },
];

export const ACCESSORY_SUBTYPES: { value: AccessorySubtype; label: string }[] = [
  { value: "scope", label: "Scope" },
  { value: "reddot", label: "Red dot" },
  { value: "holographic", label: "Holographic" },
  { value: "light", label: "Light" },
  { value: "other", label: "Other" },
];

export const AMMUNITION_SUBTYPES: { value: AmmunitionSubtype; label: string }[] = [
  { value: "pistol", label: "Pistol" },
  { value: "rifle", label: "Rifle" },
  { value: "shotgun", label: "Shotgun" },
  { value: "other", label: "Other" },
];

export function defaultSubtypeForKind(kind: AssetKind): string | null {
  if (kind === "firearm" || kind === "accessory") return "other";
  if (kind === "ammunition") return "rifle";
  return null;
}

/** Keep subtype when switching kind only if it is valid for the new kind. */
export function coerceSubtypeForKind(
  kind: AssetKind,
  previous: string | null | undefined,
): string | null {
  if (kind === "part") return null;
  const p = (previous ?? "").toLowerCase().trim();
  if (kind === "firearm") {
    const q = p === "rifle" ? "bolt_action" : p;
    if (FIREARM_SUBTYPES.some((x) => x.value === q)) {
      return q;
    }
    return "other";
  }
  if (kind === "accessory") {
    if (ACCESSORY_SUBTYPES.some((x) => x.value === p)) {
      return p;
    }
    return "other";
  }
  if (kind === "ammunition") {
    if (AMMUNITION_SUBTYPES.some((x) => x.value === p)) {
      return p;
    }
    return "rifle";
  }
  return null;
}

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
    maintenanceEveryNRounds: null,
    maintenanceEveryNDays: null,
    subtype: defaultSubtypeForKind(kind),
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
    maintenanceEveryNRounds: a.maintenanceEveryNRounds ?? null,
    maintenanceEveryNDays: a.maintenanceEveryNDays ?? null,
    subtype: a.subtype ?? null,
    tags: [...(a.tags ?? [])],
  };
}
