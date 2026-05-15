// Module-level signals shared across windows (we have one Tauri webview
// with hash-based routing, so module state persists across route changes).

import { createSignal } from "solid-js";
import type { ScanResult } from "./types";

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
