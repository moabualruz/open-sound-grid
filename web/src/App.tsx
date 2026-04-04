import { createSignal, Show, lazy } from "solid-js";
import { GraphProvider } from "./stores/graphStore";
import { SessionProvider } from "./stores/sessionStore";
import { MixerSettingsProvider } from "./stores/mixerSettings";
import { LevelsProvider } from "./stores/levelsStore";
import { MonitorProvider } from "./stores/monitorStore";
import Mixer from "./components/Mixer";

const EqDemo = lazy(() => import("./eq/EqDemo"));

function useRoute() {
  const [hash, setHash] = createSignal(window.location.hash);
  window.addEventListener("hashchange", () => setHash(window.location.hash));
  return hash;
}

export default function App() {
  const route = useRoute();

  return (
    <Show when={route() !== "#eq-demo"} fallback={<EqDemo />}>
      <GraphProvider>
        <SessionProvider>
          <MonitorProvider>
            <MixerSettingsProvider>
              <LevelsProvider>
                <Mixer />
              </LevelsProvider>
            </MixerSettingsProvider>
          </MonitorProvider>
        </SessionProvider>
      </GraphProvider>
    </Show>
  );
}
