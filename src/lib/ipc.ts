// Sole IPC surface — every `invoke` lives here so call sites stay typed
// and there's one file to grep when the Rust side changes.

import { invoke } from "@tauri-apps/api/core";
import type {
  BulkOpenResult,
  HistoryFilter,
  RegionBounds,
  ScanResult,
  ScanRow,
  ScreenshotMonitorMeta,
  Settings,
} from "./types";

export const scanScreen = (): Promise<ScanResult> =>
  invoke<ScanResult>("scan_screen");

export const scanRegion = (
  screenshotId: string,
  bounds: RegionBounds,
): Promise<ScanResult> =>
  invoke<ScanResult>("scan_region", { screenshotId, bounds });

export const copyToClipboard = (text: string): Promise<void> =>
  invoke<void>("copy_to_clipboard", { text });

/** Copy a stored row's content to the clipboard and stamp `copied_at` on
 *  the row. Atomic on the Rust side. */
export const copyRow = (id: number): Promise<void> =>
  invoke<void>("copy_row", { id });

/** Bulk: serialize the rows as JSON, copy once, mark all as copied.
 *  Returns the count of rows written. */
export const copyRowsAsJson = (ids: number[]): Promise<number> =>
  invoke<number>("copy_rows_as_json", { ids });

/** Open the row's URL in the user's browser and stamp `opened_at`. */
export const openUrl = (id: number): Promise<void> =>
  invoke<void>("open_url", { id });

/** Open every URL in `ids`. Non-URL rows are skipped. If the URL count
 *  exceeds the threshold, pass `confirmed: true` (after surfacing
 *  ConfirmOpenAll) or the Rust side refuses. */
export const openUrlsBulk = (
  ids: number[],
  confirmed: boolean,
): Promise<BulkOpenResult> =>
  invoke<BulkOpenResult>("open_urls_bulk", { ids, confirmed });

export const historyQuery = (filter: HistoryFilter): Promise<ScanRow[]> =>
  invoke<ScanRow[]>("history_query", { filter });

export const historyDelete = (id: number): Promise<void> =>
  invoke<void>("history_delete", { id });

export const historyClear = (): Promise<void> =>
  invoke<void>("history_clear");

export const hideResultsWindow = (): Promise<void> =>
  invoke<void>("hide_results_window");

/** Atomically clears and returns the pending-scan flag. */
export const consumePendingScan = (): Promise<boolean> =>
  invoke<boolean>("consume_pending_scan");

export const getScreenshotMonitors = (
  screenshotId: string,
): Promise<ScreenshotMonitorMeta[]> =>
  invoke<ScreenshotMonitorMeta[]>("get_screenshot_monitors", { screenshotId });

/** Fetch a monitor's PNG bytes as a binary IPC response (Tauri returns
 *  an ArrayBuffer when the Rust side uses tauri::ipc::Response). */
export const getScreenshotMonitorPng = (
  screenshotId: string,
  monitorIndex: number,
): Promise<ArrayBuffer> =>
  invoke<ArrayBuffer>("get_screenshot_monitor_png", {
    screenshotId,
    monitorIndex,
  });

export const getSettings = (): Promise<Settings> =>
  invoke<Settings>("get_settings");

export const setSettings = (settings: Settings): Promise<void> =>
  invoke<void>("set_settings", { settings });

/** Event name the Rust hotkey handler emits on press. */
export const SCAN_EVENT = "qrab:scan";
