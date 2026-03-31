/**
 * Effects blocks — non-EQ processing that varies by source type.
 *
 * Source type determines which effects are available:
 * ┌───────────────────────┬──────┬────────────┬───────┬──────┐
 * │ Effect                │ Mic  │ App/Device │ Cell  │ Mix  │
 * ├───────────────────────┼──────┼────────────┼───────┼──────┤
 * │ Background Noise      │ YES  │ —          │ —     │ —    │
 * │ Impact Noise          │ YES  │ —          │ —     │ —    │
 * │ AI Noise Cancellation │ YES  │ —          │ —     │ —    │
 * │ Noise Gate            │ YES  │ —          │ —     │ —    │
 * │ Compressor            │ YES  │ YES        │ YES   │ YES  │
 * │ Limiter               │ —    │ —          │ —     │ YES  │
 * │ Smart Volume          │ —    │ YES        │ —     │ YES  │
 * │ Volume Boost          │ —    │ YES        │ YES   │ YES  │
 * │ Spatial Audio         │ —    │ —          │ —     │ YES  │
 * └───────────────────────┴──────┴────────────┴───────┴──────┘
 */
import { createSignal, Show, For } from "solid-js";

export type SourceType = "mic" | "app" | "cell" | "mix";

interface ControlDef {
  id: string;
  label: string;
  min: number;
  max: number;
  step: number;
  defaultValue: number;
  unit: string;
}

interface OptionDef {
  id: string;
  label: string;
  options: string[];
  defaultValue: string;
}

interface EffectDef {
  id: string;
  label: string;
  description: string;
  availableOn: SourceType[];
  controls: ControlDef[];
  options?: OptionDef[];
  hasTestSound?: boolean;
}

const EFFECTS: EffectDef[] = [
  {
    id: "bgNoise",
    label: "Background Noise",
    description: "Reduces constant ambient noise — fans, AC, hum",
    availableOn: ["mic"],
    controls: [
      { id: "level", label: "Level", min: 0, max: 100, step: 1, defaultValue: 0, unit: "%" },
    ],
  },
  {
    id: "impactNoise",
    label: "Impact Noise",
    description: "Reduces keyboard clicks, mouse taps, bumps",
    availableOn: ["mic"],
    controls: [
      { id: "level", label: "Level", min: 0, max: 100, step: 1, defaultValue: 50, unit: "%" },
    ],
  },
  {
    id: "aiNoiseCancellation",
    label: "AI Noise Cancellation",
    description: "Deep learning noise removal — replaces manual controls with one smart slider",
    availableOn: ["mic"],
    controls: [
      { id: "level", label: "Intensity", min: 0, max: 100, step: 1, defaultValue: 70, unit: "%" },
    ],
  },
  {
    id: "noiseGate",
    label: "Noise Gate",
    description: "Silences input below threshold — cuts dead-air noise",
    availableOn: ["mic"],
    controls: [
      {
        id: "threshold",
        label: "Threshold",
        min: -60,
        max: 0,
        step: 1,
        defaultValue: -40,
        unit: "dB",
      },
      { id: "hold", label: "Hold", min: 0, max: 500, step: 10, defaultValue: 100, unit: "ms" },
    ],
  },
  {
    id: "compressor",
    label: "Compressor",
    description: "Reduces dynamic range — evens out loud and quiet",
    availableOn: ["mic", "app", "cell", "mix"],
    controls: [
      {
        id: "threshold",
        label: "Threshold",
        min: -60,
        max: 0,
        step: 1,
        defaultValue: -20,
        unit: "dB",
      },
      { id: "ratio", label: "Ratio", min: 1, max: 20, step: 0.5, defaultValue: 4, unit: ":1" },
      {
        id: "attack",
        label: "Attack",
        min: 0.1,
        max: 100,
        step: 0.1,
        defaultValue: 10,
        unit: "ms",
      },
      {
        id: "release",
        label: "Release",
        min: 10,
        max: 1000,
        step: 10,
        defaultValue: 100,
        unit: "ms",
      },
    ],
  },
  {
    id: "limiter",
    label: "Limiter",
    description: "Hard ceiling — prevents clipping on the output bus",
    availableOn: ["mix"],
    controls: [
      {
        id: "ceiling",
        label: "Ceiling",
        min: -12,
        max: 0,
        step: 0.1,
        defaultValue: -0.3,
        unit: "dB",
      },
      {
        id: "release",
        label: "Release",
        min: 10,
        max: 500,
        step: 10,
        defaultValue: 50,
        unit: "ms",
      },
    ],
  },
  {
    id: "smartVolume",
    label: "Smart Volume",
    description: "Loudness normalization — keeps all sources at similar perceived volume",
    availableOn: ["app", "mix"],
    controls: [
      {
        id: "target",
        label: "Target",
        min: -30,
        max: -10,
        step: 1,
        defaultValue: -18,
        unit: "LUFS",
      },
    ],
  },
  {
    id: "volumeBoost",
    label: "Volume Boost",
    description: "Extra gain amplification beyond 100%",
    availableOn: ["app", "cell", "mix"],
    controls: [
      { id: "boost", label: "Boost", min: 0, max: 12, step: 0.5, defaultValue: 0, unit: "dB" },
    ],
  },
  {
    id: "spatialAudio",
    label: "Spatial Audio",
    description: "HRTF virtual 7.1 surround for headphones",
    availableOn: ["mix"],
    controls: [
      { id: "distance", label: "Distance", min: 0, max: 100, step: 1, defaultValue: 50, unit: "" },
    ],
    options: [
      {
        id: "mode",
        label: "Mode",
        options: ["Performance", "Immersive"],
        defaultValue: "Performance",
      },
      {
        id: "output",
        label: "Output",
        options: ["Headphone", "Speaker"],
        defaultValue: "Headphone",
      },
    ],
  },
];

