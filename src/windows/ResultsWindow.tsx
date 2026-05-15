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
import { ScanLine } from "lucide-solid";
import { Toaster, showToast } from "../components/ui/Toast";
import { ResultCard } from "../components/ResultCard";
import { EmptyState } from "../components/EmptyState";
import { Button } from "../components/ui/Button";
import {
  copyToClipboard,
  hideResultsWindow,
  openUrl,
  scanScreen,
  SCAN_EVENT,
} from "../lib/ipc";
import type { ScanResult, ScanRow } from "../lib/types";

export const ResultsWindow: Component = () => {
  const [result, setResult] = createSignal<ScanResult | null>(null);
  const [focusIdx, setFocusIdx] = createSignal(0);
  const [loading, setLoading] = createSignal(false);

  const rows = createMemo<ScanRow[]>(() => result()?.rows ?? []);
  const hasScanned = createMemo(() => result() !== null);

  async function scan(): Promise<void> {
    if (loading()) return;
    setLoading(true);
    try {
      const r = await scanScreen();
      setResult(r);
      setFocusIdx(0);
    } catch (err) {
      showToast(`Scan failed: ${formatError(err)}`);
    } finally {
      setLoading(false);
    }
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

    onCleanup(() => {
      window.removeEventListener("keydown", onKeyDown);
      unlisten?.();
    });
  });

  return (
    <main class="flex h-full flex-col gap-3 p-4">
      <header class="flex items-center justify-between">
        <h1 class="text-sm font-semibold text-neutral-700 dark:text-neutral-200">
          qrab
        </h1>
        <Button variant="primary" onClick={scan} disabled={loading()}>
          <ScanLine size={16} />
          {loading() ? "Scanning…" : "Scan now"}
        </Button>
      </header>

      <div class="min-h-0 flex-1 overflow-auto">
        <Show
          when={rows().length > 0}
          fallback={
            <Show
              when={hasScanned()}
              fallback={
                <div class="flex h-full items-center justify-center text-sm text-neutral-500 dark:text-neutral-400">
                  Press Scan now to find QR codes on your screen.
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
