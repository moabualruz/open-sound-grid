import { For, Show } from "solid-js";
import { useGraph } from "../stores/graphStore";
import type { PwNode } from "../types";

function getNodeDisplayName(node: PwNode): string {
  const id = node.identifier;
  return id.nodeDescription ?? id.nodeNick ?? id.nodeName ?? `Node ${node.id}`;
}

function getPortSummary(node: PwNode): string {
  const sources = node.ports.filter(([, kind]) => kind === "source").length;
  const sinks = node.ports.filter(([, kind]) => kind === "sink").length;
  const parts: string[] = [];
  if (sources > 0) parts.push(`${sources} out`);
  if (sinks > 0) parts.push(`${sinks} in`);
  return parts.join(", ") || "no ports";
}

export default function NodeList() {
  const { graph, connected } = useGraph();

  const sortedNodes = () => Object.values(graph.nodes).sort((a, b) => a.id - b.id);

  return (
    <section>
      <div class="mb-4 flex items-center gap-3">
        <h2 class="text-lg font-semibold">PipeWire Nodes</h2>
        <span
          class={`inline-block h-2 w-2 rounded-full ${connected ? "bg-source" : "bg-sink"}`}
          title={connected ? "Connected" : "Disconnected"}
        />
        <span class="text-text-muted text-xs">{Object.keys(graph.nodes).length} nodes</span>
      </div>

      <Show
        when={Object.keys(graph.nodes).length > 0}
        fallback={
          <p class="text-text-muted text-sm">
            {connected ? "No nodes detected" : "Connecting to server..."}
          </p>
        }
      >
        <ul class="grid gap-2">
          <For each={sortedNodes()}>
            {(node) => (
              <li class="flex items-center justify-between rounded-lg border border-border bg-surface-alt px-4 py-3">
                <div>
                  <span class="font-medium">{getNodeDisplayName(node)}</span>
                  <span class="text-text-muted ml-2 text-xs font-mono">#{node.id}</span>
                </div>
                <div class="flex items-center gap-3 text-xs">
                  <span class="text-text-muted">{getPortSummary(node)}</span>
                  <Show when={node.mute}>
                    <span class="rounded bg-sink/20 px-1.5 py-0.5 text-sink">muted</span>
                  </Show>
                </div>
              </li>
            )}
          </For>
        </ul>
      </Show>
    </section>
  );
}
