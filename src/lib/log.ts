import {
  debug,
  error,
  info,
  trace,
  warn,
} from "@tauri-apps/plugin-log";

export { debug, error, info, trace, warn };

function describe(value: unknown): string {
  if (value instanceof Error) {
    return `${value.name}: ${value.message}${value.stack ? `\n${value.stack}` : ""}`;
  }
  if (typeof value === "string") return value;
  try {
    return JSON.stringify(value);
  } catch {
    return String(value);
  }
}

/// Installs window-level handlers so uncaught errors and unhandled promise
/// rejections from every route land in the log file. Idempotent — calling
/// twice would only double-log, which is rarely useful.
let installed = false;
export function installGlobalErrorLogging(): void {
  if (installed) return;
  installed = true;

  window.addEventListener("error", (e) => {
    const where = e.filename
      ? ` at ${e.filename}:${e.lineno}:${e.colno}`
      : "";
    const stack = e.error instanceof Error && e.error.stack
      ? `\n${e.error.stack}`
      : "";
    void error(`Uncaught error: ${e.message}${where}${stack}`);
  });

  window.addEventListener("unhandledrejection", (e) => {
    void error(`Unhandled promise rejection: ${describe(e.reason)}`);
  });
}
