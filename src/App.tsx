import {
  Component,
  createSignal,
  Match,
  onCleanup,
  onMount,
  Switch,
} from "solid-js";
import { HistoryWindow } from "./windows/HistoryWindow";
import { RegionSelectWindow } from "./windows/RegionSelectWindow";
import { ResultsWindow } from "./windows/ResultsWindow";
import { SettingsWindow } from "./windows/SettingsWindow";

function currentRoute(): string {
  return window.location.hash.slice(1) || "results";
}

const App: Component = () => {
  const [route, setRoute] = createSignal(currentRoute());

  onMount(() => {
    const handler = (): void => {
      setRoute(currentRoute());
    };
    window.addEventListener("hashchange", handler);
    onCleanup(() => window.removeEventListener("hashchange", handler));
  });

  return (
    <Switch fallback={<ResultsWindow />}>
      <Match when={route() === "results"}>
        <ResultsWindow />
      </Match>
      <Match when={route() === "region"}>
        <RegionSelectWindow />
      </Match>
      <Match when={route() === "history"}>
        <HistoryWindow />
      </Match>
      <Match when={route() === "settings"}>
        <SettingsWindow />
      </Match>
    </Switch>
  );
};

export default App;
