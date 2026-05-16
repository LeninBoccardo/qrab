import { Component, createEffect, For, Show } from "solid-js";
import { Dynamic } from "solid-js/web";
import { ChevronDown, ChevronUp, Copy, ExternalLink, Trash2 } from "lucide-solid";
import type { ScanRow, SortDir } from "../lib/types";
import { isOpenable, kindIcon, kindLabel } from "../lib/classify";
import { absoluteTime, relativeTime } from "../lib/format";

interface HistoryTableProps {
    rows: ScanRow[];
    selected: Set<number>;
    sortDir: SortDir;
    onToggleSelect: (id: number) => void;
    onSelectAll: () => void;
    onClearSelection: () => void;
    onSortToggle: () => void;
    onCopy: (row: ScanRow) => void;
    onOpen: (row: ScanRow) => void;
    onDelete: (id: number) => void;
}

export const HistoryTable: Component<HistoryTableProps> = (props) => {
    const allSelected = (): boolean =>
        props.rows.length > 0 && props.rows.every((r) => props.selected.has(r.id));
    const someSelected = (): boolean => props.rows.some((r) => props.selected.has(r.id));

    function toggleAll(): void {
        if (allSelected()) props.onClearSelection();
        else props.onSelectAll();
    }

    // <input>.indeterminate is a property, not a markup attribute, so set it
    // through a ref after each render rather than via a JSX prop.
    let headerCheckbox: HTMLInputElement | undefined;
    createEffect(() => {
        if (headerCheckbox) {
            headerCheckbox.indeterminate = someSelected() && !allSelected();
        }
    });

    return (
        <table class="w-full text-sm">
            <thead class="sticky top-0 bg-neutral-100/95 text-left text-xs uppercase text-neutral-500 backdrop-blur dark:bg-neutral-900/95 dark:text-neutral-400">
                <tr>
                    <th class="w-10 px-3 py-2">
                        <input
                            ref={headerCheckbox}
                            type="checkbox"
                            checked={allSelected()}
                            onChange={toggleAll}
                            aria-label="Select all visible"
                        />
                    </th>
                    <th class="w-24 px-2 py-2">Kind</th>
                    <th class="px-2 py-2">Content</th>
                    <th class="w-32 px-2 py-2">
                        <button
                            type="button"
                            class="inline-flex items-center gap-1 uppercase text-neutral-500 hover:text-neutral-700 dark:text-neutral-400 dark:hover:text-neutral-200"
                            onClick={() => props.onSortToggle()}
                            aria-label={
                                props.sortDir === "asc"
                                    ? "Sort scanned descending"
                                    : "Sort scanned ascending"
                            }
                            title="Toggle scanned-date order"
                        >
                            Scanned
                            {props.sortDir === "asc" ? (
                                <ChevronUp size={12} />
                            ) : (
                                <ChevronDown size={12} />
                            )}
                        </button>
                    </th>
                    <th class="w-36 px-2 py-2">Status</th>
                    <th class="w-32 px-2 py-2 text-right">Actions</th>
                </tr>
            </thead>
            <tbody>
                <Show
                    when={props.rows.length > 0}
                    fallback={
                        <tr>
                            <td colspan={6} class="px-3 py-12 text-center text-sm text-neutral-500">
                                No rows match the current filters.
                            </td>
                        </tr>
                    }
                >
                    <For each={props.rows}>
                        {(row) => (
                            <tr class="border-t border-neutral-200/40 hover:bg-neutral-50 dark:border-neutral-800/40 dark:hover:bg-neutral-900/50">
                                <td class="px-3 py-2">
                                    <input
                                        type="checkbox"
                                        checked={props.selected.has(row.id)}
                                        onChange={() => props.onToggleSelect(row.id)}
                                        aria-label="Select row"
                                    />
                                </td>
                                <td class="px-2 py-2">
                                    <div class="inline-flex items-center gap-1.5 text-neutral-600 dark:text-neutral-300">
                                        <Dynamic component={kindIcon(row.kind)} size={14} />
                                        <span class="text-xs">{kindLabel(row.kind)}</span>
                                    </div>
                                </td>
                                <td class="px-2 py-2">
                                    <div class="max-w-100 truncate" title={row.content}>
                                        {row.content}
                                    </div>
                                </td>
                                <td
                                    class="px-2 py-2 text-xs text-neutral-500"
                                    title={absoluteTime(row.scannedAt)}
                                >
                                    {relativeTime(row.scannedAt)}
                                </td>
                                <td class="px-2 py-2 text-xs">
                                    <div class="flex flex-wrap items-center gap-1">
                                        <Show when={row.opened}>
                                            <span
                                                class="rounded bg-green-100 px-1.5 py-0.5 text-[11px] font-medium text-green-700 dark:bg-green-900/30 dark:text-green-300"
                                                title={
                                                    row.openedAt !== null
                                                        ? `Opened ${absoluteTime(row.openedAt)}`
                                                        : undefined
                                                }
                                            >
                                                Opened
                                            </span>
                                        </Show>
                                        <Show when={row.copied}>
                                            <span
                                                class="rounded bg-blue-100 px-1.5 py-0.5 text-[11px] font-medium text-blue-700 dark:bg-blue-900/30 dark:text-blue-300"
                                                title={
                                                    row.copiedAt !== null
                                                        ? `Copied ${absoluteTime(row.copiedAt)}`
                                                        : undefined
                                                }
                                            >
                                                Copied
                                            </span>
                                        </Show>
                                        <Show when={!row.opened && !row.copied}>
                                            <span class="text-neutral-400">—</span>
                                        </Show>
                                    </div>
                                </td>
                                <td class="px-2 py-2 text-right">
                                    <div class="inline-flex items-center gap-0.5">
                                        <button
                                            type="button"
                                            class="rounded p-1 text-neutral-500 hover:bg-neutral-200 dark:hover:bg-neutral-800"
                                            onClick={() => props.onCopy(row)}
                                            aria-label="Copy"
                                        >
                                            <Copy size={14} />
                                        </button>
                                        <Show when={isOpenable(row.kind)}>
                                            <button
                                                type="button"
                                                class="rounded p-1 text-neutral-500 hover:bg-neutral-200 dark:hover:bg-neutral-800"
                                                onClick={() => props.onOpen(row)}
                                                aria-label="Open"
                                            >
                                                <ExternalLink size={14} />
                                            </button>
                                        </Show>
                                        <button
                                            type="button"
                                            class="rounded p-1 text-red-500 hover:bg-red-100 dark:hover:bg-red-900/30"
                                            onClick={() => props.onDelete(row.id)}
                                            aria-label="Delete"
                                        >
                                            <Trash2 size={14} />
                                        </button>
                                    </div>
                                </td>
                            </tr>
                        )}
                    </For>
                </Show>
            </tbody>
        </table>
    );
};
