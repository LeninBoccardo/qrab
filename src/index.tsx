/* @refresh reload */
import { render } from "solid-js/web";
import "./styles.css";
import App from "./App";
import { info, installGlobalErrorLogging } from "./lib/log";

installGlobalErrorLogging();
void info(`webview loaded (route: ${window.location.hash || "#"})`);

render(() => <App />, document.getElementById("root") as HTMLElement);
