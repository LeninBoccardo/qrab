/* @refresh reload */
import { render } from "solid-js/web";
import "./styles.css";
import App from "./App";
import {
  loadSettings,
  loadSupportedImageExtensions,
  maybeRunAutoUpdateCheck,
} from "./lib/state";
import { info, installGlobalErrorLogging } from "./lib/log";

installGlobalErrorLogging();
void info(`webview loaded (route: ${window.location.hash || "#"})`);

// Load settings (theme applied as a side effect), then — if the user has
// opted in via the Config toggle — run a single update check. Both are
// fire-and-forget; consumers treat `settings()` being null as "not
// loaded yet" and fall back gracefully.
void loadSettings().then(() => maybeRunAutoUpdateCheck());

// Prime the cached image-extension list so file-picker filters and
// drag-drop checks resolve synchronously after the first paint.
void loadSupportedImageExtensions();

render(() => <App />, document.getElementById("root") as HTMLElement);
