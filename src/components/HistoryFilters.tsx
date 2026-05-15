import { Component, createSignal } from "solid-js";
import * as TextField from "./ui/TextField";
import type { HistoryFilter, QrKind } from "../lib/types";

/** What the filter bar emits — limit/offset are owned by the parent. */
export type FilterValue = Omit<HistoryFilter, "limit" | "offset">;

type OpenedFilter = "all" | "opened" | "unopened";

interface HistoryFiltersProps {
  value: FilterValue;
  onChange: (value: FilterValue) => void;
}

export const HistoryFilters: Component<HistoryFiltersProps> = (props) => {
  const [search, setSearch] = createSignal(props.value.search ?? "");
  const [kind, setKind] = createSignal<QrKind | "all">(
    props.value.kind ?? "all",
  );
  const [opened, setOpened] = createSignal<OpenedFilter>(
    props.value.openedOnly
      ? "opened"
      : props.value.unopenedOnly
        ? "unopened"
        : "all",
  );

  // Debounce the search keystrokes so typing doesn't fire a query per char.
  let debounceTimer: number | undefined;

  function emit(): void {
    if (debounceTimer !== undefined) window.clearTimeout(debounceTimer);
    debounceTimer = window.setTimeout(() => {
      const next: FilterValue = {
        search: search() || undefined,
        kind: kind() === "all" ? undefined : (kind() as QrKind),
        openedOnly: opened() === "opened" ? true : undefined,
        unopenedOnly: opened() === "unopened" ? true : undefined,
      };
      props.onChange(next);
    }, 200);
  }

  return (
    <div class="flex flex-wrap items-end gap-3 border-b border-neutral-200/60 px-3 py-2 dark:border-neutral-800/60">
      <TextField.Root
        value={search()}
        onChange={(v) => {
          setSearch(v);
          emit();
        }}
      >
        <TextField.Label>Search</TextField.Label>
        <TextField.Input placeholder="content contains..." />
      </TextField.Root>

      <div class="flex flex-col gap-1">
        <label class="text-xs font-medium text-neutral-600 dark:text-neutral-400">
          Kind
        </label>
        <select
          class="rounded-md border border-neutral-300 bg-white px-2.5 py-1.5 text-sm dark:border-neutral-700 dark:bg-neutral-900"
          value={kind()}
          onChange={(e) => {
            setKind(e.currentTarget.value as QrKind | "all");
            emit();
          }}
        >
          <option value="all">All</option>
          <option value="url">Link</option>
          <option value="text">Text</option>
          <option value="wifi">Wi-Fi</option>
          <option value="vcard">Contact</option>
          <option value="email">Email</option>
          <option value="phone">Phone</option>
          <option value="other">Other</option>
        </select>
      </div>

      <div class="flex flex-col gap-1">
        <label class="text-xs font-medium text-neutral-600 dark:text-neutral-400">
          Opened
        </label>
        <select
          class="rounded-md border border-neutral-300 bg-white px-2.5 py-1.5 text-sm dark:border-neutral-700 dark:bg-neutral-900"
          value={opened()}
          onChange={(e) => {
            setOpened(e.currentTarget.value as OpenedFilter);
            emit();
          }}
        >
          <option value="all">All</option>
          <option value="opened">Opened</option>
          <option value="unopened">Unopened</option>
        </select>
      </div>
    </div>
  );
};
