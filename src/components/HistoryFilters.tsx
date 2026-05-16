import { Component, createSignal } from "solid-js";
import * as TextField from "./ui/TextField";
import type { HistoryFilter, QrKind, StatusFilter } from "../lib/types";

/** What the filter bar emits — limit/offset and sortDir are owned by the parent. */
export type FilterValue = Omit<HistoryFilter, "limit" | "offset" | "sortDir">;

/** Local 3-state for the C2 dropdown — C5 expands it to 4 states once
 *  the Status column / copied filter ship. */
type OpenedFilter = "all" | "opened" | "unopened";

function dateInputToMs(value: string, endOfDay: boolean): number | undefined {
  if (!value) return undefined;
  // <input type="date"> emits ISO-8601 yyyy-mm-dd. Parse as local time
  // (not UTC) so filters match what the user typed.
  const [y, m, d] = value.split("-").map(Number);
  if (!y || !m || !d) return undefined;
  const date = endOfDay
    ? new Date(y, m - 1, d, 23, 59, 59, 999)
    : new Date(y, m - 1, d, 0, 0, 0, 0);
  return date.getTime();
}

function msToDateInput(ms: number | undefined): string {
  if (ms === undefined) return "";
  const d = new Date(ms);
  const yyyy = d.getFullYear();
  const mm = String(d.getMonth() + 1).padStart(2, "0");
  const dd = String(d.getDate()).padStart(2, "0");
  return `${yyyy}-${mm}-${dd}`;
}

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
    props.value.status === "opened"
      ? "opened"
      : props.value.status === "untouched"
        ? "unopened"
        : "all",
  );
  const [from, setFrom] = createSignal(msToDateInput(props.value.from));
  const [to, setTo] = createSignal(msToDateInput(props.value.to));

  // Debounce the search keystrokes so typing doesn't fire a query per char.
  let debounceTimer: number | undefined;

  function emit(): void {
    if (debounceTimer !== undefined) window.clearTimeout(debounceTimer);
    debounceTimer = window.setTimeout(() => {
      const status: StatusFilter | undefined =
        opened() === "opened"
          ? "opened"
          : opened() === "unopened"
            ? "untouched"
            : undefined;
      const next: FilterValue = {
        search: search() || undefined,
        kind: kind() === "all" ? undefined : (kind() as QrKind),
        status,
        from: dateInputToMs(from(), false),
        to: dateInputToMs(to(), true),
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

      <div class="flex flex-col gap-1">
        <label class="text-xs font-medium text-neutral-600 dark:text-neutral-400">
          From
        </label>
        <input
          type="date"
          class="rounded-md border border-neutral-300 bg-white px-2.5 py-1.5 text-sm dark:border-neutral-700 dark:bg-neutral-900 dark:text-neutral-100 dark:[color-scheme:dark]"
          value={from()}
          onChange={(e) => {
            setFrom(e.currentTarget.value);
            emit();
          }}
        />
      </div>

      <div class="flex flex-col gap-1">
        <label class="text-xs font-medium text-neutral-600 dark:text-neutral-400">
          To
        </label>
        <input
          type="date"
          class="rounded-md border border-neutral-300 bg-white px-2.5 py-1.5 text-sm dark:border-neutral-700 dark:bg-neutral-900 dark:text-neutral-100 dark:[color-scheme:dark]"
          value={to()}
          onChange={(e) => {
            setTo(e.currentTarget.value);
            emit();
          }}
        />
      </div>
    </div>
  );
};
