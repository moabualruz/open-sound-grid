import { For, Show, createEffect, createSignal, onCleanup, onMount } from "solid-js";
import { Eye, EyeOff, Maximize2, Minimize2, Power, Redo2, Settings, Undo2 } from "lucide-solid";
import type { EqPageTarget } from "../eq/EqPage";
import EqPage from "../eq/EqPage";
import type { Channel, Endpoint, EndpointDescriptor } from "../types/session";
import { useGraph } from "../stores/graphStore";
import { useSession } from "../stores/sessionStore";
import { useMixerViewModel } from "../hooks/useMixerViewModel";
import ChannelCreator from "./ChannelCreator";
import ChannelLabel from "./ChannelLabel";
import CompactMode from "./CompactMode";
import DragReorder from "./DragReorder";
import EmptyState from "./EmptyState";
import MatrixCell from "./MatrixCell";
import MixCreator from "./MixCreator";
import MixEffectsRow from "./MixEffectsRow";
import MixHeader from "./MixHeader";
import SettingsPanel from "./SettingsPanel";
import WelcomeWizard from "./WelcomeWizard";
import { findEndpoint, findLink, getMixColor } from "./mixerUtils";
import { useKeyboardNav } from "./useKeyboardNav";
import { useMixOutputs } from "./useMixOutputs";

function channelAccentColor(sourceType?: Channel["sourceType"]): string {
  switch (sourceType) {
    case "hardwareMic":
      return "var(--color-source-mic)";
    case "appStream":
      return "var(--color-source-app)";
    default:
      return "var(--color-source-cell)";
  }
}

