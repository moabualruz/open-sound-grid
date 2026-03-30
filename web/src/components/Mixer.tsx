import { For, Show, createSignal } from "solid-js";
import { useSession } from "../stores/sessionStore";
import { useGraph } from "../stores/graphStore";
import MatrixCell from "./MatrixCell";
import type { Endpoint, PwClient } from "../types";

/** Default mix names for new sessions. */
const DEFAULT_MIXES = ["Monitor", "Stream"];

const MIX_COLORS: Record<string, string> = {
  Monitor: "var(--color-mix-monitor)",
  Stream: "var(--color-mix-stream)",
  VOD: "var(--color-mix-vod)",
  Chat: "var(--color-mix-chat)",
  Aux: "var(--color-mix-aux)",
};

function getMixColor(name: string): string {
  return MIX_COLORS[name] ?? "var(--color-accent-secondary)";
}

export default function Mixer() {
  const { state, send } = useSession();
  const { graph, connected } = useGraph();
  const [newChannelName, setNewChannelName] = createSignal("");
  const [mixes] = createSignal(DEFAULT_MIXES);

  const channels = () => {
    const eps = state.session.endpoints;
    return eps.filter(([desc]) => "channel" in desc).map(([desc, ep]) => ({ desc, ep }));
  };

  const detectedApps = () => {
    const clients = Object.values(graph.clients) as PwClient[];
    return clients
      .filter((c) => !c.isOsg && c.nodes.length > 0 && c.name)
      .sort((a, b) => a.name.localeCompare(b.name));
  };

  function createChannel() {
    const name = newChannelName().trim();
    if (name) {
      send({ type: "createChannel", name, kind: "duplex" });
      setNewChannelName("");
    }
  }

  function isMuted(ep: Endpoint): boolean {
    const s = ep.volumeLockedMuted;
    return s === "mutedLocked" || s === "mutedUnlocked" || s === "muteMixed";
  }

  return (
    <div class="flex h-screen flex-col">
      {/* Header */}
      <header class="flex items-center justify-between border-b border-border bg-bg-secondary px-4 py-2">
        <div class="flex items-center gap-3">
          <h1 class="text-base font-semibold">Open Sound Grid</h1>
        </div>
        <div class="flex items-center gap-4 text-xs text-text-secondary">
          <span class="flex items-center gap-1.5">
            <span
              class={`inline-block h-2 w-2 rounded-full ${connected ? "bg-vu-safe" : "bg-vu-hot"}`}
            />
            {connected ? "Connected" : "Disconnected"}
          </span>
          <span>{channels().length} channels</span>
          <span>{Object.keys(graph.nodes).length} nodes</span>
        </div>
      </header>

      <div class="flex flex-1 overflow-hidden">
        {/* Sidebar */}
        <aside class="flex w-52 shrink-0 flex-col border-r border-border bg-bg-secondary">
          <div class="border-b border-border px-3 py-2">
            <h2 class="text-xs font-semibold uppercase tracking-wide text-text-muted">Devices</h2>
          </div>
          <div class="flex-1 overflow-y-auto px-2 py-2">
            <For each={detectedApps()}>
              {(client) => (
                <button
                  onClick={() => send({ type: "createChannel", name: client.name, kind: "duplex" })}
                  class="mb-1 flex w-full items-center gap-2 rounded px-2 py-1.5 text-left text-sm text-text-secondary hover:bg-bg-hover hover:text-text-primary"
                >
                  <span class="truncate">{client.name}</span>
                  <span class="ml-auto text-xs text-text-muted">{client.nodes.length}</span>
                </button>
              )}
            </For>
          </div>
          <div class="border-t border-border px-3 py-2">
            <h2 class="text-xs font-semibold uppercase tracking-wide text-text-muted">
              Create Channel
            </h2>
            <div class="mt-2 flex gap-1">
              <input
                type="text"
                placeholder="Name..."
                value={newChannelName()}
                onInput={(e) => setNewChannelName(e.currentTarget.value)}
                onKeyDown={(e) => e.key === "Enter" && createChannel()}
                class="w-full rounded border border-border bg-bg-primary px-2 py-1 text-xs text-text-primary placeholder:text-text-muted focus:border-border-active focus:outline-none"
              />
            </div>
          </div>
        </aside>

        {/* Main mixer area */}
        <main class="flex flex-1 flex-col overflow-auto bg-bg-primary">
          {/* Mix column headers */}
          <div class="sticky top-0 z-10 flex border-b border-border bg-bg-secondary">
            <div class="w-48 shrink-0 border-r border-border px-3 py-2">
              <span class="text-xs font-semibold uppercase tracking-wide text-text-muted">
                Channels
              </span>
            </div>
            <For each={mixes()}>
              {(mix) => (
                <div
                  class="flex min-w-40 flex-1 flex-col border-r border-border px-3 py-2"
                  style={{ "border-top": `3px solid ${getMixColor(mix)}` }}
                >
                  <span class="text-sm font-semibold" style={{ color: getMixColor(mix) }}>
                    {mix}
                  </span>
                  <span class="text-xs text-text-muted">output</span>
                </div>
              )}
            </For>
          </div>

          {/* Matrix rows */}
          <Show
            when={channels().length > 0}
            fallback={
              <div class="flex flex-1 items-center justify-center text-text-muted">
                <div class="text-center">
                  <p class="mb-2 text-sm">No channels in mixer</p>
                  <p class="text-xs">Click a device in the sidebar or create a channel to start</p>
                </div>
              </div>
            }
          >
            <div class="flex-1">
              <For each={channels()}>
                {({ desc, ep }) => (
                  <div class="flex border-b border-border hover:bg-bg-hover/30">
                    {/* Channel label */}
                    <div class="flex w-48 shrink-0 items-center gap-2 border-r border-border px-3 py-2">
                      <button
                        onClick={() =>
                          send({
                            type: "setMute",
                            endpoint: desc,
                            muted: !isMuted(ep),
                          })
                        }
                        class={`text-xs ${isMuted(ep) ? "text-vu-hot" : "text-text-secondary hover:text-text-primary"}`}
                        title={isMuted(ep) ? "Unmute" : "Mute"}
                      >
                        {isMuted(ep) ? "🔇" : "🔊"}
                      </button>
                      <span class="flex-1 truncate text-sm font-medium">
                        {ep.customName ?? ep.displayName}
                      </span>
                      <button
                        onClick={() => send({ type: "removeEndpoint", endpoint: desc })}
                        class="text-text-muted hover:text-vu-hot"
                        title="Remove channel"
                      >
                        ×
                      </button>
                    </div>

                    {/* Matrix cells — one per mix */}
                    <For each={mixes()}>
                      {(mix) => (
                        <MatrixCell
                          endpoint={ep}
                          descriptor={desc}
                          mixName={mix}
                          mixColor={getMixColor(mix)}
                        />
                      )}
                    </For>
                  </div>
                )}
              </For>
            </div>
          </Show>
        </main>
      </div>

      {/* Status bar */}
      <footer class="flex items-center justify-between border-t border-border bg-bg-secondary px-4 py-1 text-xs text-text-muted">
        <span class="flex items-center gap-1.5">
          <span
            class={`inline-block h-1.5 w-1.5 rounded-full ${state.connected ? "bg-vu-safe" : "bg-vu-hot"}`}
          />
          {state.connected ? "Session connected" : "Session disconnected"}
        </span>
        <span>Open Sound Grid v0.1.0</span>
      </footer>
    </div>
  );
}
