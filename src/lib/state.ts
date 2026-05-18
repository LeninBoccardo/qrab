// Module-level signals shared across windows (we have one Tauri webview
// with hash-based routing, so module state persists across route changes).

import { createSignal } from "solid-js";
import {
  checkForUpdates as checkForUpdatesIpc,
  getSettings as getSettingsIpc,
  getSupportedImageExtensions as getSupportedImageExtensionsIpc,
  setSettings as setSettingsIpc,
} from "./ipc";
import { error as logError, info as logInfo } from "./log";
import { applyTheme, watchSystemTheme } from "./theme";
import type { ScanResult, Settings, UpdateStatus } from "./types";

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
  } catch (e) {
    const msg = e instanceof Error ? e.message : String(e);
    void logError(`loadSettings failed: ${msg}`);
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

/** Latest update-check result (manual button or auto-check on launch).
 *  `null` until any check has run. Surfaced in ConfigWindow. */
export const [updateStatus, setUpdateStatus] =
  createSignal<UpdateStatus | null>(null);

/** Error string from the most recent failed update check, or null if
 *  the most recent attempt succeeded (or no attempt has run yet). */
export const [updateError, setUpdateError] = createSignal<string | null>(null);

/** Set while an update check is in flight. Drives the button's spinner. */
export const [updateChecking, setUpdateChecking] = createSignal(false);

/** Run a single update check and write the result to the module signals.
 *  Caller doesn't need to track loading state — `updateChecking` is set
 *  for the duration. Safe to call from anywhere. */
export async function runUpdateCheck(): Promise<void> {
  setUpdateChecking(true);
  setUpdateError(null);
  try {
    const status = await checkForUpdatesIpc();
    setUpdateStatus(status);
  } catch (err) {
    setUpdateStatus(null);
    setUpdateError(err instanceof Error ? err.message : String(err));
  } finally {
    setUpdateChecking(false);
  }
}

/** Lowercase image extensions (without leading dot) accepted by the
 *  backend's decode_image_file IPC. Loaded once at startup via
 *  loadSupportedImageExtensions; empty until then. */
export const [supportedImageExtensions, setSupportedImageExtensions] =
  createSignal<readonly string[]>([]);

/** Fetch the supported image extensions from the backend and cache them
 *  in the module signal. Fire-and-forget at app start; safe to call
 *  multiple times — the cache survives the second call. */
export async function loadSupportedImageExtensions(): Promise<void> {
  try {
    const exts = await getSupportedImageExtensionsIpc();
    setSupportedImageExtensions(exts.map((e) => e.toLowerCase()));
  } catch (err) {
    const msg = err instanceof Error ? err.message : String(err);
    void logError(`loadSupportedImageExtensions failed: ${msg}`);
  }
}

/** Returns true iff `path` ends in one of the cached extensions. False
 *  while the cache is still loading — the caller should treat that as
 *  "reject" rather than "accept blindly". */
export function isSupportedImagePath(path: string): boolean {
  const lower = path.toLowerCase();
  return supportedImageExtensions().some((ext) => lower.endsWith(`.${ext}`));
}

let autoUpdateCheckRan = false;

/** Idempotent: runs the update check at most once per app load, only if
 *  the user has opted into the auto-check toggle. Settings must already
 *  be loaded — call this *after* loadSettings(). Failures are logged to
 *  the per-launch log file and otherwise silent (no toast). */
export async function maybeRunAutoUpdateCheck(): Promise<void> {
  if (autoUpdateCheckRan) return;
  autoUpdateCheckRan = true;
  const s = settings();
  if (!s?.checkForUpdatesOnLaunch) return;
  void logInfo("auto update check: starting");
  await runUpdateCheck();
  const result = updateStatus();
  const err = updateError();
  if (err) void logError(`auto update check failed: ${err}`);
  else if (result)
    void logInfo(
      `auto update check: current=${result.currentVersion} ` +
        `latest=${result.latestVersion ?? "?"} hasUpdate=${result.hasUpdate}`,
    );
}
