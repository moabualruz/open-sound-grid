import { For, Show, createSignal } from "solid-js";
import { useSession } from "../stores/sessionStore";
import { useGraph } from "../stores/graphStore";
import MatrixCell from "./MatrixCell";
import ChannelCreator from "./ChannelCreator";
import type { Endpoint, PwClient } from "../types";

const DEFAULT_MIXES = ["Monitor", "Stream"];

const MIX_COLORS: Record<string, string> = {
  Monitor: "var(--color-mix-monitor)",
  Stream: "var(--color-mix-stream)",
  VOD: "var(--color-mix-vod)",
  Chat: "var(--color-mix-chat)",
  Aux: "var(--color-mix-aux)",
};

const MIX_ICONS: Record<string, string> = {
  Monitor: "🎧",
  Stream: "📡",
  VOD: "🎬",
  Chat: "💬",
  Aux: "🔈",
};

function getMixColor(name: string): string {
  return MIX_COLORS[name] ?? "var(--color-accent-secondary)";
}

function isMuted(ep: Endpoint): boolean {
  const s = ep.volumeLockedMuted;
  return s === "mutedLocked" || s === "mutedUnlocked" || s === "muteMixed";
}

export default function Mixer() {
  const { state, send } = useSession();
  const { graph, connected } = useGraph();
  const [mixes, setMixes] = createSignal(DEFAULT_MIXES);

  function addMix() {
    const names = ["Monitor", "Stream", "VOD", "Chat", "Aux"];
    const existing = mixes();
    const next = names.find((n) => !existing.includes(n)) ?? `Mix ${existing.length + 1}`;
    setMixes([...existing, next]);
  }

  function removeMix(name: string) {
    setMixes(mixes().filter((m) => m !== name));
  }

  const channels = () =>
    state.session.endpoints
      .filter(([desc]) => "channel" in desc)
      .map(([desc, ep]) => ({ desc, ep }));

  const detectedApps = (): PwClient[] => {
    const clients = Object.values(graph.clients) as PwClient[];
    return clients
      .filter((c) => !c.isOsg && c.nodes.length > 0 && c.name)
      .sort((a, b) => a.name.localeCompare(b.name));
  };

  return (
    <div class="flex h-screen flex-col">
      {/* Header */}
      <header class="flex items-center justify-between border-b border-border bg-bg-secondary px-5 py-2.5">
        <h1 class="text-base font-semibold tracking-tight">Open Sound Grid</h1>
        <div class="flex items-center gap-5 text-xs text-text-secondary">
          <span class="flex items-center gap-1.5">
            <span
              class={`inline-block h-2 w-2 rounded-full ${connected ? "bg-vu-safe" : "bg-vu-hot"}`}
            />
            {connected ? "Connected" : "Disconnected"}
          </span>
          <span>{channels().length} ch</span>
        </div>
      </header>

      <div class="flex flex-1 overflow-hidden">
        {/* Sidebar */}
        <aside class="flex w-52 shrink-0 flex-col border-r border-border bg-bg-secondary">
          <div class="border-b border-border px-4 py-2.5">
            <h2 class="text-[11px] font-semibold uppercase tracking-widest text-text-muted">
              Devices
            </h2>
          </div>
          <div class="flex-1 overflow-y-auto px-3 py-2">
            <For each={detectedApps()}>
              {(client) => (
                <button
                  onClick={() => send({ type: "createChannel", name: client.name, kind: "duplex" })}
                  class="mb-0.5 flex w-full items-center gap-2 rounded-md px-2.5 py-2 text-left text-[13px] text-text-secondary transition-colors hover:bg-bg-hover hover:text-text-primary"
                >
                  <span class="truncate">{client.name}</span>
                  <span class="ml-auto rounded-full bg-bg-hover px-1.5 py-0.5 text-[10px] text-text-muted">
                    {client.nodes.length}
                  </span>
                </button>
              )}
            </For>
          </div>
          <div class="border-t border-border p-3">
            <div class="text-[11px] font-semibold uppercase tracking-widest text-text-muted">
              Mixes & Effects
            </div>
            <div class="mt-2 space-y-0.5">
              <For each={mixes()}>
                {(mix) => (
                  <div
                    class="flex items-center gap-2 rounded-md border-l-2 px-2.5 py-1.5 text-[13px] text-text-secondary"
                    style={{ "border-left-color": getMixColor(mix) }}
                  >
                    <span>{MIX_ICONS[mix] ?? "🔈"}</span>
                    <span>{mix}</span>
                  </div>
                )}
              </For>
            </div>
          </div>
        </aside>

        {/* Main mixer area */}
        <main class="flex flex-1 flex-col overflow-auto bg-bg-primary p-4">
          {/* Mix column headers */}
          <div class="mb-3 flex gap-3">
            {/* Channel label spacer */}
            <div class="w-52 shrink-0" />
            <For each={mixes()}>
              {(mix) => (
                <div
                  class="flex min-w-44 flex-1 items-center gap-2 rounded-t-lg border-t-[3px] bg-bg-secondary px-4 py-2.5"
                  style={{ "border-top-color": getMixColor(mix) }}
                >
                  <span class="text-lg">{MIX_ICONS[mix] ?? "🔈"}</span>
                  <div>
                    <div class="text-sm font-semibold" style={{ color: getMixColor(mix) }}>
                      {mix}
                    </div>
                    <div class="text-[11px] text-text-muted">output</div>
                  </div>
                  <button
                    onClick={() => removeMix(mix)}
                    class="ml-auto text-text-muted transition-colors hover:text-vu-hot"
                    title={`Remove ${mix}`}
                  >
                    ×
                  </button>
                </div>
              )}
            </For>
            {/* Add mix button */}
            <Show when={mixes().length < 5}>
              <button
                onClick={addMix}
                class="flex min-w-12 items-center justify-center rounded-t-lg border-t-[3px] border-t-border bg-bg-secondary px-3 py-2.5 text-text-muted transition-colors hover:border-t-accent hover:text-accent"
                title="Add mix"
              >
                +
              </button>
            </Show>
          </div>

          {/* Matrix rows */}
          <div class="flex flex-1 flex-col gap-2">
            <For each={channels()}>
              {({ desc, ep }) => (
                <div class="flex gap-3">
                  {/* Channel label */}
                  <div class="flex w-52 shrink-0 items-center gap-2 rounded-lg border border-border bg-bg-secondary px-3 py-2.5">
                    <button
                      onClick={() => send({ type: "setMute", endpoint: desc, muted: !isMuted(ep) })}
                      class={`shrink-0 text-sm transition-colors ${isMuted(ep) ? "text-vu-hot" : "text-text-muted hover:text-text-primary"}`}
                      title={isMuted(ep) ? "Unmute all" : "Mute all"}
                    >
                      {isMuted(ep) ? "🔇" : "🔊"}
                    </button>
                    <span class="flex-1 truncate text-[13px] font-medium">
                      {ep.customName ?? ep.displayName}
                    </span>
                    <button
                      onClick={() => send({ type: "removeEndpoint", endpoint: desc })}
                      class="shrink-0 text-text-muted transition-colors hover:text-vu-hot"
                      title="Remove channel"
                    >
                      ×
                    </button>
                  </div>

                  {/* Matrix cells */}
                  <For each={mixes()}>
                    {(mix) => (
                      <MatrixCell endpoint={ep} descriptor={desc} mixColor={getMixColor(mix)} />
                    )}
                  </For>
                </div>
              )}
            </For>

            {/* Create channel button */}
            <div class="flex gap-3">
              <div class="w-52 shrink-0">
                <ChannelCreator apps={detectedApps()} />
              </div>
            </div>

            {/* Empty state */}
            <Show when={channels().length === 0}>
              <div class="flex flex-1 items-center justify-center">
                <div class="text-center text-text-muted">
                  <p class="mb-1 text-sm">No channels in mixer</p>
                  <p class="text-xs">Click "+ Create channel" or a device in the sidebar</p>
                </div>
              </div>
            </Show>
          </div>
        </main>
      </div>

      {/* Status bar */}
      <footer class="flex items-center justify-between border-t border-border bg-bg-secondary px-5 py-1.5 text-[11px] text-text-muted">
        <span class="flex items-center gap-1.5">
          <span
            class={`inline-block h-1.5 w-1.5 rounded-full ${state.connected ? "bg-vu-safe" : "bg-vu-hot"}`}
          />
          {state.connected ? "Connected to PipeWire" : "Disconnected"}
        </span>
        <div class="flex items-center gap-4">
          <span>{channels().length} channels</span>
          <span>{Object.keys(graph.nodes).length} nodes</span>
          <span>v0.1.0</span>
        </div>
      </footer>
    </div>
  );
}
