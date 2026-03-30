import { GraphProvider } from "./stores/graphStore";
import { SessionProvider } from "./stores/sessionStore";
import Mixer from "./components/Mixer";

export default function App() {
  return (
    <GraphProvider>
      <SessionProvider>
        <Mixer />
      </SessionProvider>
    </GraphProvider>
  );
}
