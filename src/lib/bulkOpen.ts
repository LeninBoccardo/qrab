// Shared logic between ResultsWindow and HistoryWindow for the "Open all"
// flow. Both surfaces filter their selection to URL rows, check whether
// the count exceeds the safety threshold, and either fire openUrlsBulk
// directly or stage a ConfirmOpenAll modal first.

import { BULK_OPEN_CONFIRM_THRESHOLD, type ScanRow } from "./types";

export interface OpenAllPlan {
  /** URL rows that will be opened. */
  urlRows: ScanRow[];
  /** Non-URL rows in the input — surfaced as a "+N not opened" footnote. */
  skippedNonUrl: number;
  /** True when [`urlRows`] exceeds [`BULK_OPEN_CONFIRM_THRESHOLD`]. */
  needsConfirm: boolean;
}

/** Partition `rows` into URLs vs. non-URLs and decide whether the count
 *  triggers ConfirmOpenAll. Pure — caller drives the UI. */
export function planOpenAll(rows: ScanRow[]): OpenAllPlan {
  const urlRows = rows.filter((r) => r.kind === "url");
  return {
    urlRows,
    skippedNonUrl: rows.length - urlRows.length,
    needsConfirm: urlRows.length > BULK_OPEN_CONFIRM_THRESHOLD,
  };
}
