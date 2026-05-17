// Safety modal shown when the user asks to open more than
// BULK_OPEN_CONFIRM_THRESHOLD URLs at once (CLAUDE.md §10).
//
// Lists every URL — not just the count — so the user can scan for
// phishing domains before authorizing the open. The Rust side also
// enforces the threshold, so a UI bug can't bypass this check.

import { Component, For, Show } from "solid-js";
import { AlertTriangle, ExternalLink } from "lucide-solid";
import * as Dialog from "./ui/Dialog";
import { Button } from "./ui/Button";
import type { ScanRow } from "../lib/types";

interface ConfirmOpenAllProps {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  /** Only URL rows belong here — non-URL items are reported separately. */
  rows: ScanRow[];
  /** Number of selected non-URL rows — shown as a footnote. */
  skippedNonUrl: number;
  onConfirm: () => void;
}

export const ConfirmOpenAll: Component<ConfirmOpenAllProps> = (props) => {
  return (
    <Dialog.Root open={props.open} onOpenChange={props.onOpenChange}>
      <Dialog.Portal>
        <Dialog.Overlay />
        <Dialog.Content class="w-[520px]">
          <div class="flex items-start gap-3">
            <div class="rounded-full bg-amber-100 p-2 dark:bg-amber-900/40">
              <AlertTriangle
                size={18}
                class="text-amber-600 dark:text-amber-400"
              />
            </div>
            <div class="flex-1">
              <Dialog.Title>
                Open {props.rows.length}{" "}
                {props.rows.length === 1 ? "URL" : "URLs"}?
              </Dialog.Title>
              <Dialog.Description>
                Review the list. Each one opens in your default browser.
              </Dialog.Description>
            </div>
          </div>

          <ul class="mt-4 max-h-64 overflow-auto rounded-md border border-neutral-200 bg-neutral-50 p-1.5 dark:border-neutral-800 dark:bg-neutral-950">
            <For each={props.rows}>
              {(row) => (
                <li class="flex items-center gap-2 px-1.5 py-1 text-neutral-700 dark:text-neutral-300">
                  <ExternalLink size={12} class="shrink-0 text-neutral-400" />
                  <span class="truncate font-mono text-xs" title={row.content}>
                    {row.content}
                  </span>
                </li>
              )}
            </For>
          </ul>

          <Show when={props.skippedNonUrl > 0}>
            <p class="mt-2 text-xs text-neutral-500">
              +{props.skippedNonUrl} non-URL{" "}
              {props.skippedNonUrl === 1 ? "item" : "items"} will not be opened.
            </p>
          </Show>

          <div class="mt-5 flex justify-end gap-2">
            <Button
              variant="secondary"
              onClick={() => props.onOpenChange(false)}
            >
              Cancel
            </Button>
            <Button variant="primary" onClick={() => props.onConfirm()}>
              Open all {props.rows.length}
            </Button>
          </div>
        </Dialog.Content>
      </Dialog.Portal>
    </Dialog.Root>
  );
};
