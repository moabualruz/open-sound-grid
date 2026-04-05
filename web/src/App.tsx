import { createSignal, Show, lazy } from "solid-js";
import { GraphProvider, useGraph } from "./stores/graphStore";
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

/** Inner shell rendered inside providers so hooks (useGraph) are available. */
function AppShell() {
  const route = useRoute();
  const graphState = useGraph();

  function openSettings() {
    window.dispatchEvent(new CustomEvent("osg:open-settings"));
  }

  return (
    <div class="flex h-screen">
      <Sidebar
        currentHash={route()}
        devices={graphState.graph.devices}
        onOpenSettings={openSettings}
      />
      <div class="flex-1 min-w-0">
        <Show when={route() !== "#analyzer"} fallback={<AnalyzerPage />}>
          <Mixer />
        </Show>
      </div>
    </div>
  );
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
                <AppShell />
              </LevelsProvider>
            </MixerSettingsProvider>
          </MonitorProvider>
        </SessionProvider>
      </GraphProvider>
    </Show>
  );
}
