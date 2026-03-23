export type AssetKind = "firearm" | "part" | "accessory" | "ammunition";

export interface Asset {
  id: string;
  kind: AssetKind;
  name: string;
  manufacturer: string | null;
  model: string | null;
  serialNumber: string | null;
  caliber: string | null;
  quantity: number;
  purchaseDate: string | null;
  purchasePrice: number | null;
  notes: string | null;
  extraJson: string;
  createdAt: string;
  updatedAt: string;
  tags: string[];
}

export interface AssetInput {
  kind: AssetKind;
  name: string;
  manufacturer?: string | null;
  model?: string | null;
  serialNumber?: string | null;
  caliber?: string | null;
  quantity?: number | null;
  purchaseDate?: string | null;
  purchasePrice?: number | null;
  notes?: string | null;
  extraJson?: string | null;
  /** Omit on update to leave tags unchanged (legacy); normally send full list. */
  tags?: string[] | null;
}

export interface AssetImage {
  id: string;
  assetId: string;
  filePath: string;
  caption: string | null;
  sortOrder: number;
  createdAt: string;
}

export interface ImagePayload {
  mime: string;
  dataBase64: string;
}

export interface AppSettings {
  gunspecApiKey: string;
}

/**
 * Suggestions from `suggest_manufacturers` / `suggest_models` with optional GunSpec
 * error text (e.g. rate limits).
 */
export interface FieldSuggestions {
  items: string[];
  gunspecNotice?: string | null;
}
