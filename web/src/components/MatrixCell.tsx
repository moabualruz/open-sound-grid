import { createEffect, createSignal, onCleanup } from "solid-js";
import { useSession } from "../stores/sessionStore";
import type { Endpoint, EndpointDescriptor } from "../types";

interface MatrixCellProps {
  endpoint: Endpoint;
  descriptor: EndpointDescriptor;
  mixColor: string;
}

const DEBOUNCE_MS = 16;

export default function MatrixCell(props: MatrixCellProps) {
  const { send } = useSession();
  const [local, setLocal] = createSignal(0);
  let debounceTimer: ReturnType<typeof setTimeout> | null = null;

  createEffect(() => setLocal(props.endpoint.volume));

  const isMuted = () => {
    const s = props.endpoint.volumeLockedMuted;
    return s === "mutedLocked" || s === "mutedUnlocked" || s === "muteMixed";
  };

  function handleInput(value: number) {
    setLocal(value);
    if (debounceTimer) clearTimeout(debounceTimer);
    debounceTimer = setTimeout(() => {
      send({ type: "setVolume", endpoint: props.descriptor, volume: value });
    }, DEBOUNCE_MS);
  }

  function toggleMute() {
    send({ type: "setMute", endpoint: props.descriptor, muted: !isMuted() });
  }

  function handleWheel(e: WheelEvent) {
    e.preventDefault();
    const step = e.deltaY > 0 ? -0.01 : 0.01;
    const newVal = Math.max(0, Math.min(1, local() + step));
    handleInput(newVal);
  }

  onCleanup(() => {
    if (debounceTimer) clearTimeout(debounceTimer);
  });

  const pct = () => Math.round(local() * 100);

  return (
    <div
      class={`group flex min-w-44 flex-1 items-center gap-2 rounded-lg border px-3 py-2.5 transition-colors ${
        isMuted()
          ? "border-vu-hot/30 bg-vu-hot/5"
          : "border-border bg-bg-elevated hover:border-border-active/30"
      }`}
    >
      {/* Per-cell mute */}
      <button
        onClick={toggleMute}
        class={`shrink-0 text-sm transition-colors ${
          isMuted() ? "text-vu-hot" : "text-text-muted hover:text-text-primary"
        }`}
        title={isMuted() ? "Unmute in this mix" : "Mute in this mix"}
      >
        {isMuted() ? "🔇" : "🔊"}
      </button>

      {/* VU-as-slider-track */}
      <div class="relative flex-1" onWheel={handleWheel}>
        {/* VU meter fill (behind slider) */}
        <div
          class="pointer-events-none absolute top-1/2 left-0 h-1.5 -translate-y-1/2 rounded-full transition-all"
          style={{
            width: `${pct()}%`,
            background: isMuted()
              ? "var(--color-text-muted)"
              : pct() > 90
                ? "var(--color-vu-hot)"
                : pct() > 70
                  ? "var(--color-vu-warm)"
                  : "var(--color-vu-safe)",
            opacity: isMuted() ? 0.15 : 0.6,
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
          class="relative z-10 w-full"
        />
      </div>

      {/* Percentage */}
      <span
        class={`w-8 text-right font-mono text-xs ${isMuted() ? "text-vu-hot/60" : "text-text-secondary"}`}
      >
        {pct()}
      </span>
    </div>
  );
}
