import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { cleanup, render } from "@solidjs/testing-library";

// Mocks must be declared before importing the component under test —
// Vitest hoists `vi.mock` but bindings are resolved lazily.
const listenMock = vi.fn();
const unlistenMock = vi.fn();

vi.mock("@tauri-apps/api/event", () => ({
  listen: (...args: unknown[]) => listenMock(...args),
}));

vi.mock("../lib/log", () => ({
  info: vi.fn(),
  error: vi.fn(),
  installGlobalErrorLogging: vi.fn(),
}));

vi.mock("../lib/ipc", () => ({
  scanScreen: vi.fn(),
  scanRegion: vi.fn(),
  copyToClipboard: vi.fn(),
  copyRow: vi.fn().mockResolvedValue(undefined),
  copyRowsAsJson: vi.fn(),
  openUrl: vi.fn().mockResolvedValue(undefined),
  openUrlsBulk: vi.fn().mockResolvedValue({
    opened: [],
    failed: [],
    skippedNonUrl: 0,
  }),
  historyQuery: vi.fn(),
  historyDelete: vi.fn(),
  historyDeleteBulk: vi.fn(),
  historyClear: vi.fn(),
  hideResultsWindow: vi.fn().mockResolvedValue(undefined),
  consumePendingScan: vi.fn().mockResolvedValue(false),
  getScreenshotMonitors: vi.fn(),
  getScreenshotMonitorPng: vi.fn(),
  getSettings: vi.fn().mockResolvedValue({
    hotkey: "Ctrl+Shift+Q",
    autostart: false,
    autoCopyOnSingleResult: false,
    theme: "system",
    closeAfterCopy: false,
    closeAfterOpen: false,
    checkForUpdatesOnLaunch: false,
  }),
  setSettings: vi.fn(),
  getDefaultSettings: vi.fn(),
  getAppInfo: vi.fn(),
  getHotkeyStatus: vi.fn(),
  openScreenRecordingPrefs: vi.fn(),
  checkForUpdates: vi.fn(),
  SCAN_EVENT: "qrab:scan",
}));

// Module-under-test imports come AFTER the mocks above.
import { ResultsWindow } from "./ResultsWindow";
import { setScanResult } from "../lib/state";
import { copyRow, openUrl } from "../lib/ipc";
import type { ScanRow } from "../lib/types";

function makeRow(overrides: Partial<ScanRow> = {}): ScanRow {
  return {
    id: 1,
    batchId: "B",
    content: "x",
    kind: "url",
    monitorIndex: 0,
    scannedAt: 1,
    opened: false,
    openedAt: null,
    copied: false,
    copiedAt: null,
    ...overrides,
  };
}

beforeEach(() => {
  setScanResult(null);
  listenMock.mockReset();
  unlistenMock.mockReset();
  listenMock.mockResolvedValue(unlistenMock);
  vi.clearAllMocks();
  // Default: listen() returns the unlisten function via Promise.
  listenMock.mockResolvedValue(unlistenMock);
});

afterEach(() => cleanup());

describe("ResultsWindow keyboard nav", () => {
  it("ArrowDown then Enter on a URL row calls openUrl with that row's id", async () => {
    setScanResult({
      rows: [
        makeRow({ id: 10, kind: "url", content: "https://a.test" }),
        makeRow({ id: 20, kind: "url", content: "https://b.test" }),
        makeRow({ id: 30, kind: "url", content: "https://c.test" }),
      ],
      screenshotId: "s",
    });

    render(() => <ResultsWindow />);

    // Focus starts at idx 0. ArrowDown moves to idx 1, Enter triggers
    // the URL action on row id 20.
    window.dispatchEvent(new KeyboardEvent("keydown", { key: "ArrowDown" }));
    window.dispatchEvent(new KeyboardEvent("keydown", { key: "Enter" }));

    // Let the microtask that wraps openRow run.
    await Promise.resolve();
    expect(openUrl).toHaveBeenCalledTimes(1);
    expect(openUrl).toHaveBeenCalledWith(20);
  });

  it("ArrowUp from idx 0 wraps to the last row", async () => {
    setScanResult({
      rows: [makeRow({ id: 10 }), makeRow({ id: 20 }), makeRow({ id: 30 })],
      screenshotId: "s",
    });
    render(() => <ResultsWindow />);

    window.dispatchEvent(new KeyboardEvent("keydown", { key: "ArrowUp" }));
    window.dispatchEvent(new KeyboardEvent("keydown", { key: "Enter" }));
    await Promise.resolve();
    expect(openUrl).toHaveBeenCalledWith(30);
  });

  it("Enter on a non-URL row calls copyRow", async () => {
    setScanResult({
      rows: [makeRow({ id: 7, kind: "text", content: "plain text" })],
      screenshotId: "s",
    });
    render(() => <ResultsWindow />);

    window.dispatchEvent(new KeyboardEvent("keydown", { key: "Enter" }));
    await Promise.resolve();
    expect(copyRow).toHaveBeenCalledTimes(1);
    expect(copyRow).toHaveBeenCalledWith(7);
    expect(openUrl).not.toHaveBeenCalled();
  });

  it("keydown is a no-op when there are no rows", async () => {
    render(() => <ResultsWindow />);
    window.dispatchEvent(new KeyboardEvent("keydown", { key: "Enter" }));
    window.dispatchEvent(new KeyboardEvent("keydown", { key: "ArrowDown" }));
    await Promise.resolve();
    expect(openUrl).not.toHaveBeenCalled();
    expect(copyRow).not.toHaveBeenCalled();
  });
});

describe("ResultsWindow SCAN_EVENT listener lifecycle", () => {
  it("subscribes to SCAN_EVENT on mount and unsubscribes on unmount", async () => {
    const { unmount } = render(() => <ResultsWindow />);

    // listen() is called synchronously inside onMount.
    expect(listenMock).toHaveBeenCalledTimes(1);
    expect(listenMock).toHaveBeenCalledWith("qrab:scan", expect.any(Function));

    // Let the listen() Promise resolve so unlistenFn gets stored.
    await Promise.resolve();
    await Promise.resolve();

    unmount();
    // Cleanup must call the unlisten function returned by listen().
    expect(unlistenMock).toHaveBeenCalled();
  });

  it("unmount before listen() resolves still tears the orphan listener down", async () => {
    // Hold the resolution so we can unmount mid-flight — this is the
    // race condition fixed in 1574d50: an orphan listener used to leak.
    let resolve!: (fn: () => void) => void;
    listenMock.mockReturnValueOnce(
      new Promise<() => void>((r) => {
        resolve = r;
      }),
    );

    const { unmount } = render(() => <ResultsWindow />);
    unmount();
    // The listen() Promise resolves AFTER the component unmounted.
    resolve(unlistenMock);
    await Promise.resolve();
    await Promise.resolve();

    expect(unlistenMock).toHaveBeenCalled();
  });
});
