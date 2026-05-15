// Sole IPC surface — every `invoke` lives here so call sites stay typed
// and there's one file to grep when the Rust side changes.

import { invoke } from "@tauri-apps/api/core";
import type { ScanResult } from "./types";

export const scanScreen = (): Promise<ScanResult> =>
  invoke<ScanResult>("scan_screen");
