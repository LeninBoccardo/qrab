import { Component, createSignal, onMount, Show } from "solid-js";
import { ArrowLeft, Loader2, Trash2 } from "lucide-solid";
import { Titlebar } from "../components/Titlebar";
import { HistoryFilters, type FilterValue } from "../components/HistoryFilters";
import { HistoryTable } from "../components/HistoryTable";
import { HistorySelectionBar } from "../components/HistorySelectionBar";
import { ClearHistoryDialog } from "../components/ClearHistoryDialog";
import { Button } from "../components/ui/Button";
import { Toaster, showToast } from "../components/ui/Toast";
import { ConfirmOpenAll } from "../components/ConfirmOpenAll";
import {
  copyRow as copyRowIpc,
  copyRowsAsJson,
  hideResultsWindow,
  historyClear,
  historyDelete,
  historyDeleteBulk,
  historyQuery,
  openUrl,
  openUrlsBulk,
} from "../lib/ipc";
import { planOpenAll } from "../lib/bulkOpen";
import { formatError } from "../lib/format";
import type { HistoryFilter, ScanRow, SortDir } from "../lib/types";

const PAGE_SIZE = 50;

export const HistoryWindow: Component = () => {
  const [filter, setFilter] = createSignal<FilterValue>({});
  const [sortDir, setSortDir] = createSignal<SortDir>("desc");
  const [rows, setRows] = createSignal<ScanRow[]>([]);
  const [offset, setOffset] = createSignal(0);
  const [hasMore, setHasMore] = createSignal(false);
  const [loading, setLoading] = createSignal(false);
  const [selected, setSelected] = createSignal<Set<number>>(new Set<number>());
  const [clearConfirm, setClearConfirm] = createSignal(false);
  const [confirmOpen, setConfirmOpen] = createSignal(false);
  const [pendingUrlRows, setPendingUrlRows] = createSignal<ScanRow[]>([]);
  const [pendingSkipped, setPendingSkipped] = createSignal(0);

  async function load(reset: boolean): Promise<void> {
    if (loading()) return;
    setLoading(true);
    try {
      const off = reset ? 0 : offset();
      const f: HistoryFilter = {
        ...filter(),
        sortDir: sortDir(),
        limit: PAGE_SIZE,
        offset: off,
      };
      const result = await historyQuery(f);
      if (reset) {
        setRows(result);
        setSelected(new Set<number>());
      } else {
        setRows((prev) => [...prev, ...result]);
      }
      setOffset(off + result.length);
      setHasMore(result.length === PAGE_SIZE);
    } catch (err) {
      showToast(`History load failed: ${formatError(err)}`);
    } finally {
      setLoading(false);
    }
  }

  function applyFilter(f: FilterValue): void {
    setFilter(f);
    void load(true);
  }

  function toggleSort(): void {
    setSortDir((d) => (d === "desc" ? "asc" : "desc"));
    void load(true);
  }

  function toggleSelect(id: number): void {
    setSelected((prev) => {
      const next = new Set(prev);
      if (next.has(id)) next.delete(id);
      else next.add(id);
      return next;
    });
  }

  function selectAllVisible(): void {
    setSelected(new Set(rows().map((r) => r.id)));
  }

  function clearSelection(): void {
    setSelected(new Set<number>());
  }

  async function deleteSelected(): Promise<void> {
    const ids = [...selected()];
    if (ids.length === 0) return;
    try {
      const removed = await historyDeleteBulk(ids);
      showToast(
        `Deleted ${removed} ${removed === 1 ? "row" : "rows"}`,
      );
      await load(true);
    } catch (err) {
      showToast(`Delete failed: ${formatError(err)}`);
    }
  }

  async function copySelectedAsJson(): Promise<void> {
    const ids = [...selected()];
    if (ids.length === 0) return;
    try {
      const count = await copyRowsAsJson(ids);
      const now = Date.now();
      const idSet = new Set(ids);
      setRows((prev) =>
        prev.map((r) =>
          idSet.has(r.id) ? { ...r, copied: true, copiedAt: now } : r,
        ),
      );
      showToast(
        `Copied ${count} ${count === 1 ? "row" : "rows"} as JSON`,
      );
    } catch (err) {
      showToast(`Copy as JSON failed: ${formatError(err)}`);
    }
  }

  async function deleteOne(id: number): Promise<void> {
    try {
      await historyDelete(id);
      setRows((prev) => prev.filter((r) => r.id !== id));
      setSelected((prev) => {
        const next = new Set(prev);
        next.delete(id);
        return next;
      });
    } catch (err) {
      showToast(`Delete failed: ${formatError(err)}`);
    }
  }

  async function copyRow(row: ScanRow): Promise<void> {
    try {
      await copyRowIpc(row.id);
      const now = Date.now();
      setRows((prev) =>
        prev.map((r) =>
          r.id === row.id ? { ...r, copied: true, copiedAt: now } : r,
        ),
      );
      showToast("Copied to clipboard");
    } catch (err) {
      showToast(`Copy failed: ${formatError(err)}`);
    }
  }

  async function openRow(row: ScanRow): Promise<void> {
    try {
      await openUrl(row.id);
      // Reflect the new opened_at locally so the UI updates without a re-query
      setRows((prev) =>
        prev.map((r) =>
          r.id === row.id
            ? { ...r, opened: true, openedAt: Date.now() }
            : r,
        ),
      );
    } catch (err) {
      showToast(`Open failed: ${formatError(err)}`);
    }
  }

  function markRowsOpened(ids: number[]): void {
    if (ids.length === 0) return;
    const now = Date.now();
    const idSet = new Set(ids);
    setRows((prev) =>
      prev.map((r) =>
        idSet.has(r.id) ? { ...r, opened: true, openedAt: now } : r,
      ),
    );
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

  async function openSelected(): Promise<void> {
    const selectedRows = rows().filter((r) => selected().has(r.id));
    const plan = planOpenAll(selectedRows);
    if (plan.urlRows.length === 0) {
      showToast("No URLs in selection to open");
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

  async function confirmClearAll(): Promise<void> {
    try {
      await historyClear();
      setClearConfirm(false);
      await load(true);
      showToast("History cleared");
    } catch (err) {
      showToast(`Clear failed: ${formatError(err)}`);
    }
  }

  onMount(() => {
    void load(true);
  });

  return (
    <main class="flex h-full flex-col">
      <Titlebar
        title="qrab — history"
        actions={
          <Button
            variant="ghost"
            onClick={() => {
              window.location.hash = "results";
            }}
            title="Back to scan results"
          >
            <ArrowLeft size={14} /> Results
          </Button>
        }
        onClose={() => void hideResultsWindow()}
      />

      <HistoryFilters value={filter()} onChange={applyFilter} />

      <Show when={selected().size > 0}>
        <HistorySelectionBar
          count={selected().size}
          onClear={clearSelection}
          onCopyAsJson={() => void copySelectedAsJson()}
          onOpenUrls={() => void openSelected()}
          onDelete={() => void deleteSelected()}
        />
      </Show>

      <div class="min-h-0 flex-1 overflow-auto">
        <HistoryTable
          rows={rows()}
          selected={selected()}
          sortDir={sortDir()}
          onToggleSelect={toggleSelect}
          onSelectAll={selectAllVisible}
          onClearSelection={clearSelection}
          onSortToggle={toggleSort}
          onCopy={(r) => void copyRow(r)}
          onOpen={(r) => void openRow(r)}
          onDelete={(id) => void deleteOne(id)}
        />
      </div>

      <div class="flex shrink-0 items-center justify-between gap-3 border-t border-neutral-200/60 bg-neutral-50 px-3 py-2 text-xs text-neutral-500 dark:border-neutral-800/60 dark:bg-neutral-900">
        <Show when={hasMore()} fallback={<span />}>
          <Button
            variant="secondary"
            onClick={() => void load(false)}
            disabled={loading()}
          >
            {loading() ? (
              <>
                <Loader2 size={14} class="animate-spin" /> Loading…
              </>
            ) : (
              "Load more"
            )}
          </Button>
        </Show>
        <span>{rows().length} rows</span>
        <Button variant="ghost" onClick={() => setClearConfirm(true)}>
          <Trash2 size={14} /> Clear all
        </Button>
      </div>

      <ClearHistoryDialog
        open={clearConfirm()}
        onOpenChange={setClearConfirm}
        onConfirm={() => void confirmClearAll()}
      />

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
