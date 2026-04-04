import type { EffectsConfig } from "../types/effects";

export type SourceType = "app" | "cell" | "mix" | "mic";

export interface ControlDef {
  id: string;
  label: string;
  min: number;
  max: number;
  step: number;
  defaultValue: number;
  unit: string;
}

export interface OptionDef {
  id: string;
  label: string;
  options: string[];
  defaultValue: string;
}

export interface EffectDef {
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

export const EFFECTS: EffectDef[] = [
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
    availableOn: ["app", "mix"],
    controls: [
      {
        id: "targetDb",
        label: "Target Level",
        min: -30,
        max: -6,
        step: 1,
        defaultValue: -18,
        unit: "dB",
      },
      {
        id: "speed",
        label: "Speed",
        min: 0,
        max: 100,
        step: 1,
        defaultValue: 30,
        unit: "%",
      },
      {
        id: "maxGainDb",
        label: "Max Gain",
        min: 0,
        max: 24,
        step: 1,
        defaultValue: 12,
        unit: "dB",
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
    description: "Bauer crossfeed + stereo width for headphone listening",
    availableOn: ["mix"],
    controls: [
      {
        id: "crossfeed",
        label: "Crossfeed",
        min: 0,
        max: 100,
        step: 1,
        defaultValue: 30,
        unit: "%",
      },
      {
        id: "width",
        label: "Width",
        min: 0,
        max: 200,
        step: 1,
        defaultValue: 100,
        unit: "%",
      },
    ],
  },
];

/** Effect IDs that have backend DSP mapping. */
export const MAPPED_EFFECTS = new Set([
  "compressor",
  "limiter",
  "deEsser",
  "gate",
  "volumeBoost",
  "smartVolume",
  "spatialAudio",
]);

/** Get available effects for a source type. */
export function getEffectsForType(type: SourceType): EffectDef[] {
  return EFFECTS.filter((e) => e.availableOn.includes(type));
}

/** Default EffectsConfig matching osg-core defaults. */
export function defaultEffectsConfig(): EffectsConfig {
  return {
    compressor: { enabled: false, threshold: -20, ratio: 4, attack: 10, release: 100, makeup: 0 },
    gate: { enabled: false, threshold: -60, hold: 100, attack: 0.5, release: 50 },
    deEsser: { enabled: false, frequency: 6000, threshold: -20, reduction: -6 },
    limiter: { enabled: false, ceiling: -0.3, release: 50 },
    boost: 0,
    smartVolume: { enabled: false, targetDb: -18, speed: 0.3, maxGainDb: 12 },
    spatial: { enabled: false, crossfeed: 0.3, width: 1.0 },
  };
}

/** Build an EffectsConfig from the current card states. */
export function buildEffectsConfig(
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

  const sv = cardStates.get("smartVolume");
  if (sv) {
    config.smartVolume = {
      enabled: sv.enabled,
      targetDb: sv.values.targetDb ?? base.smartVolume.targetDb,
      speed: (sv.values.speed ?? 30) / 100, // UI 0–100% → wire 0.0–1.0
      maxGainDb: sv.values.maxGainDb ?? base.smartVolume.maxGainDb,
    };
  }

  const spatial = cardStates.get("spatialAudio");
  if (spatial) {
    config.spatial = {
      enabled: spatial.enabled,
      crossfeed: (spatial.values.crossfeed ?? 30) / 100, // UI 0–100% → wire 0.0–1.0
      width: (spatial.values.width ?? 100) / 100, // UI 0–200% → wire 0.0–2.0
    };
  }

  return config;
}

/** Extract initial values for an effect from EffectsConfig. */
export function getInitialFromConfig(
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
  if (effectId === "smartVolume") {
    const sv = config.smartVolume;
    if (!sv) return null;
    return {
      enabled: sv.enabled,
      values: { targetDb: sv.targetDb, speed: sv.speed * 100, maxGainDb: sv.maxGainDb },
    };
  }
  if (effectId === "spatialAudio") {
    const sp = config.spatial;
    if (!sp) return null;
    return {
      enabled: sp.enabled,
      values: { crossfeed: sp.crossfeed * 100, width: sp.width * 100 },
    };
  }
  return null;
}
