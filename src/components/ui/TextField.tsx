// Namespace-export wrapper for @kobalte/core/text-field per CLAUDE.md §11.
// Consumers do `import * as TextField from "../components/ui/TextField"`.

import * as KTextField from "@kobalte/core/text-field";
import type { ComponentProps } from "solid-js";
import { splitProps } from "solid-js";
import clsx from "clsx";

export const Root = KTextField.Root;

export function Label(props: ComponentProps<typeof KTextField.Label>) {
  const [local, rest] = splitProps(props, ["class"]);
  return (
    <KTextField.Label
      {...rest}
      class={clsx(
        "text-xs font-medium text-neutral-600 dark:text-neutral-400",
        local.class,
      )}
    />
  );
}

export function Input(props: ComponentProps<typeof KTextField.Input>) {
  const [local, rest] = splitProps(props, ["class"]);
  return (
    <KTextField.Input
      {...rest}
      class={clsx(
        "w-48 rounded-md border border-neutral-300 bg-white px-2.5 py-1.5 text-sm placeholder:text-neutral-400 focus:border-blue-500 focus:outline-none dark:border-neutral-700 dark:bg-neutral-900 dark:placeholder:text-neutral-500",
        local.class,
      )}
    />
  );
}

export function Description(
  props: ComponentProps<typeof KTextField.Description>,
) {
  const [local, rest] = splitProps(props, ["class"]);
  return (
    <KTextField.Description
      {...rest}
      class={clsx("text-xs text-neutral-500", local.class)}
    />
  );
}

export function ErrorMessage(
  props: ComponentProps<typeof KTextField.ErrorMessage>,
) {
  const [local, rest] = splitProps(props, ["class"]);
  return (
    <KTextField.ErrorMessage
      {...rest}
      class={clsx("text-xs text-red-500", local.class)}
    />
  );
}
