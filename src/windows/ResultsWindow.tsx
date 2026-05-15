import {
  Component,
  createMemo,
  createSignal,
  For,
  onCleanup,
  onMount,
  Show,
} from "solid-js";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { Crop, ScanLine } from "lucide-solid";
import { Toaster, showToast } from "../components/ui/Toast";
import { ResultCard } from "../components/ResultCard";
import { EmptyState } from "../components/EmptyState";
import { Titlebar } from "../components/Titlebar";
import { Button } from "../components/ui/Button";
import {
  consumePendingScan,
  copyToClipboard,
  hideResultsWindow,
  openUrl,
  scanScreen,
  SCAN_EVENT,
} from "../lib/ipc";
import {
  scanResult,
  setActiveScreenshotId,
  setScanResult,
} from "../lib/state";
import type { ScanRow } from "../lib/types";

export const ResultsWindow: Component = () => {
  const [focusIdx, setFocusIdx] = createSignal(0);
  const [loading, setLoading] = createSignal(false);

  const rows = createMemo<ScanRow[]>(() => scanResult()?.rows ?? []);
  const hasScanned = createMemo(() => scanResult() !== null);

  async function scan(): Promise<void> {
    if (loading()) return;
    setLoading(true);
    try {
      const r = await scanScreen();
      setScanResult(r);
      setFocusIdx(0);
      // Zero results → push the user straight into the region selector
      // with the freshly held screenshot. Per CLAUDE.md §5 flow.
      if (r.rows.length === 0) {
        setActiveScreenshotId(r.screenshotId);
        window.location.hash = "region";
      }
    } catch (err) {
      showToast(`Scan failed: ${formatError(err)}`);
    } finally {
      setLoading(false);
    }
  }

  function selectRegion(): void {
    const r = scanResult();
    if (!r) return;
    setActiveScreenshotId(r.screenshotId);
    window.location.hash = "region";
  }

  async function copyRow(row: ScanRow): Promise<void> {
    try {
      await copyToClipboard(row.content);
      showToast("Copied to clipboard");
    } catch (err) {
      showToast(`Copy failed: ${formatError(err)}`);
    }
  }

  async function openRow(row: ScanRow): Promise<void> {
    try {
      await openUrl(row.content);
    } catch (err) {
      showToast(`Open failed: ${formatError(err)}`);
    }
  }

  function onKeyDown(e: KeyboardEvent): void {
    const list = rows();
    if (list.length === 0) return;
    if (e.key === "ArrowDown") {
      e.preventDefault();
      setFocusIdx((i) => (i + 1) % list.length);
    } else if (e.key === "ArrowUp") {
      e.preventDefault();
      setFocusIdx((i) => (i - 1 + list.length) % list.length);
    } else if (e.key === "Enter") {
      const row = list[focusIdx()];
      if (!row) return;
      if (row.kind === "url") void openRow(row);
      else void copyRow(row);
    } else if (e.key === "Escape") {
      void hideResultsWindow();
    }
  }

  onMount(() => {
    window.addEventListener("keydown", onKeyDown);

    let unlisten: UnlistenFn | null = null;
    void listen(SCAN_EVENT, () => {
      void scan();
    }).then((fn) => {
      unlisten = fn;
    });

    // Pick up a hotkey/tray scan that fired before this listener attached
    // (cold WebView2 path). The event path above handles the warm case.
    void consumePendingScan().then((pending) => {
      if (pending) void scan();
    });

    onCleanup(() => {
      window.removeEventListener("keydown", onKeyDown);
      unlisten?.();
    });
  });

  return (
    <main class="flex h-full flex-col">
      <Titlebar onClose={() => void hideResultsWindow()} />

      <div class="flex shrink-0 items-center justify-end gap-2 border-b border-neutral-200/60 px-3 py-1.5 dark:border-neutral-800/60">
        <Button
          variant="secondary"
          onClick={selectRegion}
          disabled={!scanResult()}
          title="Refine by selecting a region of the screenshot"
        >
          <Crop size={16} />
          Select region
        </Button>
        <Button variant="primary" onClick={scan} disabled={loading()}>
          <ScanLine size={16} />
          {loading() ? "Scanning…" : "Scan now"}
        </Button>
      </div>

      <div class="min-h-0 flex-1 overflow-auto p-3">
        <Show
          when={rows().length > 0}
          fallback={
            <Show
              when={hasScanned()}
              fallback={
                <div class="flex h-full items-center justify-center text-sm text-neutral-500 dark:text-neutral-400">
                  Press the hotkey (Ctrl+Shift+Q) or Scan now to find QR codes
                  on your screen.
                </div>
              }
            >
              <EmptyState />
            </Show>
          }
        >
          <div class="flex flex-col gap-2">
            <For each={rows()}>
              {(row, i) => (
                <ResultCard
                  row={row}
                  focused={focusIdx() === i()}
                  onCopy={() => void copyRow(row)}
                  onOpen={() => void openRow(row)}
                />
              )}
            </For>
          </div>
        </Show>
      </div>

      <Toaster />
    </main>
  );
};

function formatError(err: unknown): string {
  if (typeof err === "string") return err;
  if (err instanceof Error) return err.message;
  return String(err);
}
