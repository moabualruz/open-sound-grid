import { For, Show } from "solid-js";
import { useGraph } from "../stores/graphStore";
import type { PwClient, PwNode } from "../types";

interface DetectedApp {
  clientId: number;
  name: string;
  nodes: PwNode[];
}

export default function AppList() {
  const { graph } = useGraph();

  const detectedApps = (): DetectedApp[] => {
    const clients = Object.values(graph.clients) as PwClient[];
    const nodes = graph.nodes;

    return clients
      .filter((c) => !c.isOsg && c.nodes.length > 0)
      .map((client) => ({
        clientId: client.id,
        name: client.name,
        nodes: client.nodes.map((nid) => nodes[String(nid)]).filter((n): n is PwNode => n != null),
      }))
      .filter((app) => app.nodes.length > 0)
      .sort((a, b) => a.name.localeCompare(b.name));
  };

  return (
    <section>
      <div class="mb-4 flex items-center gap-3">
        <h2 class="text-lg font-semibold">Running Apps</h2>
        <span class="text-text-muted text-xs">{detectedApps().length} apps</span>
      </div>

      <Show
        when={detectedApps().length > 0}
        fallback={<p class="text-text-muted text-sm">No audio apps detected.</p>}
      >
        <ul class="grid gap-2">
          <For each={detectedApps()}>
            {(app) => (
              <li class="flex items-center justify-between rounded-lg border border-border bg-surface-alt px-4 py-3">
                <div>
                  <span class="font-medium">{app.name}</span>
                  <span class="text-text-muted ml-2 text-xs">
                    {app.nodes.length} {app.nodes.length === 1 ? "node" : "nodes"}
                  </span>
                </div>
                <div class="flex gap-1">
                  <For each={app.nodes}>
                    {(node) => {
                      const hasSources = node.ports.some(([, kind]) => kind === "source");
                      const hasSinks = node.ports.some(([, kind]) => kind === "sink");
                      return (
                        <span
                          class={`rounded px-1.5 py-0.5 text-xs ${hasSources && hasSinks ? "bg-accent/20 text-accent" : hasSources ? "bg-source/20 text-source" : "bg-sink/20 text-sink"}`}
                        >
                          #{node.id}
                        </span>
                      );
                    }}
                  </For>
                </div>
              </li>
            )}
          </For>
        </ul>
      </Show>
    </section>
  );
}
