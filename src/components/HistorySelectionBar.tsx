// Selection-bar UI for HistoryWindow. Pulled out of HistoryWindow so the
// orchestrator stays close to the §11 ~150-line budget and the bar's
// shape can be tested in isolation.

import type { Component } from "solid-js";
import { Copy, ExternalLink, Trash2 } from "lucide-solid";
import { Button } from "./ui/Button";

interface HistorySelectionBarProps {
  count: number;
  onClear: () => void;
  onCopyAsJson: () => void;
  onOpenUrls: () => void;
  onDelete: () => void;
}

export const HistorySelectionBar: Component<HistorySelectionBarProps> = (
  props,
) => (
  <div class="flex items-center gap-2 border-b border-blue-700/40 bg-blue-900/20 px-3 py-1.5 text-xs">
    <span class="text-blue-100">{props.count} selected</span>
    <span class="flex-1" />
    <Button variant="ghost" onClick={() => props.onClear()}>
      Clear
    </Button>
    <Button
      variant="secondary"
      onClick={() => props.onCopyAsJson()}
      title="Copy the selected rows to the clipboard as JSON"
    >
      <Copy size={14} /> Copy as JSON
    </Button>
    <Button variant="secondary" onClick={() => props.onOpenUrls()}>
      <ExternalLink size={14} /> Open URLs
    </Button>
    <Button variant="primary" onClick={() => props.onDelete()}>
      <Trash2 size={14} /> Delete selected
    </Button>
  </div>
);
