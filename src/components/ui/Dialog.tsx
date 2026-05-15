// Namespace-export wrapper for @kobalte/core/dialog per CLAUDE.md §11.
// Portal-rendered — set z-index here in one place so the layer is
// consistent across windows that mount dialogs.

import * as KDialog from "@kobalte/core/dialog";
import type { ComponentProps } from "solid-js";
import { splitProps } from "solid-js";
import clsx from "clsx";

export const Root = KDialog.Root;
export const Trigger = KDialog.Trigger;
export const Portal = KDialog.Portal;
export const CloseButton = KDialog.CloseButton;

export function Overlay(props: ComponentProps<typeof KDialog.Overlay>) {
  const [local, rest] = splitProps(props, ["class"]);
  return (
    <KDialog.Overlay
      {...rest}
      class={clsx(
        "fixed inset-0 z-50 bg-black/50 backdrop-blur-sm",
        local.class,
      )}
    />
  );
}

export function Content(props: ComponentProps<typeof KDialog.Content>) {
  const [local, rest] = splitProps(props, ["class"]);
  return (
    <KDialog.Content
      {...rest}
      class={clsx(
        // Consumers pass a width via `class` (e.g. "w-[400px]"); max-w
        // keeps a sane cap on narrow windows.
        "fixed left-1/2 top-1/2 z-50 max-w-[90vw] -translate-x-1/2 -translate-y-1/2 rounded-lg border border-neutral-200 bg-white p-5 shadow-xl dark:border-neutral-800 dark:bg-neutral-900",
        local.class,
      )}
    />
  );
}

export function Title(props: ComponentProps<typeof KDialog.Title>) {
  const [local, rest] = splitProps(props, ["class"]);
  return (
    <KDialog.Title
      {...rest}
      class={clsx(
        "text-base font-semibold text-neutral-900 dark:text-neutral-100",
        local.class,
      )}
    />
  );
}

export function Description(
  props: ComponentProps<typeof KDialog.Description>,
) {
  const [local, rest] = splitProps(props, ["class"]);
  return (
    <KDialog.Description
      {...rest}
      class={clsx(
        "mt-1 text-sm text-neutral-600 dark:text-neutral-400",
        local.class,
      )}
    />
  );
}
