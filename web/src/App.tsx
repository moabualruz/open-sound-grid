import { GraphProvider } from "./stores/graphStore";
import { SessionProvider } from "./stores/sessionStore";
import { MixerSettingsProvider } from "./stores/mixerSettings";
import { LevelsProvider } from "./stores/levelsStore";
import Mixer from "./components/Mixer";

export default function App() {
  return (
    <GraphProvider>
      <SessionProvider>
        <MixerSettingsProvider>
          <LevelsProvider>
            <Mixer />
          </LevelsProvider>
        </MixerSettingsProvider>
      </SessionProvider>
    </GraphProvider>
  );
}
