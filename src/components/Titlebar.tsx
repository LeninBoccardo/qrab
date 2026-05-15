// Custom titlebar for windows with `decorations: false`.
//
// Carries the drag region (Tauri's `data-tauri-drag-region` attribute) and
// a Close button. Children without the attribute (i.e. the buttons) keep
// their normal click semantics — Tauri only starts a drag when the event
// target itself has the attribute.

import type { Component, JSX } from "solid-js";
import { X } from "lucide-solid";

interface TitlebarProps {
  title?: string;
  /** Right-side controls slot (rendered before the Close button). */
  actions?: JSX.Element;
  onClose: () => void;
}

export const Titlebar: Component<TitlebarProps> = (props) => {
  return (
    <div
      data-tauri-drag-region
      class="flex h-8 shrink-0 select-none items-center justify-between border-b border-neutral-200/60 bg-neutral-100/80 px-2 dark:border-neutral-800/60 dark:bg-neutral-900/80"
    >
      <div
        data-tauri-drag-region
        class="cursor-default truncate px-1 text-xs font-semibold text-neutral-700 dark:text-neutral-200"
      >
        {props.title ?? "qrab"}
      </div>
      <div class="flex items-center gap-1">
        {props.actions}
        <button
          type="button"
          onClick={() => props.onClose()}
          aria-label="Close"
          class="grid h-6 w-6 place-items-center rounded text-neutral-500 transition hover:bg-red-500 hover:text-white focus-visible:outline-2 focus-visible:outline-offset-1 focus-visible:outline-blue-500 dark:text-neutral-400"
        >
          <X size={14} />
        </button>
      </div>
    </div>
  );
};
