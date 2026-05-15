// Sole IPC surface — every `invoke` lives here so call sites stay typed
// and there's one file to grep when the Rust side changes.

import { invoke } from "@tauri-apps/api/core";
import type { ScanResult } from "./types";

export const scanScreen = (): Promise<ScanResult> =>
  invoke<ScanResult>("scan_screen");

export const copyToClipboard = (text: string): Promise<void> =>
  invoke<void>("copy_to_clipboard", { text });

export const openUrl = (url: string): Promise<void> =>
  invoke<void>("open_url", { url });
