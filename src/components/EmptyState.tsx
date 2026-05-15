import type { Component } from "solid-js";
import { ScanLine } from "lucide-solid";

export const EmptyState: Component = () => {
  return (
    <div class="flex h-full flex-col items-center justify-center gap-3 text-neutral-500 dark:text-neutral-400">
      <ScanLine size={32} />
      <div class="text-sm">No QR codes detected.</div>
      <div class="text-xs text-neutral-400 dark:text-neutral-500">
        Try "Select region" to pick a specific area of the screen.
      </div>
    </div>
  );
};
