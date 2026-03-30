import { GraphProvider } from "./stores/graphStore";
import { SessionProvider } from "./stores/sessionStore";
import { MixerSettingsProvider } from "./stores/mixerSettings";
import Mixer from "./components/Mixer";

export default function App() {
  return (
    <GraphProvider>
      <SessionProvider>
        <MixerSettingsProvider>
          <Mixer />
        </MixerSettingsProvider>
      </SessionProvider>
    </GraphProvider>
  );
}
