import { For, Show, createSignal, createEffect, createMemo } from "solid-js";
import { createStore } from "solid-js/store";
import { useSession } from "../stores/sessionStore";
import { useGraph } from "../stores/graphStore";
import Sidebar from "./Sidebar";
import MixHeader, { getOutputDevices } from "./MixHeader";
import MixCreator from "./MixCreator";
import ChannelLabel from "./ChannelLabel";
import MatrixCell from "./MatrixCell";
import ChannelCreator from "./ChannelCreator";
import EmptyState from "./EmptyState";
import SettingsPanel from "./SettingsPanel";
import DragReorder from "./DragReorder";
import type { Endpoint, EndpointDescriptor, MixerLink } from "../types";

const MIX_COLORS: Record<string, string> = {
  Monitor: "var(--color-mix-monitor)",
  Stream: "var(--color-mix-stream)",
  VOD: "var(--color-mix-vod)",
  Chat: "var(--color-mix-chat)",
  Aux: "var(--color-mix-aux)",
};

function getMixColor(name: string): string {
  for (const [key, color] of Object.entries(MIX_COLORS)) {
    if (name.includes(key)) return color;
  }
  return "var(--color-mix-monitor)";
}

function descriptorsEqual(a: EndpointDescriptor, b: EndpointDescriptor): boolean {
  return JSON.stringify(a) === JSON.stringify(b);
}

function findEndpoint(
  endpoints: [EndpointDescriptor, Endpoint][],
  desc: EndpointDescriptor,
): Endpoint | undefined {
  return endpoints.find(([d]) => descriptorsEqual(d, desc))?.[1];
}

function findLink(
  links: MixerLink[],
  source: EndpointDescriptor,
  target: EndpointDescriptor,
): MixerLink | null {
  return (
    links.find((l) => descriptorsEqual(l.start, source) && descriptorsEqual(l.end, target)) ?? null
  );
}