/** Get available effects for a source type. */
export function getEffectsForType(type: SourceType): EffectDef[] {
  return EFFECTS.filter((e) => e.availableOn.includes(type));
}

// ---------------------------------------------------------------------------
// Components
// ---------------------------------------------------------------------------

interface EffectsBlockProps {
  sourceType: SourceType;
  color?: string;
}

export default function EffectsBlock(props: EffectsBlockProps) {
  const effects = () => getEffectsForType(props.sourceType);

  return (
    <Show when={effects().length > 0}>
      <div class="flex flex-col gap-2">
        <For each={effects()}>{(effect) => <EffectCard effect={effect} color={props.color} />}</For>
      </div>
    </Show>
  );
}

function EffectCard(props: { effect: EffectDef; color?: string }) {
  const [enabled, setEnabled] = createSignal(false);
  const [values, setValues] = createSignal<Record<string, number>>(
    Object.fromEntries(props.effect.controls.map((c) => [c.id, c.defaultValue])),
  );
  const [options, setOptions] = createSignal<Record<string, string>>(
    Object.fromEntries((props.effect.options ?? []).map((o) => [o.id, o.defaultValue])),
  );

  const updateValue = (controlId: string, value: number) => {
    setValues((prev) => ({ ...prev, [controlId]: value }));
  };

  const updateOption = (optionId: string, value: string) => {
    setOptions((prev) => ({ ...prev, [optionId]: value }));
  };

  return (
    <div
      class="rounded-lg border overflow-hidden"
      style={{
        "border-color": enabled() ? (props.color ?? "var(--color-accent)") : "var(--color-border)",
        "background-color": "var(--color-bg-primary)",
        opacity: enabled() ? 1 : 0.7,
      }}
    >
      {/* Header */}
      <div
        class="flex items-center justify-between px-3 py-1.5"
        style={{ "background-color": "var(--color-bg-elevated)" }}
      >
        <div class="flex items-center gap-2">
          <button
            class="w-7 h-3.5 rounded-full relative transition-colors"
            style={{
              "background-color": enabled()
                ? (props.color ?? "var(--color-accent)")
                : "var(--color-bg-hover)",
            }}
            onClick={() => setEnabled(!enabled())}
          >
            <div
              class="absolute top-0.5 w-2.5 h-2.5 rounded-full transition-all duration-150"
              style={{
                left: enabled() ? "15px" : "2px",
                "background-color": "var(--color-text-primary)",
              }}
            />
          </button>
          <span
            class="text-[11px] font-medium uppercase tracking-wide"
            style={{
              color: enabled() ? "var(--color-text-secondary)" : "var(--color-text-muted)",
            }}
          >
            {props.effect.label}
          </span>
        </div>
        <span
          class="text-[9px] max-w-[50%] text-right"
          style={{ color: "var(--color-text-muted)" }}
        >
          {props.effect.description}
        </span>
      </div>

      {/* Controls */}
      <Show when={enabled()}>
        <div class="px-3 py-2 flex flex-col gap-2">
          {/* Option selectors (e.g. Spatial Audio mode) */}
          <Show when={props.effect.options && props.effect.options.length > 0}>
            <div class="flex flex-wrap gap-3">
              <For each={props.effect.options ?? []}>
                {(opt) => (
                  <div class="flex items-center gap-2">
                    <span class="text-[10px]" style={{ color: "var(--color-text-muted)" }}>
                      {opt.label}
                    </span>
                    <div
                      class="flex rounded overflow-hidden border"
                      style={{ "border-color": "var(--color-border)" }}
                    >
                      <For each={opt.options}>
                        {(val) => (
                          <button
                            class="px-2 py-0.5 text-[10px] font-medium transition-colors"
                            style={{
                              "background-color":
                                options()[opt.id] === val
                                  ? (props.color ?? "var(--color-accent)")
                                  : "var(--color-bg-primary)",
                              color:
                                options()[opt.id] === val
                                  ? "var(--color-text-primary)"
                                  : "var(--color-text-muted)",
                            }}
                            onClick={() => updateOption(opt.id, val)}
                          >
                            {val}
                          </button>
                        )}
                      </For>
                    </div>
                  </div>
                )}
              </For>
            </div>
          </Show>

          {/* Slider controls */}
          <div class="flex flex-wrap gap-x-4 gap-y-2">
            <For each={props.effect.controls}>
              {(ctrl) => (
                <div class="flex items-center gap-2 min-w-[180px] flex-1">
                  <span
                    class="text-[10px] w-16 text-right shrink-0"
                    style={{ color: "var(--color-text-muted)" }}
                  >
                    {ctrl.label}
                  </span>
                  <input
                    type="range"
                    min={ctrl.min}
                    max={ctrl.max}
                    step={ctrl.step}
                    value={values()[ctrl.id]}
                    class="flex-1 min-w-0"
                    onInput={(e) => updateValue(ctrl.id, parseFloat(e.currentTarget.value))}
                  />
                  <span
                    class="text-[10px] font-mono tabular-nums w-16 shrink-0"
                    style={{ color: "var(--color-text-secondary)" }}
                  >
                    {values()[ctrl.id]?.toFixed(ctrl.step < 1 ? 1 : 0)} {ctrl.unit}
                  </span>
                </div>
              )}
            </For>
          </div>
        </div>
      </Show>
    </div>
  );
}
