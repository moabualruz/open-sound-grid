import { Show, createEffect, createSignal } from "solid-js";
import type { JSX } from "solid-js";
import { useSession } from "../stores/sessionStore";
import { useMixerSettings } from "../stores/mixerSettings";
import { useMonitor } from "../stores/monitorStore";
import { Volume2, VolumeX, SlidersVertical, Headphones, Plus } from "lucide-solid";
import VuMeter from "./VuMeter";
import { useVolumeDebounce } from "../hooks/useVolumeDebounce";
import type { EndpointDescriptor, Endpoint, MixerLink } from "../types/session";

/** Imperative actions exposed to the parent grid for keyboard shortcuts. */
export interface MatrixCellActions {
  toggleMute: () => void;
  adjustVolume: (delta: number) => void;
}

interface MatrixCellProps {
  link: MixerLink | null;
  sourceEndpoint: Endpoint | undefined;
  sourceDescriptor: EndpointDescriptor;
  sinkDescriptor: EndpointDescriptor;
  mixColor: string;
  peakLeft?: number;
  peakRight?: number;
  onOpenEq?: () => void;
  focused?: boolean;
  /** Parent registers to receive imperative cell actions. */
  onActionsReady?: (actions: MatrixCellActions) => void;
}

export default function MatrixCell(props: MatrixCellProps): JSX.Element {
  const { send } = useSession();
  const { settings } = useMixerSettings();
  const monitor = useMonitor();
  const [cellVol, setCellVol] = createSignal(1);
  const [cellL, setCellL] = createSignal(1);
  const [cellR, setCellR] = createSignal(1);
  const [cellMuted, setCellMuted] = createSignal(false);
  let preMuteVol: { vol: number; left: number; right: number } | null = null;
  let userDragging = false;

  const sendDebounced = useVolumeDebounce((v) => {
    send({
      type: "setLinkVolume",
      source: props.sourceDescriptor,
      target: props.sinkDescriptor,
      volume: v,
    });
    userDragging = false;
  });

  const sendStereoDebounced = useVolumeDebounce((_v) => {
    send({
      type: "setLinkStereoVolume",
      source: props.sourceDescriptor,
      target: props.sinkDescriptor,
      left: cellL(),
      right: cellR(),
    });
    userDragging = false;
  });

  const isStereo = () => settings.stereoMode === "stereo";

  // Monitor state: is this cell being monitored, or muted by monitoring?
  const isMonitored = () =>
    monitor.state.monitoredCell !== null &&
    JSON.stringify(monitor.state.monitoredCell!.source) ===
      JSON.stringify(props.sourceDescriptor) &&
    JSON.stringify(monitor.state.monitoredCell!.target) === JSON.stringify(props.sinkDescriptor);

  // A cell is muted by monitoring if monitoring is active and this is not the monitored cell
  const mutedByMonitor = () =>
    monitor.state.monitoredCell !== null &&
    props.link !== null && // only consider linked cells
    !isMonitored();

  // Sync from backend — but not while the user is actively dragging the slider
  createEffect(() => {
    if (userDragging) return;
    setCellVol(props.link?.cellVolume ?? 1);
    setCellL(props.link?.cellVolumeLeft ?? 1);
    setCellR(props.link?.cellVolumeRight ?? 1);
  });

  // Cell is "muted" when no link exists, channel is muted, or cell is muted
  const isLinked = () => props.link !== null;
  const channelMuted = () => {
    const s = props.sourceEndpoint?.volumeLockedMuted;
    return s === "mutedLocked" || s === "mutedUnlocked" || s === "muteMixed";
  };
  const isMuted = () => !isLinked() || channelMuted() || cellMuted();

  const masterVol = () => props.sourceEndpoint?.volume ?? 1;
  const masterL = () => props.sourceEndpoint?.volumeLeft ?? 1;
  const masterR = () => props.sourceEndpoint?.volumeRight ?? 1;
  const cellPct = () => Math.round(cellVol() * 100);
  const cellPctL = () => Math.round(cellL() * 100);
  const cellPctR = () => Math.round(cellR() * 100);
  const effectivePct = () => Math.round(cellVol() * masterVol() * 100);
  const effectivePctL = () => Math.round(cellL() * masterL() * 100);
  const effectivePctR = () => Math.round(cellR() * masterR() * 100);

  function ensureLinked() {
    if (!isLinked()) {
      send({ type: "link", source: props.sourceDescriptor, target: props.sinkDescriptor });
    }
  }

  function handleInput(value: number) {
    ensureLinked();
    userDragging = true;
    setCellVol(value);
    setCellL(value);
    setCellR(value);
    sendDebounced(value);
  }

  function handleStereoInput(channel: "left" | "right", value: number) {
    ensureLinked();
    userDragging = true;
    if (channel === "left") setCellL(value);
    else setCellR(value);
    setCellVol((cellL() + cellR()) / 2);
    sendStereoDebounced(value);
  }

  function handleWheel(e: WheelEvent) {
    e.preventDefault();
    const step = e.deltaY > 0 ? -0.01 : 0.01;
    const next = Math.max(0, Math.min(1, cellVol() + step));
    handleInput(next);
  }

  function toggleCellMute() {
    if (!isLinked()) return;
    if (cellMuted()) {
      // Unmute: restore cached volume
      setCellMuted(false);
      const v = preMuteVol ?? { vol: 1, left: 1, right: 1 };
      preMuteVol = null;
      setCellVol(v.vol);
      setCellL(v.left);
      setCellR(v.right);
      send({
        type: "setLinkStereoVolume",
        source: props.sourceDescriptor,
        target: props.sinkDescriptor,
        left: v.left,
        right: v.right,
      });
    } else {
      // Mute: cache current volume, set 0
      preMuteVol = { vol: cellVol(), left: cellL(), right: cellR() };
      setCellMuted(true);
      setCellVol(0);
      setCellL(0);
      setCellR(0);
      send({
        type: "setLinkVolume",
        source: props.sourceDescriptor,
        target: props.sinkDescriptor,
        volume: 0,
      });
    }
  }

  // Expose imperative actions to parent for keyboard shortcuts
  function adjustVolume(delta: number) {
    const next = Math.max(0, Math.min(1, cellVol() + delta));
    handleInput(next);
  }

  createEffect(() => {
    props.onActionsReady?.({ toggleMute: toggleCellMute, adjustVolume });
  });

  return (
    <div class="group h-full">
      <Show
        when={isLinked()}
        fallback={
          /* Unlinked: minimal dashed cell — click anywhere to create the route */
          <button
            type="button"
            onClick={ensureLinked}
            title="Click to route this source to this mix"
            aria-label="Create audio route"
            class={`flex h-full w-full items-center justify-center rounded-lg border border-dashed transition-all duration-150 ${
              props.focused ? "ring-2 ring-accent" : ""
            } border-border/40 bg-bg-elevated text-text-muted/30 hover:border-border hover:text-text-muted/60`}
          >
            <Plus size={14} />
          </button>
        }
      >
        <div
          style={{ "--mix-accent": props.mixColor }}
          style={{ "--tw-ring-color": props.focused ? "var(--color-accent)" : undefined }}
          class={`flex h-full items-center gap-2 rounded-lg border px-3 py-2 transition-all duration-150 ${
            props.focused ? "ring-2" : ""
          } ${
            isMonitored()
              ? "border-accent bg-bg-elevated ring-2 ring-accent/40"
              : mutedByMonitor()
                ? "border-border/30 bg-bg-primary/50 opacity-30"
                : channelMuted()
                  ? "border-vu-hot/20 bg-vu-hot/5"
                  : "border-border bg-bg-elevated"
          }`}
        >
          {/* Per-cell route toggle */}
          <button
            type="button"
            onClick={toggleCellMute}
            title={cellMuted() ? "Unmute cell" : "Mute cell"}
            aria-label={cellMuted() ? "Unmute cell" : "Mute cell"}
            class={`shrink-0 transition-colors duration-150 ${
              isMuted() ? "text-vu-hot" : "text-text-muted hover:text-text-primary"
            }`}
          >
            <Show when={isMuted()} fallback={<Volume2 size={14} />}>
              <VolumeX size={14} />
            </Show>
          </button>

          {/* Cell volume slider(s) */}
          <Show
            when={isStereo()}
            fallback={
              <div class="relative flex-1" onWheel={handleWheel}>
                {/* Peak level fill (behind slider) */}
                <div
                  class="pointer-events-none absolute top-1/2 left-0 h-2.5 -translate-y-1/2 rounded-full transition-all duration-75"
                  style={{
                    width: `${Math.round(Math.max(props.peakLeft ?? 0, props.peakRight ?? 0) * 100)}%`,
                    background: "var(--color-vu-gradient)",
                    "background-size": `${Math.max(props.peakLeft ?? 0, props.peakRight ?? 0) > 0 ? Math.round(100 / Math.max(props.peakLeft ?? 0.01, props.peakRight ?? 0.01)) : 100}% 100%`,
                    opacity: isMuted() ? 0.08 : 0.45,
                  }}
                />
                <input
                  type="range"
                  min="0"
                  max="1"
                  step="0.01"
                  value={cellVol()}
                  onInput={(e) => handleInput(parseFloat(e.currentTarget.value))}
                  aria-label="Cell volume"
                  aria-valuetext={`${cellPct()}% (effective ${effectivePct()}%)`}
                  class="relative z-10 w-full"
                />
              </div>
            }
          >
            <div class="flex flex-1 flex-col gap-1.5">
              <div class="flex items-center gap-1">
                <span class="w-2 text-[8px] font-bold text-text-muted">L</span>
                <div class="relative flex-1">
                  <div
                    class="pointer-events-none absolute top-1/2 left-0 h-2.5 -translate-y-1/2 rounded-full transition-all duration-75"
                    style={{
                      width: `${Math.round((props.peakLeft ?? 0) * 100)}%`,
                      background: "var(--color-vu-gradient)",
                      "background-size": `${(props.peakLeft ?? 0) > 0 ? Math.round(100 / (props.peakLeft ?? 0.01)) : 100}% 100%`,
                      opacity: 0.45,
                    }}
                  />
                  <input
                    type="range"
                    min="0"
                    max="1"
                    step="0.01"
                    value={cellL()}
                    onInput={(e) => handleStereoInput("left", parseFloat(e.currentTarget.value))}
                    aria-label="Cell volume left"
                    class="relative z-10 w-full"
                  />
                </div>
                <span class="w-10 text-right font-mono text-[9px] text-text-secondary">
                  {cellPctL()}
                  <Show when={cellPctL() !== effectivePctL()}>
                    <span class="text-text-muted">→{effectivePctL()}</span>
                  </Show>
                </span>
              </div>
              <div class="flex items-center gap-1">
                <span class="w-2 text-[8px] font-bold text-text-muted">R</span>
                <div class="relative flex-1">
                  <div
                    class="pointer-events-none absolute top-1/2 left-0 h-2.5 -translate-y-1/2 rounded-full transition-all duration-75"
                    style={{
                      width: `${Math.round((props.peakRight ?? 0) * 100)}%`,
                      background: "var(--color-vu-gradient)",
                      "background-size": `${(props.peakRight ?? 0) > 0 ? Math.round(100 / (props.peakRight ?? 0.01)) : 100}% 100%`,
                      opacity: 0.45,
                    }}
                  />
                  <input
                    type="range"
                    min="0"
                    max="1"
                    step="0.01"
                    value={cellR()}
                    onInput={(e) => handleStereoInput("right", parseFloat(e.currentTarget.value))}
                    aria-label="Cell volume right"
                    class="relative z-10 w-full"
                  />
                </div>
                <span class="w-10 text-right font-mono text-[9px] text-text-secondary">
                  {cellPctR()}
                  <Show when={cellPctR() !== effectivePctR()}>
                    <span class="text-text-muted">→{effectivePctR()}</span>
                  </Show>
                </span>
              </div>
            </div>
          </Show>

          {/* Percentage (mono only) */}
          <Show when={!isStereo()}>
            <div
              class={`flex flex-col items-end font-mono text-[10px] leading-tight ${
                isMuted() ? "text-text-muted/30" : "text-text-secondary"
              }`}
            >
              <span>{cellPct()}</span>
              <Show when={!isMuted() && cellPct() !== effectivePct()}>
                <span class="text-text-muted">→{effectivePct()}</span>
              </Show>
            </div>
          </Show>

          {/* EQ button — visible on hover when route is active */}
          <button
            type="button"
            onClick={() => props.onOpenEq?.()}
            class={`shrink-0 transition-colors duration-150 ${
              isMonitored()
                ? "text-accent"
                : "text-text-muted/0 group-hover:text-text-muted/60 hover:!text-accent"
            }`}
            title="EQ & Effects"
            aria-label="EQ & Effects"
          >
            <Show when={isMonitored()} fallback={<SlidersVertical size={12} />}>
              <Headphones size={12} />
            </Show>
          </button>
        </div>

        {/* VU meter — reads peak levels from the cell's PipeWire node */}
        <div class="px-3 pb-1">
          <VuMeter nodeId={props.link?.cellNodeId ?? undefined} />
        </div>
      </Show>
    </div>
  );
}
