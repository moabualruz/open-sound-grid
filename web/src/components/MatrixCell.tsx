import { Show, createEffect, createSignal, onCleanup } from "solid-js";
import type { JSX } from "solid-js";
import { useSession } from "../stores/sessionStore";
import { Volume2, VolumeX, Plus } from "lucide-solid";
import type { EndpointDescriptor, Endpoint, MixerLink } from "../types";

interface MatrixCellProps {
  /** The link between this channel and mix, or null if unrouted */
  link: MixerLink | null;
  /** Source (channel) endpoint info — used for volume display when linked */
  sourceEndpoint: Endpoint | undefined;
  /** Source descriptor */
  sourceDescriptor: EndpointDescriptor;
  /** Sink (mix) descriptor */
  sinkDescriptor: EndpointDescriptor;
  /** Mix color for accent */
  mixColor: string;
}

const DEBOUNCE_MS = 16;

export default function MatrixCell(props: MatrixCellProps): JSX.Element {
  const { send } = useSession();
  const [local, setLocal] = createSignal(0);
  let debounceTimer: ReturnType<typeof setTimeout> | null = null;

  // Sync from server — runs whenever sourceEndpoint.volume changes
  createEffect(() => setLocal(props.sourceEndpoint?.volume ?? 0));

  const isMuted = () => {
    const s = props.sourceEndpoint?.volumeLockedMuted;
    return s === "mutedLocked" || s === "mutedUnlocked" || s === "muteMixed";
  };

  function handleInput(value: number) {
    setLocal(value);
    if (debounceTimer) clearTimeout(debounceTimer);
    debounceTimer = setTimeout(() => {
      send({ type: "setVolume", endpoint: props.sourceDescriptor, volume: value });
    }, DEBOUNCE_MS);
  }

  function handleWheel(e: WheelEvent) {
    e.preventDefault();
    const step = e.deltaY > 0 ? -0.01 : 0.01;
    const next = Math.max(0, Math.min(1, local() + step));
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

  const pct = () => Math.round(local() * 100);

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
        {/* Active cell */}
        <div
          style={{ "--mix-accent": props.mixColor }}
          class={`flex h-full items-center gap-2 rounded-lg border px-3 py-2.5 transition-colors duration-150 ${
            isMuted() ? "border-vu-hot/20 bg-vu-hot/5" : "border-border bg-bg-elevated"
          }`}
        >
          {/* Per-cell mute button */}
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

          {/* VU-as-slider-track */}
          <div class="relative flex-1" onWheel={handleWheel}>
            {/* Volume level indicator — neutral until backend provides real peak data */}
            {/* TODO(backend): Replace with real-time peak levels from /ws/levels endpoint */}
            <div
              class="pointer-events-none absolute top-1/2 left-0 h-1.5 -translate-y-1/2 rounded-full"
              style={{
                width: `${pct()}%`,
                background: isMuted() ? "var(--color-text-muted)" : "var(--color-accent)",
                opacity: isMuted() ? 0.1 : 0.2,
              }}
            />
            <input
              type="range"
              min="0"
              max="1"
              step="0.01"
              value={local()}
              disabled={isMuted()}
              onInput={(e) => handleInput(parseFloat(e.currentTarget.value))}
              aria-label="Volume"
              aria-valuetext={`${pct()}%`}
              class="relative z-10 w-full"
            />
          </div>

          {/* Percentage label */}
          <span
            class={`w-8 text-right font-mono text-xs ${
              isMuted() ? "text-vu-hot/50" : "text-text-secondary"
            }`}
          >
            {pct()}
          </span>
        </div>
      </Show>
    </div>
  );
}
