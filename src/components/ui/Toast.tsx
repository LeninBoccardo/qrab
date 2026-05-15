// Wrapper for Kobalte's toast. Namespace import per CLAUDE.md §11 so the
// compound parts are accessible as `Toast.Root`, `Toast.Title`, etc.

import * as Toast from "@kobalte/core/toast";
import { Portal } from "solid-js/web";
import type { JSX } from "solid-js";

/**
 * Imperative toast trigger. Mount [`Toaster`] once at the window root for
 * this to render anywhere.
 */
export function showToast(message: string): void {
  Toast.toaster.show((props) => (
    <Toast.Root
      toastId={props.toastId}
      class="pointer-events-auto rounded-md bg-neutral-800/95 px-4 py-2 text-sm text-neutral-100 shadow-lg backdrop-blur dark:bg-neutral-900/95"
    >
      <Toast.Title>{message}</Toast.Title>
    </Toast.Root>
  ));
}

/** Mount once per window — provides the live region the toaster targets. */
export function Toaster(): JSX.Element {
  return (
    <Portal>
      <Toast.Region>
        <Toast.List class="pointer-events-none fixed bottom-4 left-1/2 z-50 flex -translate-x-1/2 flex-col items-center gap-2 outline-none" />
      </Toast.Region>
    </Portal>
  );
}
