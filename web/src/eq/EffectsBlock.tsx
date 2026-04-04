/**
 * Effects blocks — non-EQ processing that varies by source type.
 *
 * Source type determines which effects are available:
 * ┌───────────────┬────────────┬───────┬──────┬──────┐
 * │ Effect        │ App/Device │ Cell  │ Mix  │ Mic  │
 * ├───────────────┼────────────┼───────┼──────┼──────┤
 * │ Noise Gate    │ —          │ —     │ —    │ YES  │
 * │ De-Esser      │ —          │ —     │ —    │ YES  │
 * │ Compressor    │ YES        │ YES   │ YES  │ YES  │
 * │ Limiter       │ —          │ —     │ YES  │ —    │
 * │ Smart Volume  │ YES        │ —     │ YES  │ —    │
 * │ Volume Boost  │ YES        │ YES   │ YES  │ YES  │
 * │ Spatial Audio │ —          │ —     │ YES  │ —    │
 * └───────────────┴────────────┴───────┴──────┴──────┘
 */
import { createSignal, Show, For, untrack } from "solid-js";
import type { EffectsConfig } from "../types";

export type SourceType = "app" | "cell" | "mix" | "mic";

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
  /** True for effects without backend DSP — toggle is disabled. */
  comingSoon?: boolean;
}