export default function Mixer() {
  const { state, send } = useSession();
  const graphState = useGraph();
  const [settingsOpen, setSettingsOpen] = createSignal(false);

  function channelKind(desc: EndpointDescriptor): string | undefined {
    if (!("channel" in desc)) return undefined;
    const ch = state.session.channels[desc.channel];
    return ch?.kind;
  }

  type EndpointEntry = { desc: EndpointDescriptor; ep: Endpoint };

  const rawChannels = () =>
    state.session.endpoints
      .filter(([desc, ep]) => "channel" in desc && channelKind(desc) !== "sink" && ep.visible)
      .map(([desc, ep]) => ({ desc, ep }));

  const sinkChannels = () =>
    state.session.endpoints
      .filter(([desc, ep]) => "channel" in desc && channelKind(desc) === "sink" && ep.visible)
      .map(([desc, ep]) => ({ desc, ep }));

  const rawMixes = () => {
    const fromSinks = state.session.activeSinks
      .map((desc) => ({ desc, ep: findEndpoint(state.session.endpoints, desc) }))
      .filter((m) => m.ep?.visible !== false);
    if (fromSinks.length > 0) return fromSinks;
    return sinkChannels();
  };

  const descKey = (d: EndpointDescriptor) => JSON.stringify(d);

  // Local order — instant UI, persisted to backend on change
  const [localOrder, setLocalOrder] = createSignal<EndpointDescriptor[]>([]);
  let orderInitialized = false;

  // Sync from backend ONCE on initial load only
  createEffect(() => {
    const backendOrder = state.session.displayOrder;
    if (!orderInitialized && backendOrder.length > 0) {
      orderInitialized = true;
      setLocalOrder(backendOrder);
    }
  });

  function applyOrder(items: EndpointEntry[], order: EndpointDescriptor[]): EndpointEntry[] {
    if (order.length === 0) return items;
    const orderKeys = order.map(descKey);
    const byKey = new Map(items.map((item) => [descKey(item.desc), item]));
    const ordered: EndpointEntry[] = [];
    for (const key of orderKeys) {
      const item = byKey.get(key);
      if (item) {
        ordered.push(item);
        byKey.delete(key);
      }
    }
    for (const item of byKey.values()) ordered.push(item);
    return ordered;
  }

  const channels = createMemo(() => applyOrder(rawChannels(), localOrder()));
  const mixes = createMemo(() =>
    applyOrder(
      rawMixes().filter((m): m is { desc: EndpointDescriptor; ep: Endpoint } => m.ep != null),
      localOrder(),
    ),
  );

  function persistOrder(reordered: EndpointEntry[]) {
    const order = reordered.map((item) => item.desc);
    setLocalOrder(order);
    send({ type: "setDisplayOrder", order });
  }

  // TODO(backend): persist output device assignments to settings.toml
  const [mixOutputs, setMixOutputs] = createStore<Record<string, string | null>>({});

  // Auto-assign OS default output device to Monitor mix
  createEffect(() => {
    const allDevs = getOutputDevices(graphState.graph.devices, graphState.graph.nodes);
    if (allDevs.length === 0) return;
    const monitorMix = mixes().find((m) => m.ep?.displayName.toLowerCase().includes("monitor"));
    if (!monitorMix) return;
    const monitorKey = JSON.stringify(monitorMix.desc);

    // Use PipeWire's default.audio.sink to find the right device
    const defaultName = graphState.graph.defaultSinkName;
    if (defaultName) {
      const defaultDev = allDevs.find((d) =>
        d.nodeName.toLowerCase().includes(defaultName.toLowerCase()),
      );
      if (defaultDev && mixOutputs[monitorKey] !== defaultDev.deviceId) {
        setMixOutputs(monitorKey, defaultDev.deviceId);
        return;
      }
    }

    // Fallback: assign first device if nothing assigned yet
    if (!mixOutputs[monitorKey]) {
      setMixOutputs(monitorKey, allDevs[0].deviceId);
    }
  });

  function setMixOutput(mixKey: string, deviceId: string | null) {
    // If assigning a device that another mix uses, clear it from that mix
    if (deviceId) {
      for (const [key, val] of Object.entries(mixOutputs)) {
        if (val === deviceId && key !== mixKey) {
          setMixOutputs(key, null);
        }
      }
    }
    setMixOutputs(mixKey, deviceId);
  }

  const usedDeviceIds = () => {
    const ids = new Set<string>();
    for (const val of Object.values(mixOutputs)) {
      if (val) ids.add(val);
    }
    return ids;
  };

  return (
    <div class="flex h-screen flex-col">
      {/* Header */}
      <header class="flex items-center justify-between border-b border-border bg-bg-secondary px-5 py-2">
        <h1 class="text-sm font-semibold tracking-tight text-text-primary">Open Sound Grid</h1>
        <div class="flex items-center gap-4 text-xs text-text-secondary">
          <span class="flex items-center gap-1.5">
            <span
              class={`inline-block h-1.5 w-1.5 rounded-full ${graphState.connected ? "bg-vu-safe" : "bg-vu-hot"}`}
            />
            {graphState.connected ? "Connected" : "Disconnected"}
          </span>
          <span>{channels().length} ch</span>
          <span>{mixes().length} mix</span>
        </div>
      </header>

      <div class="flex flex-1 overflow-hidden">
        <Sidebar onOpenSettings={() => setSettingsOpen(true)} />

        {/* Main mixer area */}
        <main class="flex flex-1 flex-col overflow-auto bg-bg-primary p-4">
          <Show when={graphState.connected} fallback={<EmptyState kind="disconnected" />}>
            {/* Mix column headers */}
            <div class="mb-2 flex items-stretch gap-2">
              <div class="flex w-48 shrink-0 items-stretch justify-end">
                <MixCreator maxMixes={8} currentCount={mixes().length} />
              </div>
              <DragReorder
                items={mixes()}
                keyFn={(m) => descKey(m.desc)}
                onReorder={persistOrder}
                direction="horizontal"
              >
                {(mix, _idx, dragHandle) => {
                  const mixKey = descKey(mix.desc);
                  return (
                    <div class="flex flex-col">
                      <MixHeader
                        descriptor={mix.desc}
                        endpoint={mix.ep}
                        color={getMixColor(mix.ep.displayName)}
                        outputDevice={mixOutputs[mixKey] ?? null}
                        usedDeviceIds={usedDeviceIds()}
                        onRemove={() =>
                          send({
                            type: "setEndpointVisible",
                            endpoint: mix.desc,
                            visible: false,
                          })
                        }
                        onSelectOutput={(deviceId) => setMixOutput(mixKey, deviceId)}
                        dragHandle={dragHandle}
                      />
                    </div>
                  );
                }}
              </DragReorder>
            </div>

            {/* Matrix rows */}
            <div class="flex flex-1 flex-col gap-1.5">
              <DragReorder
                items={channels()}
                keyFn={(ch) => descKey(ch.desc)}
                onReorder={persistOrder}
              >
                {(ch, _idx, dragHandle) => (
                  <div class="flex items-stretch gap-2">
                    <ChannelLabel descriptor={ch.desc} endpoint={ch.ep} dragHandle={dragHandle} />
                    <For each={mixes()}>
                      {({ desc: sinkDesc, ep: sinkEp }) => (
                        <MatrixCell
                          link={findLink(state.session.links, ch.desc, sinkDesc)}
                          sourceEndpoint={ch.ep}
                          sourceDescriptor={ch.desc}
                          sinkDescriptor={sinkDesc}
                          mixColor={getMixColor(sinkEp?.displayName ?? "")}
                        />
                      )}
                    </For>
                  </div>
                )}
              </DragReorder>

              {/* Create channel */}
              <div class="flex gap-2">
                <div class="w-48 shrink-0">
                  <ChannelCreator />
                </div>
              </div>

              {/* Empty states */}
              <Show when={channels().length === 0 && mixes().length > 0}>
                <EmptyState kind="no-channels" />
              </Show>
              <Show when={mixes().length === 0}>
                <EmptyState kind="no-mixes" />
              </Show>
            </div>
          </Show>
        </main>
      </div>

      {/* Status bar */}
      <footer class="flex items-center justify-between border-t border-border bg-bg-secondary px-5 py-1 text-[11px] text-text-muted">
        <span class="flex items-center gap-1.5">
          <span
            class={`inline-block h-1.5 w-1.5 rounded-full ${state.connected ? "bg-vu-safe" : "bg-vu-hot"}`}
          />
          {state.connected ? "Connected to PipeWire" : "Disconnected"}
        </span>
        <div class="flex items-center gap-4">
          <span>{channels().length} channels</span>
          <span>{mixes().length} mixes</span>
          <span>{Object.keys(graphState.graph.nodes).length} nodes</span>
          <span>v0.1.0</span>
        </div>
      </footer>

      <SettingsPanel open={settingsOpen()} onClose={() => setSettingsOpen(false)} />
    </div>
  );
}
