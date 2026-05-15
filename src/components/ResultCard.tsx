import type { Component } from "solid-js";
import { Show } from "solid-js";
import { Dynamic } from "solid-js/web";
import { Copy, ExternalLink } from "lucide-solid";
import type { ScanRow } from "../lib/types";
import { isOpenable, kindIcon, kindLabel } from "../lib/classify";
import { Button } from "./ui/Button";
import { Card } from "./ui/Card";

interface ResultCardProps {
  row: ScanRow;
  focused: boolean;
  onCopy: () => void;
  onOpen: () => void;
}

export const ResultCard: Component<ResultCardProps> = (props) => {
  return (
    <Card
      class={
        props.focused
          ? "ring-2 ring-blue-500 transition dark:ring-blue-400"
          : "transition"
      }
    >
      <div class="flex items-start gap-3">
        <div class="mt-0.5 text-neutral-500 dark:text-neutral-400">
          <Dynamic component={kindIcon(props.row.kind)} size={18} />
        </div>
        <div class="min-w-0 flex-1">
          <div class="text-xs uppercase tracking-wide text-neutral-500 dark:text-neutral-400">
            {kindLabel(props.row.kind)}
          </div>
          <div class="mt-0.5 truncate text-sm text-neutral-900 dark:text-neutral-100">
            {props.row.content}
          </div>
        </div>
        <div class="flex shrink-0 gap-1">
          <Button variant="ghost" onClick={props.onCopy} aria-label="Copy">
            <Copy size={16} />
          </Button>
          <Show when={isOpenable(props.row.kind)}>
            <Button variant="primary" onClick={props.onOpen} aria-label="Open">
              <ExternalLink size={16} />
            </Button>
          </Show>
        </div>
      </div>
    </Card>
  );
};
