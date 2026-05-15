// Module-level signals shared across windows (we have one Tauri webview
// with hash-based routing, so module state persists across route changes).

import { createSignal } from "solid-js";
import {
  getSettings as getSettingsIpc,
  setSettings as setSettingsIpc,
} from "./ipc";
import { applyTheme, watchSystemTheme } from "./theme";
import type { ScanResult, Settings } from "./types";

/** The current scan result on display. Set by scan_screen (ResultsWindow)
 *  and by scan_region (RegionSelectWindow); read by ResultsWindow. */
export const [scanResult, setScanResult] = createSignal<ScanResult | null>(
  null,
);

/** The screenshot_id the region window should crop. Set when scan_screen
 *  returns zero results (or when the user clicks "Select region" in C10),
 *  cleared if the held screenshot is known to have expired. */
export const [activeScreenshotId, setActiveScreenshotId] = createSignal<
  string | null
>(null);

const [settingsSignal, setSettingsSignal] = createSignal<Settings | null>(null);

/** Current user settings, or `null` until the first successful load. */
export const settings = settingsSignal;

/** Fetch settings from the backend and apply theme. Idempotent; safe to
 *  call from multiple windows on mount. */
export async function loadSettings(): Promise<void> {
  try {
    const s = await getSettingsIpc();
    setSettingsSignal(s);
    applyTheme(s.theme);
    watchSystemTheme(s.theme);
  } catch {
    // Leave signal null; consumers handle null gracefully. The error is
    // already captured by the global error logging in index.tsx.
  }
}

/** Optimistically update the local signal, apply the theme, then push
 *  the new value to the backend (which persists + handles hotkey /
 *  autostart side effects). Throws on IPC failure so the caller can
 *  surface it. */
export async function saveSettings(next: Settings): Promise<void> {
  setSettingsSignal(next);
  applyTheme(next.theme);
  watchSystemTheme(next.theme);
  await setSettingsIpc(next);
}
