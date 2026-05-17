// Confirmation modal for the "Clear all history" footer action.

import type { Component } from "solid-js";
import { Button } from "./ui/Button";
import * as Dialog from "./ui/Dialog";

interface ClearHistoryDialogProps {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  onConfirm: () => void;
}

export const ClearHistoryDialog: Component<ClearHistoryDialogProps> = (
  props,
) => (
  <Dialog.Root open={props.open} onOpenChange={props.onOpenChange}>
    <Dialog.Portal>
      <Dialog.Overlay />
      <Dialog.Content class="w-[420px]">
        <Dialog.Title>Clear all history?</Dialog.Title>
        <Dialog.Description>
          This permanently deletes every row from the database. There's no undo.
        </Dialog.Description>
        <div class="mt-5 flex justify-end gap-2">
          <Button variant="secondary" onClick={() => props.onOpenChange(false)}>
            Cancel
          </Button>
          <Button variant="primary" onClick={() => props.onConfirm()}>
            Delete all
          </Button>
        </div>
      </Dialog.Content>
    </Dialog.Portal>
  </Dialog.Root>
);
