import { For, Show, createSignal, createEffect, createMemo } from "solid-js";
import { createStore } from "solid-js/store";
import { useSession } from "../stores/sessionStore";
import { useGraph } from "../stores/graphStore";
import MixHeader, { getOutputDevices } from "./MixHeader";
import MixCreator from "./MixCreator";
import ChannelLabel from "./ChannelLabel";
import MatrixCell from "./MatrixCell";
import ChannelCreator from "./ChannelCreator";
import EmptyState from "./EmptyState";
import SettingsPanel from "./SettingsPanel";
import DragReorder from "./DragReorder";
import { useLevels } from "../stores/levelsStore";
import { Settings } from "lucide-solid";
import EqPage from "../eq/EqPage";
import type { EqPageTarget } from "../eq/EqPage";
import type { Endpoint, EndpointDescriptor, MixerLink, PwGroupNode } from "../types";

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
  const levels = useLevels();
  const [settingsOpen, setSettingsOpen] = createSignal(false);
  const [eqTarget, setEqTarget] = createSignal<EqPageTarget | null>(null);

  /** Get peak values for a channel by looking up its group node in the peak store. */
  function getPeaks(desc: EndpointDescriptor): { left: number; right: number } {
    if (!("channel" in desc)) return { left: 0, right: 0 };
    const group = graphState.graph.groupNodes[desc.channel] as PwGroupNode | undefined;
    if (!group?.id) return { left: 0, right: 0 };
    const p = levels.peaks[String(group.id)];
    return p ?? { left: 0, right: 0 };
  }

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
  const [localChannelOrder, setLocalChannelOrder] = createSignal<EndpointDescriptor[]>([]);
  const [localMixOrder, setLocalMixOrder] = createSignal<EndpointDescriptor[]>([]);
  let channelOrderInitialized = false;
  let mixOrderInitialized = false;

  createEffect(() => {
    const backendOrder = state.session.channelOrder;
    if (!channelOrderInitialized && backendOrder.length > 0) {
      channelOrderInitialized = true;
      setLocalChannelOrder(backendOrder);
    }
  });
  createEffect(() => {
    const backendOrder = state.session.mixOrder;
    if (!mixOrderInitialized && backendOrder.length > 0) {
      mixOrderInitialized = true;
      setLocalMixOrder(backendOrder);
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

  const channels = createMemo(() => applyOrder(rawChannels(), localChannelOrder()));
  const mixes = createMemo(() =>
    applyOrder(
      rawMixes().filter((m): m is { desc: EndpointDescriptor; ep: Endpoint } => m.ep != null),
      localMixOrder(),
    ),
  );

  function persistChannelOrder(reordered: EndpointEntry[]) {
    const order = reordered.map((item) => item.desc);
    setLocalChannelOrder(order);
    send({ type: "setChannelOrder", order });
  }

  function persistMixOrder(reordered: EndpointEntry[]) {
    const order = reordered.map((item) => item.desc);
    setLocalMixOrder(order);
    send({ type: "setMixOrder", order });
  }

  const [mixOutputs, setMixOutputs] = createStore<Record<string, string | null>>({});

  let outputsInitialized = false;
  createEffect(() => {
    const allDevs = getOutputDevices(graphState.graph.devices, graphState.graph.nodes);
    if (allDevs.length === 0 || outputsInitialized) return;

    for (const m of mixes()) {
      if (!("channel" in m.desc)) continue;
      const ch = state.session.channels[m.desc.channel];
      if (ch?.outputNodeId) {
        const dev = allDevs.find((d) => d.pwNodeId === ch.outputNodeId);
        if (dev) setMixOutputs(JSON.stringify(m.desc), dev.deviceId);
      }
    }

    const monitorMix = mixes().find((m) => m.ep?.displayName.toLowerCase().includes("monitor"));
    if (monitorMix) {
      const monitorKey = JSON.stringify(monitorMix.desc);
      if (!mixOutputs[monitorKey]) {
        const defaultName = graphState.graph.defaultSinkName;
        const defaultDev = defaultName ? allDevs.find((d) => d.deviceId === defaultName) : null;
        const autoDeviceId = defaultDev?.deviceId ?? allDevs[0]?.deviceId;
        if (autoDeviceId) {
          setMixOutputs(monitorKey, autoDeviceId);
          if ("channel" in monitorMix.desc) {
            const dev = allDevs.find((d) => d.deviceId === autoDeviceId);
            if (dev) {
              send({
                type: "setMixOutput",
                channel: monitorMix.desc.channel,
                outputNodeId: dev.pwNodeId ?? null,
              });
            }
          }
        }
      }
    }

    outputsInitialized = true;
  });

  function setMixOutput(mixKey: string, deviceId: string | null) {
    if (deviceId) {
      for (const [key, val] of Object.entries(mixOutputs)) {
        if (val === deviceId && key !== mixKey) {
          setMixOutputs(key, null);
        }
      }
    }
    setMixOutputs(mixKey, deviceId);

    const desc: EndpointDescriptor = JSON.parse(mixKey);
    if ("channel" in desc) {
      const allDevs = getOutputDevices(graphState.graph.devices, graphState.graph.nodes);
      const dev = deviceId ? allDevs.find((d) => d.deviceId === deviceId) : null;
      send({ type: "setMixOutput", channel: desc.channel, outputNodeId: dev?.pwNodeId ?? null });
    }
  }

  const usedDeviceIds = () => {
    const ids = new Set<string>();
    for (const val of Object.values(mixOutputs)) {
      if (val) ids.add(val);
    }
    return ids;
  };

  // --- EQ page navigation ---
  function openChannelEq(ep: Endpoint, desc: EndpointDescriptor) {
    const kind = channelKind(desc);
    const sourceType = kind === "source" ? "mic" : "app";
    setEqTarget({
      label: ep.customName ?? ep.displayName,
      sourceType,
      color: "var(--color-source-app)",
      endpoint: desc,
      initialEq: ep.eq,
    });
  }

  function openCellEq(source: EndpointDescriptor, sink: EndpointDescriptor) {
    const srcEp = findEndpoint(state.session.endpoints, source);
    const sinkEp = findEndpoint(state.session.endpoints, sink);
    const srcName = srcEp?.customName ?? srcEp?.displayName ?? "?";
    const sinkName = sinkEp?.customName ?? sinkEp?.displayName ?? "?";
    const link = state.session.links.find(
      (l) => JSON.stringify(l.start) === JSON.stringify(source) && JSON.stringify(l.end) === JSON.stringify(sink),
    );
    setEqTarget({
      label: `${srcName} → ${sinkName}`,
      sourceType: "cell",
      color: "var(--color-source-cell)",
      cellSource: source,
      cellTarget: sink,
      initialEq: link?.cellEq,
    });
  }

  function openMixEq(ep: Endpoint, desc: EndpointDescriptor) {
    setEqTarget({
      label: ep.customName ?? ep.displayName,
      sourceType: "mix",
      color: getMixColor(ep.displayName),
      endpoint: desc,
      initialEq: ep.eq,
    });
  }

  return (
    <div class="flex h-screen flex-col">
      {/* Top bar */}
      <header
        class="flex items-center justify-between border-b px-5 py-2"
        style={{
          "background-color": "var(--color-bg-secondary)",
          "border-color": "var(--color-border)",
        }}
      >
        <div class="flex items-center gap-4">
          <h1
            class="text-sm font-semibold tracking-tight"
            style={{ color: "var(--color-text-primary)" }}
          >
            Open Sound Grid
          </h1>
          {/* Grid presets */}
          <select
            class="rounded px-2 py-1 text-xs"
            style={{
              "background-color": "var(--color-bg-primary)",
              color: "var(--color-text-secondary)",
              border: "1px solid var(--color-border)",
            }}
          >
            <option>Default Grid</option>
            <option>Gaming</option>
            <option>Streaming</option>
            <option>Music Production</option>
          </select>
        </div>
        <div class="flex items-center gap-2">
          <button
            class="flex items-center gap-1 rounded p-1.5 transition-colors"
            style={{ color: "var(--color-text-muted)" }}
            onClick={() => setSettingsOpen(true)}
            aria-label="Settings"
          >
            <Settings size={16} />
          </button>
        </div>
      </header>

      {/* Main content area — either grid or EQ page */}
      <div class="flex-1 overflow-hidden relative">
        {/* Matrix grid view */}
        <div
          class="absolute inset-0 overflow-auto p-4 transition-transform duration-250"
          style={{
            "transition-timing-function": "var(--ease-out-quart)",
            transform: eqTarget() ? "translateX(-100%)" : "translateX(0)",
            "background-color": "var(--color-bg-primary)",
          }}
        >
          <Show when={graphState.connected} fallback={<EmptyState kind="disconnected" />}>
            {/* Mix column headers */}
            <div class="mb-2 flex items-stretch gap-2">
              <div class="flex w-48 shrink-0 items-stretch justify-end">
                <MixCreator maxMixes={8} currentCount={mixes().length} />
              </div>
              <DragReorder
                items={mixes()}
                keyFn={(m) => descKey(m.desc)}
                onReorder={persistMixOrder}
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
                          send({ type: "setEndpointVisible", endpoint: mix.desc, visible: false })
                        }
                        onSelectOutput={(deviceId) => setMixOutput(mixKey, deviceId)}
                        onOpenEq={() => openMixEq(mix.ep, mix.desc)}
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
                onReorder={persistChannelOrder}
              >
                {(ch, _idx, dragHandle) => (
                  <div class="flex items-stretch gap-2">
                    <ChannelLabel
                      descriptor={ch.desc}
                      endpoint={ch.ep}
                      channel={
                        "channel" in ch.desc ? state.session.channels[ch.desc.channel] : undefined
                      }
                      apps={Object.values(state.session.apps)}
                      dragHandle={dragHandle}
                      peakLeft={getPeaks(ch.desc).left}
                      peakRight={getPeaks(ch.desc).right}
                      onOpenEq={() => openChannelEq(ch.ep, ch.desc)}
                    />
                    <For each={mixes()}>
                      {({ desc: sinkDesc, ep: sinkEp }) => (
                        <MatrixCell
                          link={findLink(state.session.links, ch.desc, sinkDesc)}
                          sourceEndpoint={ch.ep}
                          sourceDescriptor={ch.desc}
                          sinkDescriptor={sinkDesc}
                          mixColor={getMixColor(sinkEp?.displayName ?? "")}
                          peakLeft={getPeaks(ch.desc).left}
                          peakRight={getPeaks(ch.desc).right}
                          onOpenEq={() => openCellEq(ch.desc, sinkDesc)}
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
        </div>

        {/* EQ page view — slides in from the right */}
        <div
          class="absolute inset-0 transition-transform duration-250"
          style={{
            "transition-timing-function": "var(--ease-out-quart)",
            transform: eqTarget() ? "translateX(0)" : "translateX(100%)",
          }}
        >
          <Show when={eqTarget()}>
            {(target) => <EqPage target={target()} onBack={() => setEqTarget(null)} send={send} />}
          </Show>
        </div>
      </div>

      {/* Status bar */}
      <footer
        class="flex items-center justify-between border-t px-5 py-1 text-[11px]"
        style={{
          "background-color": "var(--color-bg-secondary)",
          "border-color": "var(--color-border)",
          color: "var(--color-text-muted)",
        }}
      >
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
