import { Component, createSignal, onMount, Show } from "solid-js";
import { Titlebar } from "../components/Titlebar";
import { HotkeyInput } from "../components/HotkeyInput";
import * as Switch from "../components/ui/Switch";
import { Toaster, showToast } from "../components/ui/Toast";
import { hideResultsWindow } from "../lib/ipc";
import { loadSettings, saveSettings, settings } from "../lib/state";
import type { Settings, Theme } from "../lib/types";

export const SettingsWindow: Component = () => {
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

  function update<K extends keyof Settings>(key: K, value: Settings[K]): void {
    const current = settings();
    if (!current) return;
    void save({ ...current, [key]: value });
  }

  return (
    <main class="flex h-full flex-col">
      <Titlebar
        title="qrab — settings"
        onClose={() => void hideResultsWindow()}
      />

      <Show
        when={settings()}
        fallback={
          <div class="flex flex-1 items-center justify-center text-sm text-neutral-500">
            Loading…
          </div>
        }
      >
        {(s) => (
          <div class="flex flex-1 flex-col gap-5 overflow-auto p-5">
            <Row label="Hotkey" hint="Global shortcut to capture and decode.">
              <HotkeyInput
                value={s().hotkey}
                onChange={(v) => update("hotkey", v)}
              />
            </Row>

            <Row label="Theme">
              <select
                class="rounded-md border border-neutral-300 bg-white px-2 py-1.5 text-sm dark:border-neutral-700 dark:bg-neutral-900 dark:text-neutral-100"
                value={s().theme}
                onChange={(e) =>
                  update("theme", e.currentTarget.value as Theme)
                }
              >
                <option value="system">System</option>
                <option value="light">Light</option>
                <option value="dark">Dark</option>
              </select>
            </Row>

            <ToggleRow
              label="Launch on login"
              checked={s().autostart}
              onChange={(v) => update("autostart", v)}
            />

            <ToggleRow
              label="Auto-copy when a scan finds one result"
              hint="Skip the click — drop the decoded text on the clipboard automatically."
              checked={s().autoCopyOnSingleResult}
              onChange={(v) => update("autoCopyOnSingleResult", v)}
            />

            <ToggleRow
              label="Close after copy"
              checked={s().closeAfterCopy}
              onChange={(v) => update("closeAfterCopy", v)}
            />

            <ToggleRow
              label="Close after open"
              checked={s().closeAfterOpen}
              onChange={(v) => update("closeAfterOpen", v)}
            />

            <Show when={saving()}>
              <div class="text-xs text-neutral-500">Saving…</div>
            </Show>
          </div>
        )}
      </Show>

      <Toaster />
    </main>
  );
};

interface RowProps {
  label: string;
  hint?: string;
  children: any;
}

const Row: Component<RowProps> = (props) => (
  <div class="flex items-start justify-between gap-4">
    <div class="flex flex-col">
      <span class="text-sm font-medium text-neutral-900 dark:text-neutral-100">
        {props.label}
      </span>
      <Show when={props.hint}>
        <span class="text-xs text-neutral-500 dark:text-neutral-400">
          {props.hint}
        </span>
      </Show>
    </div>
    <div class="shrink-0">{props.children}</div>
  </div>
);

interface ToggleRowProps {
  label: string;
  hint?: string;
  checked: boolean;
  onChange: (next: boolean) => void;
}

const ToggleRow: Component<ToggleRowProps> = (props) => (
  <Switch.Root
    checked={props.checked}
    onChange={props.onChange}
    class="flex items-start justify-between gap-4"
  >
    <div class="flex flex-col">
      <Switch.Label>{props.label}</Switch.Label>
      <Show when={props.hint}>
        <Switch.Description>{props.hint}</Switch.Description>
      </Show>
    </div>
    <Switch.Input class="sr-only" />
    <Switch.Control>
      <Switch.Thumb />
    </Switch.Control>
  </Switch.Root>
);

function formatError(err: unknown): string {
  if (typeof err === "string") return err;
  if (err instanceof Error) return err.message;
  return String(err);
}
