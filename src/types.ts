export type AssetKind = "firearm" | "part" | "accessory" | "ammunition";

/** Firearm form / icon subtype (stored as lowercase in DB). */
export type FirearmSubtype =
  | "pistol"
  | "semi_auto"
  | "bolt_action"
  | "revolver"
  | "shotgun"
  | "pcc_sub"
  | "other";

/** Accessory form / icon subtype (stored as lowercase in DB). */
export type AccessorySubtype =
  | "scope"
  | "reddot"
  | "holographic"
  | "light"
  | "other";

/** Ammunition form / icon subtype (stored as lowercase in DB). */
export type AmmunitionSubtype = "pistol" | "rifle" | "shotgun" | "other";

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
  /** Total rounds logged from completed range days (firearms). */
  lifetimeRoundsFired: number;
  /** Rounds since last maintenance record (firearms). */
  roundsFiredSinceMaintenance: number;
  /** Service every N rounds since last maintenance (firearms); null if unset. */
  maintenanceEveryNRounds: number | null;
  /** Service every N days from anchor (last maintenance, else purchase, else created); null if unset. */
  maintenanceEveryNDays: number | null;
  /** Firearm, accessory, or ammunition; null for part. */
  subtype: string | null;
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
  /** Firearms only; omit or null to clear. */
  maintenanceEveryNRounds?: number | null;
  maintenanceEveryNDays?: number | null;
  /** Firearm, accessory, or ammunition; null clears or omits. */
  subtype?: string | null;
  /** Omit on update to leave tags unchanged (legacy); normally send full list. */
  tags?: string[] | null;
}

/** Dashboard IPC payload from `get_dashboard_stats`. */
export interface DashboardAmmoCaliberRow {
  caliber: string;
  rounds: number;
}

export interface DashboardUpcomingMaintenanceRow {
  assetId: string;
  name: string;
  summary: string;
}

export interface DashboardTopFirearmRow {
  assetId: string;
  name: string;
  lifetimeRoundsFired: number;
  completedRangeDayCount: number;
}

export interface DashboardStats {
  ammoByCaliber: DashboardAmmoCaliberRow[];
  upcomingMaintenance: DashboardUpcomingMaintenanceRow[];
  topFirearms: DashboardTopFirearmRow[];
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

export interface RangeDaySummary {
  id: string;
  scheduledDate: string;
  status: string;
  notes: string | null;
  completedAt: string | null;
  createdAt: string;
  updatedAt: string;
  itemCount: number;
}

export interface RangeDayItemDetail {
  assetId: string;
  name: string;
  kind: string;
  roundsFired: number | null;
}

export interface RangeDayAmmoLink {
  firearmAssetId: string;
  firearmName: string;
  ammunitionAssetId: string;
  ammunitionName: string;
  ammunitionCaliber: string | null;
  quantityOnHand: number;
  roundsConsumed: number | null;
}

export interface RangeDayDetail {
  id: string;
  scheduledDate: string;
  status: string;
  notes: string | null;
  completedAt: string | null;
  createdAt: string;
  updatedAt: string;
  items: RangeDayItemDetail[];
  /** Present when backend supports range-day ammunition (defaults to []). */
  ammoLinks?: RangeDayAmmoLink[];
}

export interface RangeDayAmmoConsumptionEntry {
  firearmAssetId: string;
  ammunitionAssetId: string;
  rounds: number;
}

export interface RangeDayRoundEntry {
  assetId: string;
  roundsFired: number;
}

export interface AssetMaintenance {
  id: string;
  assetId: string;
  performedAt: string;
  notes: string | null;
  createdAt: string;
}
