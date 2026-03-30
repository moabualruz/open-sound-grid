import { For, Show, createSignal } from "solid-js";
import type { JSX } from "solid-js";
import { useSession } from "../stores/sessionStore";
import { useGraph } from "../stores/graphStore";
import {
  Plus,
  Mic,
  Music,
  Globe,
  Bell,
  Gamepad2,
  MessageCircle,
  Speaker,
  Zap,
  Volume2,
  PenLine,
} from "lucide-solid";
import type { App, PwNode, PwDevice, AudioGraph } from "../types";

const CHANNEL_TEMPLATES = [
  { name: "Music", icon: Music, kind: "duplex" as const },
  { name: "Browser", icon: Globe, kind: "duplex" as const },
  { name: "System", icon: Bell, kind: "duplex" as const },
  { name: "Game", icon: Gamepad2, kind: "duplex" as const },
  { name: "SFX", icon: Zap, kind: "duplex" as const },
  { name: "Voice Chat", icon: MessageCircle, kind: "duplex" as const },
  { name: "Aux 1", icon: Volume2, kind: "duplex" as const },
];

function nodeHasInputPorts(node: PwNode): boolean {
  // Source ports that are NOT monitor ports — excludes output device monitors
  return node.ports.some(([, kind, isMonitor]) => kind === "source" && !isMonitor);
}

function isOsgNode(node: PwNode): boolean {
  const name = node.identifier.nodeName ?? "";
  return name.startsWith("osg.group.");
}

function isInternalNode(node: PwNode): boolean {
  const name = node.identifier.nodeName ?? "";
  return (
    name.startsWith("Midi-Bridge") ||
    name.startsWith("bluez_midi") ||
    name.includes("v4l2_") ||
    name.startsWith("libpipewire")
  );
}

/** Internal EasyEffects DSP nodes (spectrum, level meters, per-effect filters). */
function isEasyEffectsInternal(node: PwNode): boolean {
  return (node.identifier.nodeName ?? "").startsWith("ee_");
}

/** EasyEffects processed mic source — the only EasyEffects node that's an input. */
function isEasyEffectsSource(node: PwNode): boolean {
  return (node.identifier.nodeName ?? "") === "easyeffects_source";
}

function deviceNodeIds(devices: Record<string, PwDevice>): Set<number> {
  const ids = new Set<number>();
  for (const dev of Object.values(devices) as PwDevice[]) {
    for (const nodeId of dev.nodes) ids.add(nodeId);
  }
  return ids;
}

/** Resolve a PW node name (ALSA string) to a human-readable description. */
function resolveNodeDisplayName(nodeName: string, graph: AudioGraph): string {
  const node = (Object.values(graph.nodes) as PwNode[]).find(
    (n) => n.identifier.nodeName === nodeName,
  );
  return node?.identifier.nodeDescription ?? node?.identifier.nodeNick ?? nodeName;
}

function inputNodeName(node: PwNode, graph: AudioGraph): string {
  if (isEasyEffectsSource(node) && graph.defaultSourceName) {
    return `EE - ${resolveNodeDisplayName(graph.defaultSourceName, graph)}`;
  }
  if (isEasyEffectsSource(node)) return "EE - Mic";
  return node.identifier.nodeDescription ?? node.identifier.nodeNick ?? `Node ${node.id}`;
}

