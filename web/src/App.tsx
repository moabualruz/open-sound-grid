import { createSignal, Show, lazy } from "solid-js";
import { GraphProvider } from "./stores/graphStore";
import { SessionProvider } from "./stores/sessionStore";
import { MixerSettingsProvider } from "./stores/mixerSettings";
import { LevelsProvider } from "./stores/levelsStore";
import { MonitorProvider } from "./stores/monitorStore";
import Mixer from "./components/Mixer";
import Sidebar from "./components/Sidebar";

const EqDemo = lazy(() => import("./eq/EqDemo"));
const AnalyzerPage = lazy(() => import("./spectrum/AnalyzerPage"));

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
                <div class="flex h-screen">
                  <Sidebar currentHash={route()} />
                  <div class="flex-1 min-w-0">
                    <Show
                      when={route() !== "#analyzer"}
                      fallback={<AnalyzerPage />}
                    >
                      <Mixer />
                    </Show>
                  </div>
                </div>
              </LevelsProvider>
            </MixerSettingsProvider>
          </MonitorProvider>
        </SessionProvider>
      </GraphProvider>
    </Show>
  );
}