export default function Mixer() {
  const { state, send } = useSession();
  const graphState = useGraph();
  const [settingsOpen, setSettingsOpen] = createSignal(false);
  const [eqTarget, setEqTarget] = createSignal<EqPageTarget | null>(null);
  const [wizardDismissed, setWizardDismissed] = createSignal(false);
  const [hiddenSectionOpen, setHiddenSectionOpen] = createSignal(false);
  const [expandedMixKey, setExpandedMixKey] = createSignal<string | null>(null);
  const [compactMode, setCompactMode] = createSignal(false);
  const [compactMixKey, setCompactMixKey] = createSignal<string | null>(null);

  const showWelcomeWizard = () =>
    !wizardDismissed() &&
    !state.session.welcomeDismissed &&
    Object.keys(state.session.channels).length === 0;

  const {
    channels,
    hiddenChannels,
    mixes,
    descKey,
    persistChannelOrder,
    persistMixOrder,
  } = useMixerViewModel();

  function toggleMixExpand(mixKey: string) {
    setExpandedMixKey((current) => (current === mixKey ? null : mixKey));
  }

  const { mixOutputs, setMixOutput, usedDeviceIds } = useMixOutputs(
    mixes,
    () => state.session.channels,
    () => graphState.graph,
    send,
  );

  function handleUndoRedo(event: KeyboardEvent) {
    const target = event.target as HTMLElement;
    if (target.tagName === "INPUT" || target.tagName === "TEXTAREA") return;

    if (event.key === "z" && (event.ctrlKey || event.metaKey)) {
      event.preventDefault();
      send({ type: event.shiftKey ? "redo" : "undo" });
    }
  }

  function handleOpenSettings() {
    setSettingsOpen(true);
  }

  onMount(() => {
    document.addEventListener("keydown", handleUndoRedo);
    window.addEventListener("osg:open-settings", handleOpenSettings);
  });

  onCleanup(() => {
    document.removeEventListener("keydown", handleUndoRedo);
    window.removeEventListener("osg:open-settings", handleOpenSettings);
  });

  function openCellEq(source: EndpointDescriptor, sink: EndpointDescriptor) {
    const sourceEndpoint = findEndpoint(state.session.endpoints, source);
    const sinkEndpoint = findEndpoint(state.session.endpoints, sink);
    const sourceName = sourceEndpoint?.customName ?? sourceEndpoint?.displayName ?? "?";
    const sinkName = sinkEndpoint?.customName ?? sinkEndpoint?.displayName ?? "?";
    const link = findLink(state.session.links, source, sink);
    const isMic =
      "channel" in source && state.session.channels[source.channel]?.sourceType === "hardwareMic";

    setEqTarget({
      label: `${sourceName} → ${sinkName}`,
      sourceType: isMic ? "mic" : "cell",
      color: "var(--color-source-cell)",
      cellSource: source,
      cellTarget: sink,
      initialEq: link?.cellEq,
      initialEffects: link?.cellEffects,
      sinkDescriptor: sink,
      spectrumNodeKey:
        "channel" in source && "channel" in sink
          ? `${source.channel}-to-${sink.channel}`
          : undefined,
    });
  }

  function openMixEq(endpoint: Endpoint, descriptor: EndpointDescriptor) {
    setEqTarget({
      label: endpoint.customName ?? endpoint.displayName,
      sourceType: "mix",
      color: getMixColor(endpoint.displayName),
      endpoint: descriptor,
      initialEq: endpoint.eq,
      initialEffects: endpoint.effects,
      sinkDescriptor: descriptor,
      spectrumNodeKey: "channel" in descriptor ? `mix.${descriptor.channel}` : undefined,
    });
  }

  function openChannelEffects(endpoint: Endpoint, descriptor: EndpointDescriptor) {
    const channel = "channel" in descriptor ? state.session.channels[descriptor.channel] : undefined;
    setEqTarget({
      label: endpoint.customName ?? endpoint.displayName,
      sourceType: channel?.sourceType === "hardwareMic" ? "mic" : "app",
      color: channelAccentColor(channel?.sourceType),
      endpoint: descriptor,
      initialEq: endpoint.eq,
      initialEffects: endpoint.effects,
      sinkDescriptor: descriptor,
      spectrumNodeKey: "channel" in descriptor ? `source.${descriptor.channel}` : undefined,
    });
  }

  const { focusedCell, setFocusedCell, registerCellActions, handleGridKeyDown } = useKeyboardNav(
    channels,
    mixes,
    () => eqTarget() !== null,
    openCellEq,
  );

  createEffect(() => {
    const availableMixes = mixes();
    const current = compactMixKey();

    if (availableMixes.length === 0) {
      if (current !== null) setCompactMixKey(null);
      return;
    }

    if (!current || !availableMixes.some((mix) => descKey(mix.desc) === current)) {
      setCompactMixKey(descKey(availableMixes[0].desc));
    }
  });

  const gridCols = () => `12rem repeat(${mixes().length}, minmax(10rem, 1fr))`;
  let gridRef: HTMLDivElement | undefined;

  return (
    <div class="flex h-screen flex-col">
      <Show when={state.reconnecting}>
        <div
          aria-live="assertive"
          role="status"
          class="flex items-center justify-center gap-2 bg-vu-hot/10 px-4 py-1.5 text-xs text-vu-hot"
        >
          Reconnecting to PipeWire… attempt {state.reconnectAttempt + 1}
          {state.nextRetryMs > 0 && ` · retry in ${(state.nextRetryMs / 1000).toFixed(0)}s`}
        </div>
      </Show>

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
        </div>

        <div class="flex items-center gap-2">
          <button
            class="flex items-center gap-1 rounded p-1.5 transition-colors"
            style={{
              color: "var(--color-text-muted)",
              opacity: state.session.canUndo ? 1 : 0.35,
            }}
            onClick={() => send({ type: "undo" })}
            disabled={!state.session.canUndo}
            aria-label="Undo"
            title="Undo (Ctrl+Z)"
          >
            <Undo2 size={16} />
          </button>
          <button
            class="flex items-center gap-1 rounded p-1.5 transition-colors"
            style={{
              color: "var(--color-text-muted)",
              opacity: state.session.canRedo ? 1 : 0.35,
            }}
            onClick={() => send({ type: "redo" })}
            disabled={!state.session.canRedo}
            aria-label="Redo"
            title="Redo (Ctrl+Shift+Z)"
          >
            <Redo2 size={16} />
          </button>
          <button
            class="flex items-center gap-1 rounded p-1.5 transition-colors"
            style={{ color: compactMode() ? "var(--color-accent)" : "var(--color-text-muted)" }}
            onClick={() => setCompactMode((value) => !value)}
            aria-label={compactMode() ? "Exit compact mode" : "Enable compact mode"}
            title={compactMode() ? "Exit compact mode" : "Enable compact mode"}
          >
            <Show when={compactMode()} fallback={<Minimize2 size={16} />}>
              <Maximize2 size={16} />
            </Show>
          </button>
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

      <div class="relative flex-1 overflow-hidden">
        <div
          class="absolute inset-0 overflow-auto p-4 transition-transform duration-250"
          style={{
            "transition-timing-function": "var(--ease-out-quart)",
            transform: eqTarget() ? "translateX(-100%)" : "translateX(0)",
            "background-color": "var(--color-bg-primary)",
          }}
        >
          <Show when={state.connected} fallback={<EmptyState kind="disconnected" />}>
            <Show
              when={compactMode()}
              fallback={
                <div
                  ref={gridRef}
                  role="grid"
                  aria-label="Mixer matrix"
                  tabIndex={0}
                  onKeyDown={handleGridKeyDown}
                  class="outline-none"
                >
                  <div
                    class="mb-2 grid items-stretch gap-2"
                    style={{ "grid-template-columns": gridCols() }}
                    role="row"
                  >
                    <div class="flex items-stretch justify-end" role="columnheader">
                      <MixCreator maxMixes={8} currentCount={mixes().length} />
                    </div>
                    <DragReorder
                      items={mixes()}
                      keyFn={(mix) => descKey(mix.desc)}
                      onReorder={persistMixOrder}
                      direction="horizontal"
                    >
                      {(mix, _index, dragHandle) => {
                        const mixKey = descKey(mix.desc);
                        return (
                          <div class="flex flex-col" role="columnheader">
                            <MixHeader
                              descriptor={mix.desc}
                              endpoint={mix.ep}
                              color={getMixColor(mix.ep.displayName)}
                              outputDevice={mixOutputs[mixKey] ?? null}
                              usedDeviceIds={usedDeviceIds()}
                              onRemove={() => send({ type: "removeEndpoint", endpoint: mix.desc })}
                              onSelectOutput={(deviceId) => setMixOutput(mixKey, deviceId)}
                              onOpenEq={() => openMixEq(mix.ep, mix.desc)}
                              dragHandle={dragHandle}
                              expanded={expandedMixKey() === mixKey}
                              onToggleExpand={() => toggleMixExpand(mixKey)}
                            />
                          </div>
                        );
                      }}
                    </DragReorder>
                  </div>

                  <div class="flex flex-col gap-1.5">
                    <DragReorder
                      items={channels()}
                      keyFn={(channel) => descKey(channel.desc)}
                      onReorder={persistChannelOrder}
                    >
                      {(channel, rowIdx, dragHandle) => (
                        <>
                          <div
                            class="grid min-h-[4.5rem] items-stretch gap-2"
                            style={{ "grid-template-columns": gridCols() }}
                            role="row"
                          >
                            <ChannelLabel
                              descriptor={channel.desc}
                              endpoint={channel.ep}
                              channel={
                                "channel" in channel.desc
                                  ? state.session.channels[channel.desc.channel]
                                  : undefined
                              }
                              apps={Object.values(state.session.apps)}
                              dragHandle={dragHandle}
                              onOpenEffects={() => openChannelEffects(channel.ep, channel.desc)}
                            />
                            <For each={mixes()}>
                              {({ desc: sinkDesc, ep: sinkEndpoint }, colIdx) => (
                                <div
                                  role="gridcell"
                                  aria-label={`${channel.ep.customName ?? channel.ep.displayName} to ${sinkEndpoint?.customName ?? sinkEndpoint?.displayName ?? "mix"}`}
                                  onClick={() => setFocusedCell({ row: rowIdx(), col: colIdx() })}
                                >
                                  <MatrixCell
                                    link={findLink(state.session.links, channel.desc, sinkDesc)}
                                    sourceEndpoint={channel.ep}
                                    sourceDescriptor={channel.desc}
                                    sinkDescriptor={sinkDesc}
                                    mixColor={getMixColor(sinkEndpoint?.displayName ?? "")}
                                    onOpenEq={() => openCellEq(channel.desc, sinkDesc)}
                                    focused={
                                      focusedCell()?.row === rowIdx() &&
                                      focusedCell()?.col === colIdx()
                                    }
                                    onActionsReady={(actions) =>
                                      registerCellActions(rowIdx(), colIdx(), actions)
                                    }
                                  />
                                </div>
                              )}
                            </For>
                          </div>

                          <Show when={expandedMixKey() !== null}>
                            {(() => {
                              const currentMixKey = expandedMixKey()!;
                              const expandedMix = mixes().find((mix) => descKey(mix.desc) === currentMixKey);
                              if (!expandedMix) return null;

                              const mixColor = getMixColor(expandedMix.ep.displayName);
                              const link = findLink(state.session.links, channel.desc, expandedMix.desc);

                              return (
                                <MixEffectsRow
                                  cells={[
                                    {
                                      sourceDescriptor: channel.desc,
                                      cellEq: link?.cellEq,
                                      cellEffects: link?.cellEffects,
                                      linked: link !== null,
                                    },
                                  ]}
                                  mixColor={mixColor}
                                  gridTemplateColumns="12rem 1fr"
                                  onOpenCellEq={() => openCellEq(channel.desc, expandedMix.desc)}
                                />
                              );
                            })()}
                          </Show>
                        </>
                      )}
                    </DragReorder>

                    <div class="flex gap-2">
                      <div class="w-48 shrink-0">
                        <ChannelCreator />
                      </div>
                    </div>

                    <Show when={hiddenChannels().length > 0}>
                      <div class="mt-2 border-t pt-2" style={{ "border-color": "var(--color-border)" }}>
                        <button
                          class="flex items-center gap-1.5 text-[11px] transition-colors"
                          style={{ color: "var(--color-text-muted)" }}
                          onClick={() => setHiddenSectionOpen((value) => !value)}
                          aria-expanded={hiddenSectionOpen()}
                          aria-label="Toggle hidden channels"
                        >
                          <EyeOff size={12} />
                          <span>
                            {hiddenChannels().length} hidden channel
                            {hiddenChannels().length !== 1 ? "s" : ""}
                          </span>
                          <span class="ml-1">{hiddenSectionOpen() ? "▲" : "▼"}</span>
                        </button>
                        <Show when={hiddenSectionOpen()}>
                          <div class="mt-2 flex flex-wrap gap-2">
                            <For each={hiddenChannels()}>
                              {(channel) => (
                                <div
                                  class={`flex items-center gap-1.5 rounded border px-2 py-1 text-[11px] ${
                                    channel.ep.disabled ? "opacity-40" : "opacity-60"
                                  }`}
                                  style={{
                                    "border-color": "var(--color-border)",
                                    "background-color": "var(--color-bg-elevated)",
                                    color: "var(--color-text-muted)",
                                  }}
                                >
                                  <span>{channel.ep.customName ?? channel.ep.displayName}</span>
                                  <Show when={channel.ep.disabled}>
                                    <span class="text-vu-hot" title="Disabled">
                                      <Power size={10} />
                                    </span>
                                  </Show>
                                  <button
                                    onClick={() =>
                                      send({
                                        type: "setEndpointVisible",
                                        endpoint: channel.desc,
                                        visible: true,
                                      })
                                    }
                                    class="transition-colors hover:text-text-primary"
                                    title="Show channel"
                                    aria-label={`Show ${channel.ep.customName ?? channel.ep.displayName}`}
                                  >
                                    <Eye size={11} />
                                  </button>
                                </div>
                              )}
                            </For>
                          </div>
                        </Show>
                      </div>
                    </Show>

                    <Show when={channels().length === 0 && mixes().length > 0}>
                      <EmptyState kind="no-channels" />
                    </Show>
                    <Show when={mixes().length === 0}>
                      <EmptyState kind="no-mixes" />
                    </Show>
                  </div>
                </div>
              }
            >
              <CompactMode
                channels={channels()}
                channelsById={state.session.channels}
                links={state.session.links}
                mixes={mixes()}
                selectedMixKey={compactMixKey()}
                onSelectMixKey={setCompactMixKey}
                descKey={descKey}
                mixOutputs={mixOutputs}
                usedDeviceIds={usedDeviceIds()}
                onSelectOutput={setMixOutput}
                onOpenCellEq={openCellEq}
                onOpenChannelEffects={openChannelEffects}
                onOpenMixEq={openMixEq}
                onRemoveMix={(descriptor) => send({ type: "removeEndpoint", endpoint: descriptor })}
              />
            </Show>
          </Show>
        </div>

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

      <footer
        aria-live="polite"
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
          <Show when={state.session.lastPresetName}>
            {(presetName) => <span>Preset: {presetName()}</span>}
          </Show>
          <span>{channels().length} channels</span>
          <span>{mixes().length} mixes</span>
          <span>{Object.keys(graphState.graph.nodes).length} nodes</span>
          <span>v0.1.0</span>
        </div>
      </footer>

      <SettingsPanel open={settingsOpen()} onClose={() => setSettingsOpen(false)} />

      <Show when={showWelcomeWizard()}>
        <WelcomeWizard onDone={() => setWizardDismissed(true)} />
      </Show>
    </div>
  );
}
