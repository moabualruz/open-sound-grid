import { GraphProvider } from "./stores/graphStore";
import NodeList from "./components/NodeList";

export default function App() {
  return (
    <GraphProvider>
      <div class="min-h-screen p-6">
        <header class="mb-8">
          <h1 class="text-2xl font-bold">Open Sound Grid</h1>
          <p class="text-text-muted text-sm">PipeWire audio routing</p>
        </header>
        <main>
          <NodeList />
        </main>
      </div>
    </GraphProvider>
  );
}
