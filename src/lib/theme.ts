import type { Theme } from "./types";

const DARK_CLASS = "dark";

function systemPrefersDark(): boolean {
  return (
    typeof window !== "undefined" &&
    window.matchMedia?.("(prefers-color-scheme: dark)").matches === true
  );
}

/// Adds or removes the `.dark` class on `<html>` based on the requested
/// theme. styles.css declares `@custom-variant dark` keyed on this class,
/// so this single toggle drives every `dark:` utility across the app.
export function applyTheme(theme: Theme): void {
  const root = document.documentElement;
  const wantDark =
    theme === "dark" || (theme === "system" && systemPrefersDark());
  root.classList.toggle(DARK_CLASS, wantDark);
}

let systemListener: ((e: MediaQueryListEvent) => void) | null = null;

/// When theme is "system", listen for OS color-scheme flips so the app
/// follows along live. Switching to an explicit theme tears the listener
/// down. Idempotent for the same theme.
export function watchSystemTheme(theme: Theme): void {
  if (typeof window === "undefined" || !window.matchMedia) return;
  const mq = window.matchMedia("(prefers-color-scheme: dark)");
  if (systemListener) {
    mq.removeEventListener("change", systemListener);
    systemListener = null;
  }
  if (theme === "system") {
    systemListener = () => applyTheme("system");
    mq.addEventListener("change", systemListener);
  }
}
