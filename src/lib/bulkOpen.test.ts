import { describe, expect, it } from "vitest";
import { planOpenAll } from "./bulkOpen";
import { BULK_OPEN_CONFIRM_THRESHOLD, type ScanRow } from "./types";

function row(id: number, kind: ScanRow["kind"], content = ""): ScanRow {
  return {
    id,
    batchId: "B",
    content: content || `row-${id}`,
    kind,
    monitorIndex: 0,
    scannedAt: id * 1000,
    opened: false,
    openedAt: null,
    copied: false,
    copiedAt: null,
  };
}

describe("planOpenAll", () => {
  it("partitions url rows from non-url rows", () => {
    const plan = planOpenAll([
      row(1, "url"),
      row(2, "text"),
      row(3, "url"),
      row(4, "wifi"),
    ]);
    expect(plan.urlRows.map((r) => r.id)).toEqual([1, 3]);
    expect(plan.skippedNonUrl).toBe(2);
  });

  it("does not need confirmation at or below the threshold", () => {
    const at = planOpenAll(
      Array.from({ length: BULK_OPEN_CONFIRM_THRESHOLD }, (_, i) =>
        row(i + 1, "url"),
      ),
    );
    expect(at.needsConfirm).toBe(false);
  });

  it("needs confirmation strictly above the threshold", () => {
    const above = planOpenAll(
      Array.from({ length: BULK_OPEN_CONFIRM_THRESHOLD + 1 }, (_, i) =>
        row(i + 1, "url"),
      ),
    );
    expect(above.needsConfirm).toBe(true);
  });

  it("counts only url rows toward the threshold", () => {
    // Mix of URLs + non-URLs where total is above threshold but URLs alone
    // are at the threshold — still no confirmation.
    const urls = Array.from({ length: BULK_OPEN_CONFIRM_THRESHOLD }, (_, i) =>
      row(i + 1, "url"),
    );
    const nonUrls = Array.from({ length: 5 }, (_, i) => row(100 + i, "text"));
    const plan = planOpenAll([...urls, ...nonUrls]);
    expect(plan.needsConfirm).toBe(false);
    expect(plan.skippedNonUrl).toBe(5);
  });

  it("handles an empty input", () => {
    const plan = planOpenAll([]);
    expect(plan.urlRows).toEqual([]);
    expect(plan.skippedNonUrl).toBe(0);
    expect(plan.needsConfirm).toBe(false);
  });

  it("handles a list with only non-url rows", () => {
    const plan = planOpenAll([row(1, "text"), row(2, "wifi")]);
    expect(plan.urlRows).toEqual([]);
    expect(plan.skippedNonUrl).toBe(2);
    expect(plan.needsConfirm).toBe(false);
  });
});
