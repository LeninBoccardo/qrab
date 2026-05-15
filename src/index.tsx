/* @refresh reload */
import { render } from "solid-js/web";
import "./styles.css";
import App from "./App";
import { loadSettings } from "./lib/state";
import { info, installGlobalErrorLogging } from "./lib/log";

installGlobalErrorLogging();
void info(`webview loaded (route: ${window.location.hash || "#"})`);

// Load settings (theme applied as a side effect). Fire-and-forget — every
// consumer treats `settings()` being null as "not loaded yet" and falls
// back gracefully.
void loadSettings();

render(() => <App />, document.getElementById("root") as HTMLElement);