const EFFECTS: EffectDef[] = [
  {
    id: "gate",
    label: "Noise Gate",
    description: "Silences signal below threshold — removes background noise",
    availableOn: ["mic"],
    controls: [
      {
        id: "threshold",
        label: "Threshold",
        min: -80,
        max: -20,
        step: 1,
        defaultValue: -60,
        unit: "dB",
      },
      {
        id: "hold",
        label: "Hold",
        min: 10,
        max: 500,
        step: 10,
        defaultValue: 100,
        unit: "ms",
      },
      {
        id: "attack",
        label: "Attack",
        min: 0.1,
        max: 10,
        step: 0.1,
        defaultValue: 0.5,
        unit: "ms",
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
    id: "deEsser",
    label: "De-Esser",
    description: "Reduces sibilance (harsh s/t/sh sounds)",
    availableOn: ["mic"],
    controls: [
      {
        id: "frequency",
        label: "Frequency",
        min: 4000,
        max: 10000,
        step: 100,
        defaultValue: 6000,
        unit: "Hz",
      },
      {
        id: "threshold",
        label: "Threshold",
        min: -40,
        max: 0,
        step: 1,
        defaultValue: -20,
        unit: "dB",
      },
      {
        id: "reduction",
        label: "Reduction",
        min: -12,
        max: 0,
        step: 0.5,
        defaultValue: -6,
        unit: "dB",
      },
    ],
  },
  {
    id: "compressor",
    label: "Compressor",
    description: "Reduces dynamic range — evens out loud and quiet",
    availableOn: ["app", "cell", "mix", "mic"],
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
    comingSoon: true,
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
    availableOn: ["app", "cell", "mix", "mic"],
    controls: [
      { id: "boost", label: "Boost", min: 0, max: 12, step: 0.5, defaultValue: 0, unit: "dB" },
    ],
  },
  {
    id: "spatialAudio",
    label: "Spatial Audio",
    description: "HRTF virtual 7.1 surround for headphones",
    comingSoon: true,
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
  initialEffects?: EffectsConfig;
  onEffectsChange?: (effects: EffectsConfig) => void;
}

/** Default EffectsConfig matching osg-core defaults. */
function defaultEffectsConfig(): EffectsConfig {
  return {
    compressor: { enabled: false, threshold: -20, ratio: 4, attack: 10, release: 100, makeup: 0 },
    gate: { enabled: false, threshold: -60, hold: 100, attack: 0.5, release: 50 },
    deEsser: { enabled: false, frequency: 6000, threshold: -20, reduction: -6 },
    limiter: { enabled: false, ceiling: -0.3, release: 50 },
    boost: 0,
  };
}

/** Build an EffectsConfig from the current card states. */
function buildEffectsConfig(
  cardStates: Map<string, { enabled: boolean; values: Record<string, number> }>,
  base: EffectsConfig,
): EffectsConfig {
  const config = { ...base };

  const comp = cardStates.get("compressor");
  if (comp) {
    config.compressor = {
      enabled: comp.enabled,
      threshold: comp.values.threshold ?? base.compressor.threshold,
      ratio: comp.values.ratio ?? base.compressor.ratio,
      attack: comp.values.attack ?? base.compressor.attack,
      release: comp.values.release ?? base.compressor.release,
      makeup: base.compressor.makeup,
    };
  }

  const lim = cardStates.get("limiter");
  if (lim) {
    config.limiter = {
      enabled: lim.enabled,
      ceiling: lim.values.ceiling ?? base.limiter.ceiling,
      release: lim.values.release ?? base.limiter.release,
    };
  }

  const gate = cardStates.get("gate");
  if (gate) {
    config.gate = {
      enabled: gate.enabled,
      threshold: gate.values.threshold ?? base.gate.threshold,
      hold: gate.values.hold ?? base.gate.hold,
      attack: gate.values.attack ?? base.gate.attack,
      release: gate.values.release ?? base.gate.release,
    };
  }

  const deEsser = cardStates.get("deEsser");
  if (deEsser) {
    config.deEsser = {
      enabled: deEsser.enabled,
      frequency: deEsser.values.frequency ?? base.deEsser.frequency,
      threshold: deEsser.values.threshold ?? base.deEsser.threshold,
      reduction: deEsser.values.reduction ?? base.deEsser.reduction,
    };
  }

  const boost = cardStates.get("volumeBoost");
  if (boost) {
    config.boost = boost.enabled ? (boost.values.boost ?? 0) : 0;
  }

  return config;
}

export default function EffectsBlock(props: EffectsBlockProps) {
  const effects = () => getEffectsForType(props.sourceType);
  const cardStates = new Map<string, { enabled: boolean; values: Record<string, number> }>();
  const baseConfig = () => props.initialEffects ?? defaultEffectsConfig();

  /** Effect IDs that have backend DSP mapping. */
  const MAPPED_EFFECTS = new Set(["compressor", "limiter", "deEsser", "gate", "volumeBoost"]);

  function handleCardChange(effectId: string, enabled: boolean, values: Record<string, number>) {
    cardStates.set(effectId, { enabled, values });
    if (MAPPED_EFFECTS.has(effectId)) {
      props.onEffectsChange?.(buildEffectsConfig(cardStates, baseConfig()));
    }
  }

  return (
    <Show when={effects().length > 0}>
      <div class="flex flex-col gap-2">
        <For each={effects()}>
          {(effect) => (
            <EffectCard
              effect={effect}
              color={props.color}
              initialEffects={props.initialEffects}
              onChange={(enabled, values) => handleCardChange(effect.id, enabled, values)}
            />
          )}
        </For>
      </div>
    </Show>
  );
}

interface EffectCardProps {
  effect: EffectDef;
  color?: string;
  initialEffects?: EffectsConfig;
  onChange?: (enabled: boolean, values: Record<string, number>) => void;
}

/** Extract initial values for an effect from EffectsConfig. */
function getInitialFromConfig(
  effectId: string,
  config: EffectsConfig | undefined,
): { enabled: boolean; values: Record<string, number> } | null {
  if (!config) return null;
  if (effectId === "gate") {
    const g = config.gate;
    return {
      enabled: g.enabled,
      values: { threshold: g.threshold, hold: g.hold, attack: g.attack, release: g.release },
    };
  }
  if (effectId === "compressor") {
    const c = config.compressor;
    return {
      enabled: c.enabled,
      values: { threshold: c.threshold, ratio: c.ratio, attack: c.attack, release: c.release },
    };
  }
  if (effectId === "limiter") {
    const l = config.limiter;
    return { enabled: l.enabled, values: { ceiling: l.ceiling, release: l.release } };
  }
  if (effectId === "deEsser") {
    const d = config.deEsser;
    return {
      enabled: d.enabled,
      values: { frequency: d.frequency, threshold: d.threshold, reduction: d.reduction },
    };
  }
  if (effectId === "volumeBoost") {
    const b = config.boost ?? 0;
    return { enabled: b > 0, values: { boost: b } };
  }
  return null;
}

function EffectCard(props: EffectCardProps) {
  const isComingSoon = () => props.effect.comingSoon === true;
  const initial = untrack(() => getInitialFromConfig(props.effect.id, props.initialEffects));
  const [enabled, setEnabled] = createSignal(isComingSoon() ? false : (initial?.enabled ?? false));
  const [values, setValues] = createSignal<Record<string, number>>(
    untrack(() => {
      const defaults = Object.fromEntries(props.effect.controls.map((c) => [c.id, c.defaultValue]));
      return initial?.values ? { ...defaults, ...initial.values } : defaults;
    }),
  );
  const [options, setOptions] = createSignal<Record<string, string>>(
    untrack(() =>
      Object.fromEntries((props.effect.options ?? []).map((o) => [o.id, o.defaultValue])),
    ),
  );

  const updateValue = (controlId: string, value: number) => {
    setValues((prev) => {
      const next = { ...prev, [controlId]: value };
      props.onChange?.(enabled(), next);
      return next;
    });
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
              opacity: isComingSoon() ? 0.4 : 1,
              cursor: isComingSoon() ? "not-allowed" : "pointer",
            }}
            disabled={isComingSoon()}
            onClick={() => {
              if (isComingSoon()) return;
              const next = !enabled();
              setEnabled(next);
              props.onChange?.(next, values());
            }}
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
            <Show when={isComingSoon()}>
              <span
                class="ml-1 text-[8px] normal-case"
                style={{ color: "var(--color-text-muted)" }}
              >
                (Coming Soon)
              </span>
            </Show>
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
