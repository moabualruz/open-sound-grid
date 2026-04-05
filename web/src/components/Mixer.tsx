import { For, Show, createSignal, onMount, onCleanup } from "solid-js";
import MixEffectsRow from "./MixEffectsRow";
import { useSession } from "../stores/sessionStore";
import { useGraph } from "../stores/graphStore";
import MixHeader from "./MixHeader";
import MixCreator from "./MixCreator";
import ChannelLabel from "./ChannelLabel";
import MatrixCell from "./MatrixCell";
import ChannelCreator from "./ChannelCreator";
import EmptyState from "./EmptyState";
import SettingsPanel from "./SettingsPanel";
import WelcomeWizard from "./WelcomeWizard";
import DragReorder from "./DragReorder";
import { Settings, Eye, EyeOff, Power, Undo2, Redo2 } from "lucide-solid";
import EqPage from "../eq/EqPage";
import type { EqPageTarget } from "../eq/EqPage";
import type { Endpoint, EndpointDescriptor } from "../types/session";
import { getMixColor, findEndpoint, findLink } from "./mixerUtils";
import { useKeyboardNav } from "./useKeyboardNav";
import { useMixOutputs } from "./useMixOutputs";
import { useMixerViewModel } from "../hooks/useMixerViewModel";

export default function Mixer() {
  const { state, send } = useSession();
  const graphState = useGraph();
  const [settingsOpen, setSettingsOpen] = createSignal(false);
  const [eqTarget, setEqTarget] = createSignal<EqPageTarget | null>(null);
  const [wizardDismissed, setWizardDismissed] = createSignal(false);

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

  const [hiddenSectionOpen, setHiddenSectionOpen] = createSignal(false);

  // --- Mix expand accordion (only one mix expanded at a time) ---
  const [expandedMixKey, setExpandedMixKey] = createSignal<string | null>(null);

  function toggleMixExpand(mixKey: string) {
    setExpandedMixKey((current) => (current === mixKey ? null : mixKey));
  }

  const { mixOutputs, setMixOutput, usedDeviceIds } = useMixOutputs(
    mixes,
    () => state.session.channels,
    () => graphState.graph,
    send,
  );

  // --- Undo/Redo keyboard handler ---
  function handleUndoRedo(e: KeyboardEvent) {
    const target = e.target as HTMLElement;
    if (target.tagName === "INPUT" || target.tagName === "TEXTAREA") return;
    if (e.key === "z" && (e.ctrlKey || e.metaKey)) {
      if (e.shiftKey) {
        e.preventDefault();
        send({ type: "redo" });
      } else {
        e.preventDefault();
        send({ type: "undo" });
      }
    }
  }

  onMount(() => document.addEventListener("keydown", handleUndoRedo));
  onCleanup(() => document.removeEventListener("keydown", handleUndoRedo));

  // --- EQ page navigation ---
  function openCellEq(source: EndpointDescriptor, sink: EndpointDescriptor) {
    const srcEp = findEndpoint(state.session.endpoints, source);
    const sinkEp = findEndpoint(state.session.endpoints, sink);
    const srcName = srcEp?.customName ?? srcEp?.displayName ?? "?";
    const sinkName = sinkEp?.customName ?? sinkEp?.displayName ?? "?";
    const link = findLink(state.session.links, source, sink);
    const isMic =
      "channel" in source && state.session.channels[source.channel]?.sourceType === "hardwareMic";
    setEqTarget({
      label: `${srcName} → ${sinkName}`,
      sourceType: isMic ? "mic" : "cell",
      color: "var(--color-source-cell)",
      cellSource: source,
      cellTarget: sink,
      initialEq: link?.cellEq,
      initialEffects: link?.cellEffects,
      sinkDescriptor: sink,
    });
  }

  function openMixEq(ep: Endpoint, desc: EndpointDescriptor) {
    setEqTarget({
      label: ep.customName ?? ep.displayName,
      sourceType: "mix",
      color: getMixColor(ep.displayName),
      endpoint: desc,
      initialEq: ep.eq,
      initialEffects: ep.effects,
      sinkDescriptor: desc,
    });
  }

  // --- Keyboard navigation ---
  const { focusedCell, setFocusedCell, registerCellActions, handleGridKeyDown } = useKeyboardNav(
    channels,
    mixes,
    () => eqTarget() !== null,
    openCellEq,
  );

  const gridCols = () => `12rem repeat(${mixes().length}, minmax(10rem, 1fr))`;

  let gridRef: HTMLDivElement | undefined;

  return (
    <div class="flex h-screen flex-col">
      {/* Reconnecting banner */}
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
            style={{ color: "var(--color-text-muted)" }}
            onClick={() => setSettingsOpen(true)}
            aria-label="Settings"
          >
            <Settings size={16} />
          </button>
        </div>
      </header>

      {/* Main content area — either grid or EQ page */}
      <div class="relative flex-1 overflow-hidden">
        {/* Matrix grid view */}
        <div
          class="absolute inset-0 overflow-auto p-4 transition-transform duration-250"
          style={{
            "transition-timing-function": "var(--ease-out-quart)",
            transform: eqTarget() ? "translateX(-100%)" : "translateX(0)",
            "background-color": "var(--color-bg-primary)",
          }}
        >
          <Show when={state.connected} fallback={<EmptyState kind="disconnected" />}>
            <div
              ref={gridRef}
              role="grid"
              aria-label="Mixer matrix"
              tabIndex={0}
              onKeyDown={handleGridKeyDown}
              class="outline-none"
            >
              {/* Mix column headers */}
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
                  keyFn={(m) => descKey(m.desc)}
                  onReorder={persistMixOrder}
                  direction="horizontal"
                >
                  {(mix, _idx, dragHandle) => {
                    const mixKey = descKey(mix.desc);
                    return (
                      <div class="flex flex-col" role="columnheader">
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
                          expanded={expandedMixKey() === mixKey}
                          onToggleExpand={() => toggleMixExpand(mixKey)}
                        />
                      </div>
                    );
                  }}
                </DragReorder>
              </div>

              {/* Matrix rows */}
              <div class="flex flex-col gap-1.5">
                <DragReorder
                  items={channels()}
                  keyFn={(ch) => descKey(ch.desc)}
                  onReorder={persistChannelOrder}
                >
                  {(ch, rowIdx, dragHandle) => (
                    <>
                      <div
                        class="grid min-h-[4.5rem] items-stretch gap-2"
                        style={{ "grid-template-columns": gridCols() }}
                        role="row"
                      >
                        <ChannelLabel
                          descriptor={ch.desc}
                          endpoint={ch.ep}
                          channel={
                            "channel" in ch.desc
                              ? state.session.channels[ch.desc.channel]
                              : undefined
                          }
                          apps={Object.values(state.session.apps)}
                          dragHandle={dragHandle}
                        />
                        <For each={mixes()}>
                          {({ desc: sinkDesc, ep: sinkEp }, colIdx) => (
                            <div
                              role="gridcell"
                              aria-label={`${ch.ep.customName ?? ch.ep.displayName} to ${sinkEp?.customName ?? sinkEp?.displayName ?? "mix"}`}
                              onClick={() => setFocusedCell({ row: rowIdx(), col: colIdx() })}
                            >
                              <MatrixCell
                                link={findLink(state.session.links, ch.desc, sinkDesc)}
                                sourceEndpoint={ch.ep}
                                sourceDescriptor={ch.desc}
                                sinkDescriptor={sinkDesc}
                                mixColor={getMixColor(sinkEp?.displayName ?? "")}
                                onOpenEq={() => openCellEq(ch.desc, sinkDesc)}
                                focused={
                                  focusedCell()?.row === rowIdx() && focusedCell()?.col === colIdx()
                                }
                                onActionsReady={(actions) =>
                                  registerCellActions(rowIdx(), colIdx(), actions)
                                }
                              />
                            </div>
                          )}
                        </For>
                      </div>

                      {/* Effects sub-row — shown below each channel row when a mix is expanded */}
                      <Show when={expandedMixKey() !== null}>
                        {(() => {
                          const expandedKey = expandedMixKey()!;
                          const expandedMix = mixes().find((m) => descKey(m.desc) === expandedKey);
                          if (!expandedMix) return null;
                          const mixColor = getMixColor(expandedMix.ep.displayName);
                          const link = findLink(state.session.links, ch.desc, expandedMix.desc);
                          return (
                            <MixEffectsRow
                              cells={[
                                {
                                  sourceDescriptor: ch.desc,
                                  cellEq: link?.cellEq,
                                  cellEffects: link?.cellEffects,
                                  linked: link !== null,
                                },
                              ]}
                              mixColor={mixColor}
                              gridTemplateColumns={`12rem 1fr`}
                              onOpenCellEq={() => openCellEq(ch.desc, expandedMix.desc)}
                            />
                          );
                        })()}
                      </Show>
                    </>
                  )}
                </DragReorder>

                {/* Create channel */}
                <div class="flex gap-2">
                  <div class="w-48 shrink-0">
                    <ChannelCreator />
                  </div>
                </div>

                {/* Hidden channels collapsible section */}
                <Show when={hiddenChannels().length > 0}>
                  <div class="mt-2 border-t pt-2" style={{ "border-color": "var(--color-border)" }}>
                    <button
                      class="flex items-center gap-1.5 text-[11px] transition-colors"
                      style={{ color: "var(--color-text-muted)" }}
                      onClick={() => setHiddenSectionOpen((v) => !v)}
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
                          {(ch) => (
                            <div
                              class={`flex items-center gap-1.5 rounded border px-2 py-1 text-[11px] ${
                                ch.ep.disabled ? "opacity-40" : "opacity-60"
                              }`}
                              style={{
                                "border-color": "var(--color-border)",
                                "background-color": "var(--color-bg-elevated)",
                                color: "var(--color-text-muted)",
                              }}
                            >
                              <span>{ch.ep.customName ?? ch.ep.displayName}</span>
                              <Show when={ch.ep.disabled}>
                                <span class="text-vu-hot" title="Disabled">
                                  <Power size={10} />
                                </span>
                              </Show>
                              <button
                                onClick={() =>
                                  send({
                                    type: "setEndpointVisible",
                                    endpoint: ch.desc,
                                    visible: true,
                                  })
                                }
                                class="transition-colors hover:text-text-primary"
                                title="Show channel"
                                aria-label={`Show ${ch.ep.customName ?? ch.ep.displayName}`}
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

                {/* Empty states */}
                <Show when={channels().length === 0 && mixes().length > 0}>
                  <EmptyState kind="no-channels" />
                </Show>
                <Show when={mixes().length === 0}>
                  <EmptyState kind="no-mixes" />
                </Show>
              </div>
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
