import {
  Component,
  createResource,
  createSignal,
  onMount,
  Show,
} from "solid-js";
import { ArrowLeft, Loader2 } from "lucide-solid";
import { Titlebar } from "../components/Titlebar";
import { Button } from "../components/ui/Button";
import * as Switch from "../components/ui/Switch";
import { Toaster, showToast } from "../components/ui/Toast";
import { formatError } from "../lib/format";
import { getAppInfo, hideResultsWindow } from "../lib/ipc";
import { loadSettings, saveSettings, settings } from "../lib/state";
import type { Settings } from "../lib/types";

export const ConfigWindow: Component = () => {
  const [info] = createResource(getAppInfo);
  const [saving, setSaving] = createSignal(false);

  onMount(() => {
    if (!settings()) void loadSettings();
  });

  async function save(next: Settings): Promise<void> {
    setSaving(true);
    try {
      await saveSettings(next);
    } catch (err) {
      showToast(`Save failed: ${formatError(err)}`);
    } finally {
      setSaving(false);
    }
  }

  function toggleAutostart(next: boolean): void {
    const current = settings();
    if (!current) return;
    void save({ ...current, autostart: next });
  }

  return (
    <main class="flex h-full flex-col">
      <Titlebar
        title="qrab — config"
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

      <div class="flex flex-1 flex-col gap-6 overflow-auto p-6">
        <section class="flex flex-col gap-2">
          <h2 class="text-xs font-semibold uppercase tracking-wide text-neutral-500 dark:text-neutral-400">
            About
          </h2>
          <Show
            when={info()}
            fallback={
              <div class="inline-flex items-center gap-1.5 text-sm text-neutral-500">
                <Loader2 size={14} class="animate-spin" />
                Loading…
              </div>
            }
          >
            {(i) => (
              <div class="flex flex-col gap-1.5 text-sm text-neutral-900 dark:text-neutral-100">
                <div class="flex items-baseline gap-2">
                  <span class="text-lg font-semibold">{i().name}</span>
                  <span class="font-mono text-xs text-neutral-500 dark:text-neutral-400">
                    v{i().version}
                  </span>
                </div>
                <p class="text-neutral-700 dark:text-neutral-300">
                  {i().description}
                </p>
                <p class="text-xs text-neutral-500 dark:text-neutral-400">
                  By {i().author}
                </p>
              </div>
            )}
          </Show>
        </section>

        <div class="border-t border-neutral-200/60 dark:border-neutral-800/60" />

        <section class="flex flex-col gap-3">
          <h2 class="text-xs font-semibold uppercase tracking-wide text-neutral-500 dark:text-neutral-400">
            Configuration
          </h2>
          <Show
            when={settings()}
            fallback={
              <div class="inline-flex items-center gap-1.5 text-sm text-neutral-500">
                <Loader2 size={14} class="animate-spin" />
                Loading…
              </div>
            }
          >
            {(s) => (
              <Switch.Root
                checked={s().autostart}
                onChange={toggleAutostart}
                class="flex items-start justify-between gap-4"
              >
                <div class="flex flex-col">
                  <Switch.Label>Start app at initialization</Switch.Label>
                  <Switch.Description>
                    Launch qrab automatically when you log in.
                  </Switch.Description>
                </div>
                <Switch.Input class="sr-only" />
                <Switch.Control>
                  <Switch.Thumb />
                </Switch.Control>
              </Switch.Root>
            )}
          </Show>
          <Show when={saving()}>
            <div class="text-xs text-neutral-500">Saving…</div>
          </Show>
        </section>
      </div>

      <Toaster />
    </main>
  );
};
