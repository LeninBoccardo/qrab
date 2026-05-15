// Mirrors the Rust types in src-tauri/src/commands.rs and the SQLite schema
// in CLAUDE.md §7. Keep names in sync — serde renames camelCase on the way
// out, so what arrives over IPC matches the field names below.

export type QrKind =
  | "url"
  | "text"
  | "wifi"
  | "vcard"
  | "email"
  | "phone"
  | "other";

export interface ScanRow {
  id: number;
  batchId: string;
  content: string;
  kind: QrKind;
  monitorIndex: number;
  /** Unix epoch milliseconds. */
  scannedAt: number;
  opened: boolean;
  /** Unix epoch milliseconds, or null if never opened. */
  openedAt: number | null;
}

export interface ScanResult {
  rows: ScanRow[];
  /** Opaque handle echoed back to `scan_region`. */
  screenshotId: string;
}

/** Rectangle in image-native pixels of a given monitor. */
export interface RegionBounds {
  x: number;
  y: number;
  w: number;
  h: number;
  monitorIndex: number;
}

/** Per-monitor metadata for a held screenshot. */
export interface ScreenshotMonitorMeta {
  index: number;
  width: number;
  height: number;
}

/** Filter parameters for `history_query`. Absent fields are unfiltered. */
export interface HistoryFilter {
  search?: string;
  kind?: QrKind;
  openedOnly?: boolean;
  unopenedOnly?: boolean;
  /** Unix epoch ms — inclusive lower bound on scannedAt. */
  from?: number;
  /** Unix epoch ms — inclusive upper bound on scannedAt. */
  to?: number;
  limit: number;
  offset: number;
}

export interface BulkOpenFailure {
  id: number;
  error: string;
}

export interface BulkOpenResult {
  opened: number[];
  failed: BulkOpenFailure[];
  skippedNonUrl: number;
}

/** Mirror of BULK_OPEN_CONFIRM_THRESHOLD in commands.rs (CLAUDE.md §10).
 *  Above this many URLs, ConfirmOpenAll must be shown first. */
export const BULK_OPEN_CONFIRM_THRESHOLD = 3;

export type Theme = "system" | "light" | "dark";

/** Mirrors `Settings` in src-tauri/src/settings.rs (CLAUDE.md §9). */
export interface Settings {
  hotkey: string;
  autostart: boolean;
  autoCopyOnSingleResult: boolean;
  theme: Theme;
  closeAfterCopy: boolean;
  closeAfterOpen: boolean;
}
