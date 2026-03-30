import { GraphProvider } from "./stores/graphStore";
import { SessionProvider } from "./stores/sessionStore";
import NodeList from "./components/NodeList";
import ChannelList from "./components/ChannelList";

export default function App() {
  return (
    <GraphProvider>
      <SessionProvider>
        <div class="min-h-screen p-6">
          <header class="mb-8">
            <h1 class="text-2xl font-bold">Open Sound Grid</h1>
            <p class="text-text-muted text-sm">PipeWire audio routing</p>
          </header>
          <main class="grid gap-8 lg:grid-cols-2">
            <ChannelList />
            <NodeList />
          </main>
        </div>
      </SessionProvider>
    </GraphProvider>
  );
}
