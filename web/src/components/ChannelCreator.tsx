import { For, Show, createSignal } from "solid-js";
import { useSession } from "../stores/sessionStore";
import { useGraph } from "../stores/graphStore";
import type { PwNode } from "../types";

const PRESETS = [
  { name: "Music", icon: "🎵", kind: "duplex" as const },
  { name: "Browser", icon: "🌐", kind: "duplex" as const },
  { name: "System", icon: "🔔", kind: "duplex" as const },
  { name: "Game", icon: "🎮", kind: "duplex" as const },
  { name: "Voice Chat", icon: "💬", kind: "duplex" as const },
  { name: "SFX", icon: "🔊", kind: "duplex" as const },
  { name: "Aux 1", icon: "🎛️", kind: "duplex" as const },
];

/** Get a human-friendly name for a PipeWire node. */
function nodeName(node: PwNode): string {
  return node.identifier.nodeDescription ?? node.identifier.nodeNick ?? `Node ${node.id}`;
}

/** Filter out internal/system nodes, keep user-facing audio apps. */
function isUserApp(node: PwNode): boolean {
  const name = node.identifier.nodeName ?? "";
  // Skip internal PipeWire/system nodes
  if (name.startsWith("Midi-Bridge") || name.startsWith("bluez_midi")) return false;
  if (name.includes("v4l2_")) return false; // video devices
  // Must have audio ports
  return node.ports.length > 0;
}

export default function ChannelCreator() {
  const { send } = useSession();
  const { graph } = useGraph();
  const [open, setOpen] = createSignal(false);
  const [search, setSearch] = createSignal("");

  const audioApps = () => {
    const q = search().toLowerCase();
    return (Object.values(graph.nodes) as PwNode[])
      .filter(isUserApp)
      .filter((n) => nodeName(n).toLowerCase().includes(q))
      .sort((a, b) => nodeName(a).localeCompare(nodeName(b)));
  };

  const filteredPresets = () => {
    const q = search().toLowerCase();
    return PRESETS.filter((p) => p.name.toLowerCase().includes(q));
  };

  function create(name: string, kind: "source" | "duplex" | "sink") {
    send({ type: "createChannel", name, kind });
    setOpen(false);
    setSearch("");
  }

  return (
    <div class="relative">
      <button
        onClick={() => setOpen(!open())}
        class="flex w-full items-center justify-center gap-1.5 rounded-lg border border-dashed border-border bg-bg-secondary px-3 py-2.5 text-[13px] text-text-muted transition-colors hover:border-accent hover:text-accent"
      >
        <span class="text-base">+</span> Create channel
      </button>

      <Show when={open()}>
        <div class="fixed inset-0 z-20" onClick={() => setOpen(false)} />

        <div class="absolute bottom-full left-0 z-30 mb-1 w-72 rounded-lg border border-border bg-bg-elevated shadow-xl">
          <div class="border-b border-border p-2">
            <input
              type="text"
              placeholder="Search..."
              value={search()}
              onInput={(e) => setSearch(e.currentTarget.value)}
              autofocus
              class="w-full rounded-md border border-border bg-bg-primary px-2.5 py-1.5 text-xs text-text-primary placeholder:text-text-muted focus:border-border-active focus:outline-none"
            />
          </div>

          <div class="max-h-72 overflow-y-auto">
            <Show when={audioApps().length > 0}>
              <div class="px-2 pt-2">
                <div class="px-1 pb-1 text-[10px] font-semibold uppercase tracking-widest text-text-muted">
                  Audio Sources
                </div>
                <For each={audioApps()}>
                  {(node) => {
                    const name = nodeName(node);
                    const hasPorts = node.ports.length > 0;
                    return (
                      <button
                        onClick={() => create(name, "duplex")}
                        class="flex w-full items-center gap-2.5 rounded-md px-2 py-1.5 text-left text-xs text-text-secondary hover:bg-bg-hover hover:text-text-primary"
                      >
                        <span class="text-base">🔈</span>
                        <span class="flex-1 truncate">{name}</span>
                        <Show when={hasPorts}>
                          <span class="rounded-full bg-vu-safe/20 px-1.5 py-0.5 text-[10px] text-vu-safe">
                            live
                          </span>
                        </Show>
                      </button>
                    );
                  }}
                </For>
              </div>
            </Show>

            <div class="px-2 py-2">
              <div class="px-1 pb-1 text-[10px] font-semibold uppercase tracking-widest text-text-muted">
                Add Empty Channel
              </div>
              <For each={filteredPresets()}>
                {(preset) => (
                  <button
                    onClick={() => create(preset.name, preset.kind)}
                    class="flex w-full items-center gap-2.5 rounded-md px-2 py-1.5 text-left text-xs text-text-secondary hover:bg-bg-hover hover:text-text-primary"
                  >
                    <span class="text-base">{preset.icon}</span>
                    <span>{preset.name}</span>
                  </button>
                )}
              </For>
            </div>
          </div>
        </div>
      </Show>
    </div>
  );
}
