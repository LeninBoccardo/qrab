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

/** Sentinel `monitorIndex` for rows whose source was a file decoded via
 *  decode_image_file rather than a real screen capture. The SQLite
 *  schema requires NOT NULL on `monitor_index`, so this constant beats
 *  a nullable-column migration. Display code that formats a "Monitor N"
 *  label should special-case this value as "from file". */
export const FILE_SOURCE_MONITOR_INDEX = -1;

export interface ScanRow {
  id: number;
  batchId: string;
  content: string;
  kind: QrKind;
  /** Index of the captured monitor (0-based), or
   *  [`FILE_SOURCE_MONITOR_INDEX`] (-1) for rows decoded from a file. */
  monitorIndex: number;
  /** Unix epoch milliseconds. */
  scannedAt: number;
  opened: boolean;
  /** Unix epoch milliseconds, or null if never opened. */
  openedAt: number | null;
  copied: boolean;
  /** Unix epoch milliseconds, or null if never copied. */
  copiedAt: number | null;
}

/** Mirrors `StatusFilter` in src-tauri/src/storage/queries.rs.
 *  `opened` and `copied` overlap; `untouched` is the AND-NOT case. */
export type StatusFilter = "all" | "opened" | "copied" | "untouched";

/** Sort direction for history_query. Default is "desc" (newest first). */
export type SortDir = "asc" | "desc";

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
  /** Absent or `"all"` means no status narrowing. */
  status?: StatusFilter;
  /** Unix epoch ms — inclusive lower bound on scannedAt. */
  from?: number;
  /** Unix epoch ms — inclusive upper bound on scannedAt. */
  to?: number;
  /** Absent defaults to "desc" (newest first). */
  sortDir?: SortDir;
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

/** Static app metadata from Cargo — name, version, author, description.
 *  Returned by `get_app_info`. Doesn't change at runtime. */
export interface AppInfo {
  name: string;
  version: string;
  author: string;
  description: string;
}

/** Whether the OS accepted the current hotkey binding. `registered: false`
 *  means the chord could not be bound — usually a conflict with another
 *  app or a platform restriction (Wayland). */
export interface HotkeyStatus {
  binding: string;
  registered: boolean;
}

/** Result of a manual or auto update check (CLAUDE.md §5 carve-out).
 *  `latestVersion` and `releaseUrl` are non-null on a successful check;
 *  failures return as IPC errors instead, not as null fields. */
export interface UpdateStatus {
  currentVersion: string;
  latestVersion: string | null;
  hasUpdate: boolean;
  releaseUrl: string | null;
}

/** Mirrors `Settings` in src-tauri/src/settings.rs (CLAUDE.md §9). */
export interface Settings {
  hotkey: string;
  autostart: boolean;
  autoCopyOnSingleResult: boolean;
  theme: Theme;
  closeAfterCopy: boolean;
  closeAfterOpen: boolean;
  /** Opt-in: when true, qrab pings api.github.com once per launch to
   *  check for a newer release. Default false. */
  checkForUpdatesOnLaunch: boolean;
}
