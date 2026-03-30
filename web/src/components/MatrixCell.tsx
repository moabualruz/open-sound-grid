import { Show, createEffect, createSignal, onCleanup } from "solid-js";
import type { JSX } from "solid-js";
import { useSession } from "../stores/sessionStore";
import { Volume2, VolumeX, Plus } from "lucide-solid";
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
  const [cellVol, setCellVol] = createSignal(1);
  let debounceTimer: ReturnType<typeof setTimeout> | null = null;

  // Sync cell volume from link's cellVolume (per-route, independent)
  createEffect(() => setCellVol(props.link?.cellVolume ?? 1));

  const isMuted = () => {
    const s = props.sourceEndpoint?.volumeLockedMuted;
    return s === "mutedLocked" || s === "mutedUnlocked" || s === "muteMixed";
  };

  const masterVol = () => props.sourceEndpoint?.volume ?? 1;
  const cellPct = () => Math.round(cellVol() * 100);
  const effectivePct = () => Math.round(cellVol() * masterVol() * 100);

  function handleInput(value: number) {
    setCellVol(value);
    if (debounceTimer) clearTimeout(debounceTimer);
    debounceTimer = setTimeout(() => {
      send({
        type: "setLinkVolume",
        source: props.sourceDescriptor,
        target: props.sinkDescriptor,
        volume: value,
      });
    }, DEBOUNCE_MS);
  }

  function handleWheel(e: WheelEvent) {
    e.preventDefault();
    const step = e.deltaY > 0 ? -0.01 : 0.01;
    const next = Math.max(0, Math.min(1, cellVol() + step));
    handleInput(next);
  }

  function toggleMute() {
    send({ type: "setMute", endpoint: props.sourceDescriptor, muted: !isMuted() });
  }

  function handleLink() {
    send({ type: "link", source: props.sourceDescriptor, target: props.sinkDescriptor });
  }

  onCleanup(() => {
    if (debounceTimer) clearTimeout(debounceTimer);
  });

  return (
    <div class="group min-w-[10rem] flex-1">
      <Show
        when={props.link !== null}
        fallback={
          <div
            onClick={handleLink}
            style={{ "--mix-accent": props.mixColor }}
            class="flex h-full items-center justify-center rounded-lg border border-dashed border-border bg-bg-empty-cell px-3 cursor-pointer transition-colors duration-150 hover:bg-bg-hover/50"
          >
            <Plus
              size={16}
              class="text-text-muted/40 opacity-0 group-hover:opacity-100 transition-opacity duration-150"
            />
          </div>
        }
      >
        <div
          style={{ "--mix-accent": props.mixColor }}
          class={`flex h-full items-center gap-2 rounded-lg border px-3 py-2 transition-colors duration-150 ${
            isMuted() ? "border-vu-hot/20 bg-vu-hot/5" : "border-border bg-bg-elevated"
          }`}
        >
          {/* Per-cell mute */}
          <button
            type="button"
            onClick={toggleMute}
            title={isMuted() ? "Unmute in this mix" : "Mute in this mix"}
            aria-label={isMuted() ? "Unmute in this mix" : "Mute in this mix"}
            class={`shrink-0 transition-colors duration-150 ${
              isMuted() ? "text-vu-hot" : "text-text-muted hover:text-text-primary"
            }`}
          >
            <Show when={isMuted()} fallback={<Volume2 size={14} />}>
              <VolumeX size={14} />
            </Show>
          </button>

          {/* Cell volume slider */}
          <div class="relative flex-1" onWheel={handleWheel}>
            {/* Effective volume ghost indicator (master × cell) */}
            <div
              class="pointer-events-none absolute top-1/2 left-0 h-1 -translate-y-1/2 rounded-full"
              style={{
                width: `${effectivePct()}%`,
                background: isMuted() ? "var(--color-text-muted)" : "var(--color-vu-safe)",
                opacity: isMuted() ? 0.08 : 0.25,
              }}
            />
            {/* Cell ratio indicator */}
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
              disabled={isMuted()}
              onInput={(e) => handleInput(parseFloat(e.currentTarget.value))}
              aria-label="Cell volume"
              aria-valuetext={`${cellPct()}% (effective ${effectivePct()}%)`}
              class="relative z-10 w-full"
            />
          </div>

          {/* Percentage: cell% (→effective%) */}
          <div
            class={`flex flex-col items-end font-mono text-[10px] leading-tight ${
              isMuted() ? "text-vu-hot/50" : "text-text-secondary"
            }`}
          >
            <span>{cellPct()}</span>
            <Show when={cellPct() !== effectivePct()}>
              <span class="text-text-muted">→{effectivePct()}</span>
            </Show>
          </div>
        </div>
      </Show>
    </div>
  );
}
