import { Show, createEffect, createSignal, onCleanup } from "solid-js";
import type { JSX } from "solid-js";
import { useSession } from "../stores/sessionStore";
import { useMixerSettings } from "../stores/mixerSettings";
import { Volume2, VolumeX } from "lucide-solid";
import type { EndpointDescriptor, Endpoint, MixerLink } from "../types";

interface MatrixCellProps {
  link: MixerLink | null;
  sourceEndpoint: Endpoint | undefined;
  sourceDescriptor: EndpointDescriptor;
  sinkDescriptor: EndpointDescriptor;
  mixColor: string;
}

const DEBOUNCE_MS = 16;

export default function MatrixCell(props: MatrixCellProps): JSX.Element {
  const { send } = useSession();
  const { settings } = useMixerSettings();
  const [cellVol, setCellVol] = createSignal(1);
  const [cellL, setCellL] = createSignal(1);
  const [cellR, setCellR] = createSignal(1);
  let debounceTimer: ReturnType<typeof setTimeout> | null = null;
  let userDragging = false;

  const isStereo = () => settings.stereoMode === "stereo";

  // Sync from backend — but not while the user is actively dragging the slider
  createEffect(() => {
    if (userDragging) return;
    setCellVol(props.link?.cellVolume ?? 1);
    setCellL(props.link?.cellVolumeLeft ?? 1);
    setCellR(props.link?.cellVolumeRight ?? 1);
  });

  // Cell is "muted" when no link exists (unrouted) or channel is muted
  const isLinked = () => props.link !== null;
  const channelMuted = () => {
    const s = props.sourceEndpoint?.volumeLockedMuted;
    return s === "mutedLocked" || s === "mutedUnlocked" || s === "muteMixed";
  };
  const isMuted = () => !isLinked() || channelMuted();

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
    if (debounceTimer) clearTimeout(debounceTimer);
    debounceTimer = setTimeout(() => {
      send({
        type: "setLinkVolume",
        source: props.sourceDescriptor,
        target: props.sinkDescriptor,
        volume: value,
      });
      userDragging = false;
    }, DEBOUNCE_MS);
  }

  function handleStereoInput(channel: "left" | "right", value: number) {
    ensureLinked();
    userDragging = true;
    if (channel === "left") setCellL(value);
    else setCellR(value);
    setCellVol((cellL() + cellR()) / 2);
    if (debounceTimer) clearTimeout(debounceTimer);
    debounceTimer = setTimeout(() => {
      send({
        type: "setLinkStereoVolume",
        source: props.sourceDescriptor,
        target: props.sinkDescriptor,
        left: cellL(),
        right: cellR(),
      });
      userDragging = false;
    }, DEBOUNCE_MS);
  }

  function handleWheel(e: WheelEvent) {
    e.preventDefault();
    const step = e.deltaY > 0 ? -0.01 : 0.01;
    const next = Math.max(0, Math.min(1, cellVol() + step));
    handleInput(next);
  }

  function toggleRoute() {
    if (isLinked()) {
      send({ type: "removeLink", source: props.sourceDescriptor, target: props.sinkDescriptor });
    } else {
      send({ type: "link", source: props.sourceDescriptor, target: props.sinkDescriptor });
    }
  }

  onCleanup(() => {
    if (debounceTimer) clearTimeout(debounceTimer);
  });

  return (
    <div class="group min-w-[10rem] flex-1">
      <div
        style={{ "--mix-accent": props.mixColor }}
        class={`flex h-full items-center gap-2 rounded-lg border px-3 py-2 transition-colors duration-150 ${
          !isLinked()
            ? "border-border/30 bg-bg-primary/50 opacity-40 hover:opacity-70"
            : channelMuted()
              ? "border-vu-hot/20 bg-vu-hot/5"
              : "border-border bg-bg-elevated"
        }`}
      >
        {/* Per-cell route toggle */}
        <button
          type="button"
          onClick={toggleRoute}
          title={isLinked() ? "Disconnect route" : "Connect route"}
          aria-label={isLinked() ? "Disconnect route" : "Connect route"}
          class={`shrink-0 transition-colors duration-150 ${
            !isLinked()
              ? "text-text-muted/30 hover:text-text-muted"
              : channelMuted()
                ? "text-vu-hot"
                : "text-text-muted hover:text-text-primary"
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
              <div
                class="pointer-events-none absolute top-1/2 left-0 h-1 -translate-y-1/2 rounded-full"
                style={{
                  width: `${effectivePct()}%`,
                  background: isMuted() ? "var(--color-text-muted)" : "var(--color-vu-safe)",
                  opacity: isMuted() ? 0.08 : 0.25,
                }}
              />
              <div
                class="pointer-events-none absolute top-1/2 left-0 h-1.5 -translate-y-1/2 rounded-full"
                style={{
                  width: `${cellPct()}%`,
                  background: isMuted() ? "var(--color-text-muted)" : "var(--color-accent)",
                  opacity: isMuted() ? 0.1 : 0.15,
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
          <div class="flex flex-1 flex-col gap-0">
            <div class="flex items-center gap-1">
              <span class="w-2 text-[8px] font-bold text-text-muted">L</span>
              <input
                type="range"
                min="0"
                max="1"
                step="0.01"
                value={cellL()}
                onInput={(e) => handleStereoInput("left", parseFloat(e.currentTarget.value))}
                aria-label="Cell volume left"
                class="w-full"
              />
              <span class="w-10 text-right font-mono text-[9px] text-text-secondary">
                {cellPctL()}
                <Show when={cellPctL() !== effectivePctL()}>
                  <span class="text-text-muted">→{effectivePctL()}</span>
                </Show>
              </span>
            </div>
            <div class="flex items-center gap-1">
              <span class="w-2 text-[8px] font-bold text-text-muted">R</span>
              <input
                type="range"
                min="0"
                max="1"
                step="0.01"
                value={cellR()}
                onInput={(e) => handleStereoInput("right", parseFloat(e.currentTarget.value))}
                aria-label="Cell volume right"
                class="w-full"
              />
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
      </div>
    </div>
  );
}
