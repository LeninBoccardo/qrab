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
  /** Opaque handle echoed back to `scan_region` (Phase 2). */
  screenshotId: string;
}
