import {
  Component,
  createEffect,
  createMemo,
  createResource,
  createSignal,
  For,
  on,
  onCleanup,
  Show,
} from "solid-js";
import { Loader2 } from "lucide-solid";
import { Titlebar } from "../components/Titlebar";
import { RegionSelector, type Bounds } from "../components/RegionSelector";
import { Toaster, showToast } from "../components/ui/Toast";
import {
  getScreenshotMonitorPng,
  getScreenshotMonitors,
  hideResultsWindow,
  scanRegion,
} from "../lib/ipc";
import { formatError } from "../lib/format";
import { activeScreenshotId, setScanResult } from "../lib/state";

export const RegionSelectWindow: Component = () => {
  const [monitorIdx, setMonitorIdx] = createSignal(0);
  const [emptyMessage, setEmptyMessage] = createSignal<string | null>(null);
  const [imageUrl, setImageUrl] = createSignal<string | null>(null);
  // Monotonic request token. The PNG load effect awaits an async IPC; if the
  // user clicks a different monitor before the previous call resolves, the
  // stale response must not overwrite the newer image.
  let pngRequestSeq = 0;

  const [monitors] = createResource(activeScreenshotId, async (id) => {
    if (!id) return [];
    try {
      return await getScreenshotMonitors(id);
    } catch (err) {
      showToast(`Failed to load screenshot: ${formatError(err)}`);
      return [];
    }
  });

  const currentMonitor = createMemo(() => {
    const ms = monitors() ?? [];
    if (ms.length === 0) return null;
    return ms[Math.min(monitorIdx(), ms.length - 1)] ?? null;
  });

  // Load the PNG whenever the active screenshot or selected monitor changes.
  // Always revoke the previous Blob URL so the old image bytes can be GC'd.
  createEffect(
    on(
      () => [activeScreenshotId(), currentMonitor()?.index] as const,
      async ([id, mIdx]) => {
        const myReq = ++pngRequestSeq;
        const previous = imageUrl();
        if (previous) URL.revokeObjectURL(previous);
        setImageUrl(null);
        setEmptyMessage(null);
        if (!id || mIdx === undefined) return;
        try {
          const buf = await getScreenshotMonitorPng(id, mIdx);
          if (myReq !== pngRequestSeq) return;
          const blob = new Blob([buf], { type: "image/png" });
          setImageUrl(URL.createObjectURL(blob));
        } catch (err) {
          if (myReq !== pngRequestSeq) return;
          showToast(`Failed to load screenshot: ${formatError(err)}`);
        }
      },
    ),
  );

  onCleanup(() => {
    const u = imageUrl();
    if (u) URL.revokeObjectURL(u);
  });

  async function onDecode(bounds: Bounds): Promise<void> {
    const id = activeScreenshotId();
    const mon = currentMonitor();
    if (!id || !mon) return;
    try {
      const r = await scanRegion(id, { ...bounds, monitorIndex: mon.index });
      if (r.rows.length === 0) {
        setEmptyMessage("Nothing found in that region — try again.");
        return;
      }
      setScanResult(r);
      window.location.hash = "results";
    } catch (err) {
      showToast(`Region decode failed: ${formatError(err)}`);
    }
  }

  function onCancel(): void {
    window.location.hash = "results";
  }

  return (
    <main class="flex h-full flex-col">
      <Titlebar
        title="qrab — select region"
        onClose={() => void hideResultsWindow()}
      />

      <Show when={emptyMessage()}>
        <div class="border-b border-amber-700/50 bg-amber-900/30 px-3 py-1.5 text-xs text-amber-200">
          {emptyMessage()}
        </div>
      </Show>

      <Show when={(monitors() ?? []).length > 1}>
        <div class="flex items-center gap-2 border-b border-neutral-800 bg-neutral-900 px-3 py-1.5 text-xs text-neutral-300">
          <span class="text-neutral-500">Monitor:</span>
          <For each={monitors() ?? []}>
            {(_m, i) => (
              <button
                type="button"
                onClick={() => setMonitorIdx(i())}
                class={
                  "rounded px-2 py-0.5 " +
                  (monitorIdx() === i()
                    ? "bg-blue-600 text-white"
                    : "bg-neutral-800 text-neutral-300 hover:bg-neutral-700")
                }
              >
                {i() + 1}
              </button>
            )}
          </For>
        </div>
      </Show>

      <div class="min-h-0 flex-1">
        <Show
          when={imageUrl() && currentMonitor()}
          fallback={
            <div class="flex h-full items-center justify-center px-6 text-center text-sm text-neutral-400">
              <Show
                when={activeScreenshotId()}
                fallback={
                  <span>
                    No screenshot available. Press the hotkey to scan first.
                  </span>
                }
              >
                <span class="inline-flex items-center gap-1.5">
                  <Loader2 size={14} class="animate-spin" />
                  Loading screenshot…
                </span>
              </Show>
            </div>
          }
        >
          <RegionSelector
            imageSrc={imageUrl()!}
            imageWidth={currentMonitor()!.width}
            imageHeight={currentMonitor()!.height}
            onDecode={onDecode}
            onCancel={onCancel}
          />
        </Show>
      </div>

      <Toaster />
    </main>
  );
};
