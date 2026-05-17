import type { JSX } from "solid-js";
import { splitProps } from "solid-js";
import clsx from "clsx";

export function Card(props: JSX.HTMLAttributes<HTMLDivElement>): JSX.Element {
  const [local, rest] = splitProps(props, ["class", "children"]);
  return (
    <div
      {...rest}
      class={clsx(
        "rounded-lg border border-neutral-200 bg-white p-3 shadow-sm dark:border-neutral-800 dark:bg-neutral-900",
        local.class,
      )}
    >
      {local.children}
    </div>
  );
}