export default function ChannelCreator(): JSX.Element {
  const { state, send } = useSession();
  const graphState = useGraph();
  const [open, setOpen] = createSignal(false);
  const [search, setSearch] = createSignal("");

  const inputDevices = () => {
    const q = search().toLowerCase();
    const devNodes = deviceNodeIds(graphState.graph.devices);
    return (Object.values(graphState.graph.nodes) as PwNode[])
      .filter((n) => {
        if (isOsgNode(n) || isInternalNode(n) || isEasyEffectsInternal(n)) return false;
        // EasyEffects source (processed mic) is always included (its ports are monitor-flagged)
        if (isEasyEffectsSource(n)) return true;
        if (!nodeHasInputPorts(n)) return false;
        // Include: hardware device inputs
        return devNodes.has(n.id);
      })
      .filter((n) => {
        const name = inputNodeName(n, graphState.graph);
        return name.toLowerCase().includes(q) && !existingDeviceNodeIds().has(n.id);
      })
      .sort((a, b) =>
        inputNodeName(a, graphState.graph).localeCompare(inputNodeName(b, graphState.graph)),
      );
  };

  const runningApps = () => {
    const q = search().toLowerCase();
    return (Object.values(state.session.apps) as App[])
      .filter((app) => {
        const display = app.name || app.binary;
        if (!display || display.includes("(deleted)")) return false;
        return display.toLowerCase().includes(q);
      })
      .sort((a, b) => (a.name || a.binary).localeCompare(b.name || b.binary));
  };

  /** Display names of visible channels — for filtering presets only. */
  const existingChannelNames = () => {
    const names = new Set<string>();
    for (const [, ep] of state.session.endpoints) {
      if (!ep.visible) continue;
      names.add(ep.displayName);
      if (ep.customName) names.add(ep.customName);
    }
    return names;
  };

  /** PW node IDs already used as visible channels — for filtering devices. */
  const existingDeviceNodeIds = () => {
    const ids = new Set<number>();
    const nodes = Object.values(graphState.graph.nodes) as PwNode[];
    // Group node names match channel display names or custom names
    for (const [, ep] of state.session.endpoints) {
      if (!ep.visible) continue;
      const name = ep.customName ?? ep.displayName;
      const node = nodes.find(
        (n) =>
          (n.identifier.nodeDescription === name || n.identifier.nodeNick === name) &&
          !isOsgNode(n),
      );
      if (node) ids.add(node.id);
    }
    return ids;
  };

  const availableTemplates = () => {
    const q = search().toLowerCase();
    const existing = existingChannelNames();
    return CHANNEL_TEMPLATES.filter(
      (p) => !existing.has(p.name) && p.name.toLowerCase().includes(q),
    );
  };

  const [customName, setCustomName] = createSignal("");

  function create(name: string, kind: "source" | "duplex" | "sink") {
    send({ type: "createChannel", name, kind });
    setOpen(false);
    setSearch("");
    setCustomName("");
  }

  function close() {
    setOpen(false);
    setSearch("");
  }

  const hasResults = () =>
    inputDevices().length > 0 || runningApps().length > 0 || availableTemplates().length > 0;

  return (
    <div class="relative">
      <button
        onClick={() => setOpen((v) => !v)}
        aria-expanded={open()}
        aria-haspopup="listbox"
        class="flex w-full items-center justify-center gap-1.5 rounded-lg border border-dashed border-border bg-bg-elevated px-3 py-2.5 transition-colors duration-150 hover:border-accent hover:text-accent"
      >
        <Plus size={14} class="text-text-muted" />
        <span class="text-[13px] text-text-muted">Create channel</span>
      </button>

      <Show when={open()}>
        <div class="fixed inset-0 z-20" onClick={close} />

        <div
          class="absolute bottom-full left-0 z-30 mb-1 w-72 rounded-lg border border-border bg-bg-elevated shadow-xl"
          onKeyDown={(e: KeyboardEvent) => e.key === "Escape" && close()}
        >
          <div class="border-b border-border p-2">
            <input
              type="text"
              placeholder="Search devices, apps, presets..."
              value={search()}
              onInput={(e) => setSearch(e.currentTarget.value)}
              autofocus
              class="w-full rounded-md border border-border bg-bg-primary px-2.5 py-1.5 text-xs text-text-primary placeholder:text-text-muted focus:border-border-active focus:outline-none"
            />
          </div>

          <div class="max-h-72 overflow-y-auto">
            {/* Input Devices (hardware capture — mics, interfaces) */}
            <Show when={inputDevices().length > 0}>
              <div class="px-2 pt-2">
                <div class="px-3 pb-1 pt-2 text-[10px] font-semibold uppercase tracking-widest text-text-muted">
                  Input Devices
                </div>
                <For each={inputDevices()}>
                  {(node) => {
                    const name = inputNodeName(node, graphState.graph);
                    const isEE = isEasyEffectsSource(node);
                    return (
                      <button
                        onClick={() => create(name, "source")}
                        class="flex w-full items-center gap-2.5 rounded-md px-2 py-1.5 text-left transition-colors duration-150 hover:bg-bg-hover hover:text-text-primary"
                      >
                        <Mic size={16} class="shrink-0 text-text-muted" />
                        <span class="flex flex-1 items-center gap-1.5 truncate text-xs text-text-secondary">
                          <Show when={isEE}>
                            <span class="shrink-0 rounded bg-accent/20 px-1 py-0.5 text-[10px] font-bold text-accent">
                              EE
                            </span>
                          </Show>
                          {isEE ? name.replace("EE - ", "") : name}
                        </span>
                        <span class="rounded-full bg-vu-safe/15 px-1.5 py-0.5 text-[10px] text-vu-safe">
                          input
                        </span>
                      </button>
                    );
                  }}
                </For>
              </div>
            </Show>

            {/* Running Apps (backend-detected audio apps) */}
            <Show when={runningApps().length > 0}>
              <div class="px-2 pt-2">
                <div class="px-3 pb-1 pt-2 text-[10px] font-semibold uppercase tracking-widest text-text-muted">
                  Running Apps
                </div>
                <For each={runningApps()}>
                  {(app) => {
                    const display = app.name || app.binary;
                    return (
                      <button
                        onClick={() => create(display, "duplex")}
                        class="flex w-full items-center gap-2.5 rounded-md px-2 py-1.5 text-left transition-colors duration-150 hover:bg-bg-hover hover:text-text-primary"
                      >
                        <Speaker size={16} class="shrink-0 text-text-muted" />
                        <span class="flex-1 truncate text-xs text-text-secondary">{display}</span>
                        <span class="rounded-full bg-accent-secondary/15 px-1.5 py-0.5 text-[10px] text-accent-secondary">
                          live
                        </span>
                      </button>
                    );
                  }}
                </For>
              </div>
            </Show>

            {/* Channel Templates + Custom */}
            <div class="px-2 py-2">
              <div class="px-3 pb-1 pt-2 text-[10px] font-semibold uppercase tracking-widest text-text-muted">
                Add Empty Channel
              </div>
              <For each={availableTemplates()}>
                {(preset) => (
                  <button
                    onClick={() => create(preset.name, preset.kind)}
                    class="flex w-full items-center gap-2.5 rounded-md px-2 py-1.5 text-left transition-colors duration-150 hover:bg-bg-hover hover:text-text-primary"
                  >
                    <preset.icon size={16} class="shrink-0 text-text-muted" />
                    <span class="text-xs text-text-secondary">{preset.name}</span>
                  </button>
                )}
              </For>
              {/* Custom channel with user-defined name */}
              <div class="mt-1 flex items-center gap-1.5 rounded-md px-2 py-1">
                <PenLine size={16} class="shrink-0 text-text-muted" />
                <input
                  type="text"
                  placeholder="Custom name..."
                  value={customName()}
                  onInput={(e) => setCustomName(e.currentTarget.value)}
                  onKeyDown={(e) => {
                    if (e.key === "Enter" && customName().trim()) {
                      create(customName().trim(), "duplex");
                    }
                  }}
                  class="flex-1 rounded border border-border bg-bg-primary px-2 py-1 text-xs text-text-primary placeholder:text-text-muted focus:border-border-active focus:outline-none"
                />
                <button
                  onClick={() => customName().trim() && create(customName().trim(), "duplex")}
                  disabled={!customName().trim()}
                  class="rounded bg-accent px-2 py-1 text-xs text-white disabled:opacity-30"
                >
                  Add
                </button>
              </div>
            </div>

            <Show when={search() && !hasResults()}>
              <p class="px-4 py-6 text-center text-xs text-text-muted">
                No results for &ldquo;{search()}&rdquo;
              </p>
            </Show>
          </div>
        </div>
      </Show>
    </div>
  );
}
