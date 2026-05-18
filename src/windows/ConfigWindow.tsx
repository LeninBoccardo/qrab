import {
  Component,
  createResource,
  createSignal,
  onMount,
  Show,
} from "solid-js";
import { ArrowLeft, Download, Loader2 } from "lucide-solid";
import { Titlebar } from "../components/Titlebar";
import { Button } from "../components/ui/Button";
import * as Switch from "../components/ui/Switch";
import { Toaster, showToast } from "../components/ui/Toast";
import { formatError } from "../lib/format";
import {
  checkForUpdates,
  getAppInfo,
  hideResultsWindow,
} from "../lib/ipc";
import { loadSettings, saveSettings, settings } from "../lib/state";
import type { Settings, UpdateStatus } from "../lib/types";
import { openUrl as openExternal } from "@tauri-apps/plugin-opener";
import primaryLogo from "../../docs/branding/extracted/primary-logo.png";

export const ConfigWindow: Component = () => {
  const [info] = createResource(getAppInfo);
  const [saving, setSaving] = createSignal(false);
  const [checking, setChecking] = createSignal(false);
  const [updateStatus, setUpdateStatus] = createSignal<UpdateStatus | null>(
    null,
  );
  const [updateError, setUpdateError] = createSignal<string | null>(null);

  onMount(() => {
    if (!settings()) void loadSettings();
  });

  async function runUpdateCheck(): Promise<void> {
    setChecking(true);
    setUpdateError(null);
    try {
      const status = await checkForUpdates();
      setUpdateStatus(status);
    } catch (err) {
      setUpdateStatus(null);
      setUpdateError(formatError(err));
    } finally {
      setChecking(false);
    }
  }

  async function openReleasePage(): Promise<void> {
    const url = updateStatus()?.releaseUrl;
    if (!url) return;
    try {
      await openExternal(url);
    } catch (err) {
      showToast(`Couldn't open release page: ${formatError(err)}`);
    }
  }

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
              <div class="flex flex-col gap-3 text-sm text-neutral-900 dark:text-neutral-100">
                <img
                  src={primaryLogo}
                  alt="qrab"
                  class="h-10 w-auto self-start dark:invert dark:hue-rotate-180"
                />
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
            Updates
          </h2>
          <p class="text-xs text-neutral-500 dark:text-neutral-400">
            Compares the running version with the latest GitHub release.
            One GET to api.github.com per check — nothing else is sent.
          </p>
          <div class="flex items-center gap-3">
            <Button
              variant="secondary"
              onClick={() => void runUpdateCheck()}
              disabled={checking()}
            >
              {checking() ? (
                <>
                  <Loader2 size={14} class="animate-spin" /> Checking…
                </>
              ) : (
                "Check for updates"
              )}
            </Button>
            <Show when={!checking() && updateStatus()}>
              {(s) => (
                <Show
                  when={s().hasUpdate && s().latestVersion}
                  fallback={
                    <span class="text-xs text-neutral-500 dark:text-neutral-400">
                      You're on the latest version
                      <Show when={s().latestVersion}>
                        {" "}(v{s().latestVersion})
                      </Show>
                      .
                    </span>
                  }
                >
                  <div class="flex items-center gap-2 text-xs">
                    <span class="text-emerald-700 dark:text-emerald-400">
                      Update available: v{s().latestVersion}
                    </span>
                    <Show when={s().releaseUrl}>
                      <button
                        type="button"
                        onClick={() => void openReleasePage()}
                        class="inline-flex items-center gap-1 rounded border border-emerald-500/50 px-2 py-0.5 text-emerald-700 transition hover:bg-emerald-500/10 dark:text-emerald-300"
                      >
                        <Download size={12} /> View release
                      </button>
                    </Show>
                  </div>
                </Show>
              )}
            </Show>
            <Show when={!checking() && updateError()}>
              <span class="text-xs text-amber-700 dark:text-amber-400">
                Couldn't check: {updateError()}
              </span>
            </Show>
          </div>
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
