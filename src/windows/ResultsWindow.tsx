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
import {
  AlertTriangle,
  Crop,
  ExternalLink,
  History,
  Loader2,
  ScanLine,
  Settings,
  X,
} from "lucide-solid";
import { Toaster, showToast } from "../components/ui/Toast";
import { ResultCard } from "../components/ResultCard";
import { EmptyState } from "../components/EmptyState";
import { Titlebar } from "../components/Titlebar";
import { Button } from "../components/ui/Button";
import { ConfirmOpenAll } from "../components/ConfirmOpenAll";
import {
  consumePendingScan,
  copyRow as copyRowIpc,
  hideResultsWindow,
  openScreenRecordingPrefs,
  openUrl,
  openUrlsBulk,
  scanScreen,
  SCAN_EVENT,
} from "../lib/ipc";
import { info as logInfo } from "../lib/log";
import { planOpenAll } from "../lib/bulkOpen";
import { formatError } from "../lib/format";
import {
  scanResult,
  setActiveScreenshotId,
  setScanResult,
  settings,
} from "../lib/state";
import type { ScanRow } from "../lib/types";

export const ResultsWindow: Component = () => {
  const [focusIdx, setFocusIdx] = createSignal(0);
  const [loading, setLoading] = createSignal(false);

  const rows = createMemo<ScanRow[]>(() => scanResult()?.rows ?? []);
  const hasScanned = createMemo(() => scanResult() !== null);
  const urlRowCount = createMemo(
    () => rows().filter((r) => r.kind === "url").length,
  );

  const [confirmOpen, setConfirmOpen] = createSignal(false);
  const [pendingUrlRows, setPendingUrlRows] = createSignal<ScanRow[]>([]);
  const [pendingSkipped, setPendingSkipped] = createSignal(0);

  // Sticky banner shown when the OS denies screen recording access.
  // Cleared automatically on the next successful scan.
  const [permissionBlocked, setPermissionBlocked] = createSignal(false);
  const isMac = (): boolean => /Mac/i.test(navigator.userAgent);

  async function scan(): Promise<void> {
    if (loading()) return;
    setLoading(true);
    try {
      const r = await scanScreen();
      setScanResult(r);
      setFocusIdx(0);
      setPermissionBlocked(false);
      // Zero results → push the user straight into the region selector
      // with the freshly held screenshot. Per CLAUDE.md §5 flow.
      if (r.rows.length === 0) {
        setActiveScreenshotId(r.screenshotId);
        window.location.hash = "region";
        return;
      }
      // Single result + auto-copy enabled → copy without the click
      // (CLAUDE.md §9 autoCopyOnSingleResult). copyRow respects
      // closeAfterCopy so the whole flow can be one-shot when the user
      // wants it.
      if (r.rows.length === 1 && settings()?.autoCopyOnSingleResult) {
        await copyRow(r.rows[0]);
      }
    } catch (err) {
      const msg = formatError(err);
      // CaptureError::PermissionDenied stringifies to this. Switch from
      // ephemeral toast to a sticky banner with an actionable button.
      if (/screen recording permission/i.test(msg)) {
        setPermissionBlocked(true);
      } else {
        showToast(`Scan failed: ${msg}`);
      }
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
      await copyRowIpc(row.id);
      // Reflect the new copied_at locally so the Status column updates
      // without a re-query.
      const r = scanResult();
      if (r) {
        const now = Date.now();
        setScanResult({
          ...r,
          rows: r.rows.map((x) =>
            x.id === row.id ? { ...x, copied: true, copiedAt: now } : x,
          ),
        });
      }
      showToast("Copied to clipboard");
      if (settings()?.closeAfterCopy) {
        await hideResultsWindow();
      }
    } catch (err) {
      showToast(`Copy failed: ${formatError(err)}`);
    }
  }

  async function openRow(row: ScanRow): Promise<void> {
    try {
      await openUrl(row.id);
      if (settings()?.closeAfterOpen) {
        await hideResultsWindow();
      }
    } catch (err) {
      showToast(`Open failed: ${formatError(err)}`);
    }
  }

  function markRowsOpened(ids: number[]): void {
    const now = Date.now();
    const idSet = new Set(ids);
    const r = scanResult();
    if (!r) return;
    setScanResult({
      ...r,
      rows: r.rows.map((row) =>
        idSet.has(row.id) ? { ...row, opened: true, openedAt: now } : row,
      ),
    });
  }

  async function executeOpenAll(
    ids: number[],
    confirmed: boolean,
  ): Promise<void> {
    try {
      const result = await openUrlsBulk(ids, confirmed);
      markRowsOpened(result.opened);
      const lines = [`Opened ${result.opened.length}`];
      if (result.failed.length > 0)
        lines.push(`${result.failed.length} failed`);
      if (result.skippedNonUrl > 0)
        lines.push(`${result.skippedNonUrl} skipped`);
      showToast(lines.join(", "));
    } catch (err) {
      showToast(`Open all failed: ${formatError(err)}`);
    }
  }

  async function openAll(): Promise<void> {
    const plan = planOpenAll(rows());
    if (plan.urlRows.length === 0) {
      showToast("No URLs to open");
      return;
    }
    if (plan.needsConfirm) {
      setPendingUrlRows(plan.urlRows);
      setPendingSkipped(plan.skippedNonUrl);
      setConfirmOpen(true);
      return;
    }
    await executeOpenAll(
      plan.urlRows.map((r) => r.id),
      false,
    );
  }

  async function confirmOpenAllAndRun(): Promise<void> {
    setConfirmOpen(false);
    await executeOpenAll(
      pendingUrlRows().map((r) => r.id),
      true,
    );
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

    // `alive` closes the gap between unmount and these async Promises
    // resolving. Without it, navigating away from #results before
    // `listen(SCAN_EVENT)` resolved would leak the subscription — the
    // cleanup ran with `unlisten` still null, then the Promise resolved
    // and parked a now-orphan listener on the webview. The orphan
    // closure kept its own `loading` reference, so the in-scope loading
    // guard didn't prevent it from firing scan() the next time the user
    // pressed the hotkey. That's the root of the intermittent
    // "scan-on-route-change" symptom.
    let alive = true;
    let unlistenFn: UnlistenFn | null = null;

    void listen(SCAN_EVENT, () => {
      void logInfo("scan(): triggered via SCAN_EVENT");
      void scan();
    }).then((fn) => {
      if (!alive) {
        // Component unmounted while we were waiting for the listener to
        // register. Tear it down immediately so it never fires.
        fn();
      } else {
        unlistenFn = fn;
      }
    });

    // Cold-WebView2 path: hotkey fired before our SCAN_EVENT listener
    // attached. consume_pending_scan atomically clears + returns the flag.
    void consumePendingScan().then((pending) => {
      if (!alive) return;
      if (pending) {
        void logInfo("scan(): triggered via pending_scan flag");
        void scan();
      }
    });

    onCleanup(() => {
      alive = false;
      window.removeEventListener("keydown", onKeyDown);
      unlistenFn?.();
    });
  });

  return (
    <main class="flex h-full flex-col">
      <Titlebar onClose={() => void hideResultsWindow()} />

      <Show when={permissionBlocked()}>
        <div class="flex items-center gap-2 border-b border-amber-700/50 bg-amber-100 px-3 py-2 text-xs dark:bg-amber-900/30">
          <AlertTriangle
            size={14}
            class="shrink-0 text-amber-700 dark:text-amber-300"
          />
          <span class="flex-1 text-amber-900 dark:text-amber-100">
            Screen Recording permission is required for qrab to see your screen.
            <Show when={isMac()}>
              {" "}
              Open System Settings, enable qrab under Privacy &amp; Security →
              Screen Recording, then quit and reopen qrab.
            </Show>
          </span>
          <Show when={isMac()}>
            <Button
              variant="secondary"
              onClick={() => void openScreenRecordingPrefs()}
            >
              Open Settings
            </Button>
          </Show>
          <button
            type="button"
            class="grid h-5 w-5 place-items-center rounded text-amber-700 hover:bg-amber-200 dark:text-amber-300 dark:hover:bg-amber-900/50"
            onClick={() => setPermissionBlocked(false)}
            aria-label="Dismiss"
          >
            <X size={12} />
          </button>
        </div>
      </Show>

      <div class="flex shrink-0 items-center gap-2 border-b border-neutral-200/60 px-3 py-1.5 dark:border-neutral-800/60">
        <Button
          variant="ghost"
          onClick={() => {
            window.location.hash = "history";
          }}
          title="View scan history"
        >
          <History size={16} />
          History
        </Button>
        <Button
          variant="ghost"
          onClick={() => {
            window.location.hash = "settings";
          }}
          title="Open settings"
        >
          <Settings size={16} />
          Settings
        </Button>
        <span class="flex-1" />
        <Show when={urlRowCount() > 0}>
          <Button
            variant="secondary"
            onClick={() => void openAll()}
            title="Open every URL in the results"
          >
            <ExternalLink size={16} />
            Open all {urlRowCount() > 1 ? `(${urlRowCount()})` : ""}
          </Button>
        </Show>
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
          {loading() ? (
            <Loader2 size={16} class="animate-spin" />
          ) : (
            <ScanLine size={16} />
          )}
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

      <ConfirmOpenAll
        open={confirmOpen()}
        onOpenChange={setConfirmOpen}
        rows={pendingUrlRows()}
        skippedNonUrl={pendingSkipped()}
        onConfirm={() => void confirmOpenAllAndRun()}
      />

      <Toaster />
    </main>
  );
};
