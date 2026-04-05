import { createSignal, Show, lazy, onCleanup } from "solid-js";
import { GraphProvider, useGraph } from "./stores/graphStore";
import { SessionProvider } from "./stores/sessionStore";
import { MixerSettingsProvider } from "./stores/mixerSettings";
import { LevelsProvider } from "./stores/levelsStore";
import { MonitorProvider } from "./stores/monitorStore";
import Mixer from "./components/Mixer";
import Sidebar from "./components/Sidebar";

function useCompactMode() {
  const [compact, setCompact] = createSignal(false);
  const handler = (e: Event) => setCompact((e as CustomEvent<boolean>).detail);
  window.addEventListener("osg:compact-mode", handler);
  onCleanup(() => window.removeEventListener("osg:compact-mode", handler));
  return compact;
}

const EqDemo = lazy(() => import("./eq/EqDemo"));
const AnalyzerPage = lazy(() => import("./spectrum/AnalyzerPage"));

function useRoute() {
  const [hash, setHash] = createSignal(window.location.hash);
  const handler = () => setHash(window.location.hash);
  window.addEventListener("hashchange", handler);
  onCleanup(() => window.removeEventListener("hashchange", handler));
  return hash;
}

/** Inner shell rendered inside providers so hooks (useGraph) are available. */
function AppShell(shellProps: { route: () => string }) {
  const graphState = useGraph();
  const compact = useCompactMode();

  function openSettings() {
    window.dispatchEvent(new CustomEvent("osg:open-settings"));
  }

  return (
    <div class="flex h-screen">
      <div style={{ display: compact() ? "none" : undefined }}>
        <Sidebar
          currentHash={shellProps.route()}
          devices={graphState.graph.devices}
          onOpenSettings={openSettings}
        />
      </div>
      <div class="flex-1 min-w-0">
        <Show when={shellProps.route() !== "#analyzer"} fallback={<AnalyzerPage />}>
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
                <AppShell route={route} />
              </LevelsProvider>
            </MixerSettingsProvider>
          </MonitorProvider>
        </SessionProvider>
      </GraphProvider>
    </Show>
  );
}
