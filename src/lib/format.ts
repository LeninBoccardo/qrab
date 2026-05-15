// Pure formatting helpers — no Tauri imports, no DOM dependencies.

/** "5m ago", "2h ago", "3d ago", or a locale date for older entries. */
export function relativeTime(ms: number): string {
  const diff = Date.now() - ms;
  if (diff < 60_000) return "just now";
  if (diff < 3_600_000) return `${Math.floor(diff / 60_000)}m ago`;
  if (diff < 86_400_000) return `${Math.floor(diff / 3_600_000)}h ago`;
  if (diff < 7 * 86_400_000) return `${Math.floor(diff / 86_400_000)}d ago`;
  return new Date(ms).toLocaleDateString();
}

/** Full locale string — used in tooltips. */
export function absoluteTime(ms: number): string {
  return new Date(ms).toLocaleString();
}

/** Convert an unknown thrown value (string, Error, anything else) into a
 *  display-ready message. Used for IPC error toasts across windows. */
export function formatError(err: unknown): string {
  if (typeof err === "string") return err;
  if (err instanceof Error) return err.message;
  return String(err);
}
