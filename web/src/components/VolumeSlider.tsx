import { createEffect, createSignal, onCleanup } from "solid-js";

interface VolumeSliderProps {
  value: number;
  muted?: boolean;
  orientation?: "horizontal" | "vertical";
  onChange: (value: number) => void;
}

const DEBOUNCE_MS = 16;

export default function VolumeSlider(props: VolumeSliderProps) {
  const [local, setLocal] = createSignal(0);
  let debounceTimer: ReturnType<typeof setTimeout> | null = null;

  // Sync from server when props change (e.g. another client sets volume)
  createEffect(() => setLocal(props.value));

  function handleInput(value: number) {
    setLocal(value);
    if (debounceTimer) clearTimeout(debounceTimer);
    debounceTimer = setTimeout(() => props.onChange(value), DEBOUNCE_MS);
  }

  onCleanup(() => {
    if (debounceTimer) clearTimeout(debounceTimer);
  });

  const isVertical = () => props.orientation === "vertical";

  return (
    <div class={`flex items-center gap-2 ${isVertical() ? "h-24 flex-col-reverse" : "w-full"}`}>
      <input
        type="range"
        min="0"
        max="1"
        step="0.01"
        value={local()}
        onInput={(e) => handleInput(parseFloat(e.currentTarget.value))}
        class={`accent-accent ${isVertical() ? "h-20 -rotate-90" : "w-full"} ${props.muted ? "opacity-40" : ""}`}
      />
      <span class="text-text-muted w-8 text-center font-mono text-xs">
        {Math.round(local() * 100)}
      </span>
    </div>
  );
}
