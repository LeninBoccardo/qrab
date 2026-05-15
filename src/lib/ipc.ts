// Sole IPC surface — every `invoke` lives here so call sites stay typed
// and there's one file to grep when the Rust side changes.

import { invoke } from "@tauri-apps/api/core";
import type {
  RegionBounds,
  ScanResult,
  ScreenshotMonitorMeta,
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

export const openUrl = (url: string): Promise<void> =>
  invoke<void>("open_url", { url });

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

/** Event name the Rust hotkey handler emits on press. */
export const SCAN_EVENT = "qrab:scan";
