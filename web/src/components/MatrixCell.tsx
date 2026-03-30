import { createEffect, createSignal, onCleanup } from "solid-js";
import { useSession } from "../stores/sessionStore";
import type { Endpoint, EndpointDescriptor } from "../types";

interface MatrixCellProps {
  endpoint: Endpoint;
  descriptor: EndpointDescriptor;
  mixName: string;
  mixColor: string;
}

const DEBOUNCE_MS = 16;

export default function MatrixCell(props: MatrixCellProps) {
  const { send } = useSession();
  const [local, setLocal] = createSignal(0);
  let debounceTimer: ReturnType<typeof setTimeout> | null = null;

  createEffect(() => setLocal(props.endpoint.volume));

  function handleInput(value: number) {
    setLocal(value);
    if (debounceTimer) clearTimeout(debounceTimer);
    debounceTimer = setTimeout(() => {
      send({ type: "setVolume", endpoint: props.descriptor, volume: value });
    }, DEBOUNCE_MS);
  }

  onCleanup(() => {
    if (debounceTimer) clearTimeout(debounceTimer);
  });

  const isMuted = () => {
    const s = props.endpoint.volumeLockedMuted;
    return s === "mutedLocked" || s === "mutedUnlocked" || s === "muteMixed";
  };

  const pct = () => Math.round(local() * 100);

  return (
    <div class="flex min-w-40 flex-1 items-center gap-2 border-r border-border px-3 py-2">
      <div class="relative flex-1">
        {/* Active fill behind slider */}
        <div
          class="pointer-events-none absolute top-1/2 left-0 h-1 -translate-y-1/2 rounded-l"
          style={{
            width: `${pct()}%`,
            background: isMuted() ? "var(--color-text-muted)" : props.mixColor,
            opacity: isMuted() ? 0.2 : 0.5,
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
          style={{
            "--tw-accent": props.mixColor,
          }}
        />
      </div>
      <span class="w-8 text-right font-mono text-xs text-text-muted">{pct()}</span>
    </div>
  );
}
