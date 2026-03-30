import { For, Show } from "solid-js";
import { useGraph } from "../stores/graphStore";
import { useSession } from "../stores/sessionStore";
import type { PwClient, PwNode } from "../types";

interface DetectedApp {
  clientId: number;
  name: string;
  nodes: PwNode[];
}

export default function AppList() {
  const { graph } = useGraph();
  const { send } = useSession();

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

  function addNodeAsEndpoint(node: PwNode) {
    const hasSources = node.ports.some(([, kind]) => kind === "source");
    send({
      type: "createChannel",
      name: node.identifier.nodeDescription ?? node.identifier.nodeNick ?? `Node ${node.id}`,
      kind: hasSources ? "source" : "sink",
    });
  }

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
              <li class="rounded-lg border border-border bg-surface-alt px-4 py-3">
                <div class="flex items-center justify-between">
                  <div>
                    <span class="font-medium">{app.name}</span>
                    <span class="text-text-muted ml-2 text-xs">
                      {app.nodes.length} {app.nodes.length === 1 ? "node" : "nodes"}
                    </span>
                  </div>
                </div>
                <div class="mt-2 flex flex-wrap gap-1">
                  <For each={app.nodes}>
                    {(node) => {
                      const hasSources = node.ports.some(([, kind]) => kind === "source");
                      const hasSinks = node.ports.some(([, kind]) => kind === "sink");
                      const label =
                        node.identifier.nodeDescription ??
                        node.identifier.nodeNick ??
                        `#${node.id}`;
                      return (
                        <button
                          onClick={() => addNodeAsEndpoint(node)}
                          title={`Add "${label}" as channel`}
                          class={`rounded px-2 py-1 text-xs transition-colors hover:brightness-125 ${hasSources && hasSinks ? "bg-accent/20 text-accent" : hasSources ? "bg-source/20 text-source" : "bg-sink/20 text-sink"}`}
                        >
                          {label}
                        </button>
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
