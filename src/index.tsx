/* @refresh reload */
import { render } from "solid-js/web";
import "./styles.css";
import App from "./App";
import { getSettings } from "./lib/ipc";
import { info, installGlobalErrorLogging } from "./lib/log";
import { applyTheme, watchSystemTheme } from "./lib/theme";

installGlobalErrorLogging();
void info(`webview loaded (route: ${window.location.hash || "#"})`);

// Apply the theme as early as possible so any flash is short. We can't go
// fully sync because settings live behind IPC; SettingsWindow's effect
// reapplies on every change after this.
void getSettings()
  .then((s) => {
    applyTheme(s.theme);
    watchSystemTheme(s.theme);
  })
  .catch(() => {
    /* swallow — fall back to default (no .dark class, system styling) */
  });

render(() => <App />, document.getElementById("root") as HTMLElement);
