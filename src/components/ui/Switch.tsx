// Namespace-export wrapper for @kobalte/core/switch per CLAUDE.md §11.

import * as KSwitch from "@kobalte/core/switch";
import type { ComponentProps } from "solid-js";
import { splitProps } from "solid-js";
import clsx from "clsx";

export const Root = KSwitch.Root;
export const Input = KSwitch.Input;

export function Label(props: ComponentProps<typeof KSwitch.Label>) {
  const [local, rest] = splitProps(props, ["class"]);
  return (
    <KSwitch.Label
      {...rest}
      class={clsx(
        "text-sm font-medium text-neutral-900 dark:text-neutral-100",
        local.class,
      )}
    />
  );
}

export function Description(props: ComponentProps<typeof KSwitch.Description>) {
  const [local, rest] = splitProps(props, ["class"]);
  return (
    <KSwitch.Description
      {...rest}
      class={clsx(
        "text-xs text-neutral-500 dark:text-neutral-400",
        local.class,
      )}
    />
  );
}

export function Control(props: ComponentProps<typeof KSwitch.Control>) {
  const [local, rest] = splitProps(props, ["class"]);
  return (
    <KSwitch.Control
      {...rest}
      class={clsx(
        "inline-flex h-5 w-9 shrink-0 cursor-pointer items-center rounded-full bg-neutral-300 px-0.5 transition-colors data-[checked]:bg-blue-500 dark:bg-neutral-700",
        local.class,
      )}
    />
  );
}

export function Thumb(props: ComponentProps<typeof KSwitch.Thumb>) {
  const [local, rest] = splitProps(props, ["class"]);
  return (
    <KSwitch.Thumb
      {...rest}
      class={clsx(
        "block h-4 w-4 rounded-full bg-white shadow transition-transform data-[checked]:translate-x-4",
        local.class,
      )}
    />
  );
}
