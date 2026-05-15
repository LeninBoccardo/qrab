import {
  Component,
  createSignal,
  Match,
  onCleanup,
  onMount,
  Switch,
} from "solid-js";
import { ResultsWindow } from "./windows/ResultsWindow";

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
      {/* Future routes: #region (C7), #history (C14), #settings (C19) */}
    </Switch>
  );
};

export default App;
