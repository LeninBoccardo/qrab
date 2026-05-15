import {
  Component,
  createEffect,
  createSignal,
  onCleanup,
} from "solid-js";
import clsx from "clsx";
import { Button } from "./ui/Button";

interface Props {
  value: string;
  onChange: (next: string) => void;
}

const MODIFIER_KEYS = new Set(["Control", "Shift", "Alt", "Meta"]);

function chordFromEvent(e: KeyboardEvent): string | null {
  if (MODIFIER_KEYS.has(e.key)) return null;
  const parts: string[] = [];
  if (e.ctrlKey) parts.push("Ctrl");
  if (e.shiftKey) parts.push("Shift");
  if (e.altKey) parts.push("Alt");
  if (e.metaKey) parts.push("Cmd");
  // Reject combos without modifiers — a bare letter as a global hotkey
  // would hijack every keystroke in that app.
  if (parts.length === 0) return null;
  const key = e.key.length === 1 ? e.key.toUpperCase() : e.key;
  parts.push(key);
  return parts.join("+");
}

export const HotkeyInput: Component<Props> = (props) => {
  const [capturing, setCapturing] = createSignal(false);

  createEffect(() => {
    if (!capturing()) return;

    function handle(e: KeyboardEvent): void {
      e.preventDefault();
      e.stopPropagation();
      if (e.key === "Escape") {
        setCapturing(false);
        return;
      }
      const next = chordFromEvent(e);
      if (!next) return;
      props.onChange(next);
      setCapturing(false);
    }

    window.addEventListener("keydown", handle, true);
    onCleanup(() => window.removeEventListener("keydown", handle, true));
  });

  return (
    <div class="inline-flex items-center gap-2">
      <code
        class={clsx(
          "inline-flex min-w-[160px] items-center justify-center rounded-md border border-neutral-300 bg-neutral-50 px-2 py-1 text-center text-xs font-mono text-neutral-800 dark:border-neutral-700 dark:bg-neutral-900 dark:text-neutral-200",
          capturing() && "ring-2 ring-blue-500",
        )}
      >
        {capturing() ? "Press a combination…" : props.value}
      </code>
      <Button
        variant="secondary"
        onClick={() => setCapturing((c) => !c)}
        title={
          capturing()
            ? "Press Esc to cancel"
            : "Click then press a key combination"
        }
      >
        {capturing() ? "Cancel" : "Rebind"}
      </Button>
    </div>
  );
};
