/**
 * Built-in EQ presets grouped by category.
 * Each preset uses exactly 5 bands for clean UI display.
 */
import type { EqConfig, EqBand as SerializedBand, FilterType } from "../types";

export interface PresetDef {
  id: string;
  name: string;
  category: "app" | "mic" | "mix" | "cell";
  description: string;
  eq: EqConfig;
}

/** Shorthand band constructor — defaults: enabled=true, q=0.707 */
function band(
  filterType: FilterType,
  frequency: number,
  gain: number = 0,
  q: number = 0.707,
): SerializedBand {
  return { enabled: true, filterType, frequency, gain, q };
}

// ---------------------------------------------------------------------------
// App presets (8)
// ---------------------------------------------------------------------------

const APP_PRESETS: PresetDef[] = [
  {
    id: "flat",
    name: "Flat",
    category: "app",
    description: "No processing — all bands at 0 dB",
    eq: {
      enabled: true,
      bands: [
        band("lowShelf", 80),
        band("peaking", 250),
        band("peaking", 1000),
        band("peaking", 3500),
        band("highShelf", 10000),
      ],
    },
  },
  {
    id: "voiceBoost",
    name: "Voice Boost",
    category: "app",
    description: "Presence boost for dialogue and vocals",
    eq: {
      enabled: true,
      bands: [
        band("highPass", 80),
        band("peaking", 200, -2),
        band("peaking", 1000, 1),
        band("peaking", 3000, 3),
        band("highShelf", 8000, 1.5),
      ],
    },
  },
  {
    id: "bassHeavy",
    name: "Bass Heavy",
    category: "app",
    description: "Boosted low end for music and games",
    eq: {
      enabled: true,
      bands: [
        band("lowShelf", 60, 6),
        band("peaking", 125, 4),
        band("peaking", 250, 1),
        band("peaking", 1000, -1),
        band("highShelf", 8000, 2),
      ],
    },
  },
  {
    id: "trebleBoost",
    name: "Treble Boost",
    category: "app",
    description: "Bright, airy top end",
    eq: {
      enabled: true,
      bands: [
        band("lowShelf", 80, -1),
        band("peaking", 2500, 2),
        band("peaking", 5000, 3),
        band("peaking", 5000, 3),
        band("highShelf", 10000, 4),
      ],
    },
  },
  {
    id: "gaming",
    name: "Gaming",
    category: "app",
    description: "Footsteps and spatial cues emphasized",
    eq: {
      enabled: true,
      bands: [
        band("lowShelf", 60, -3),
        band("peaking", 1500, 2),
        band("peaking", 4000, 4),
        band("peaking", 4000, 4),
        band("highShelf", 8000, 2),
      ],
    },
  },
  {
    id: "music",
    name: "Music",
    category: "app",
    description: "Balanced warm sound for music listening",
    eq: {
      enabled: true,
      bands: [
        band("lowShelf", 60, 3),
        band("peaking", 1000, -1),
        band("peaking", 4000, 2),
        band("peaking", 4000, 2),
        band("highShelf", 12000, 2),
      ],
    },
  },
  {
    id: "loFi",
    name: "Lo-Fi",
    category: "app",
    description: "Warm, muffled retro sound",
    eq: {
      enabled: true,
      bands: [
        band("lowShelf", 100, 3),
        band("peaking", 400, 2),
        band("peaking", 2000, -2),
        band("peaking", 5000, -4),
        band("lowPass", 12000),
      ],
    },
  },
  {
    id: "telephone",
    name: "Telephone",
    category: "app",
    description: "Narrow bandpass for radio/phone effect",
    eq: {
      enabled: true,
      bands: [
        band("highPass", 300),
        band("peaking", 800, 2),
        band("peaking", 2000, 3),
        band("peaking", 2000, 3),
        band("lowPass", 3400),
      ],
    },
  },
];

// ---------------------------------------------------------------------------
// Mic presets (5)
// ---------------------------------------------------------------------------

const MIC_PRESETS: PresetDef[] = [
  {
    id: "broadcastVoice",
    name: "Broadcast Voice",
    category: "mic",
    description: "Radio-style clarity with presence lift",
    eq: {
      enabled: true,
      bands: [
        band("highPass", 80),
        band("peaking", 200, -2.5),
        band("peaking", 900, -1),
        band("peaking", 3500, 3),
        band("highShelf", 10000, -1),
      ],
    },
  },
  {
    id: "podcastVoice",
    name: "Podcast Voice",
    category: "mic",
    description: "Warm, intimate podcast sound",
    eq: {
      enabled: true,
      bands: [
        band("highPass", 100),
        band("peaking", 250, 1.5),
        band("peaking", 800, -1.5),
        band("peaking", 3000, 2.5),
        band("peaking", 6000, -2),
      ],
    },
  },
  {
    id: "streamingVoice",
    name: "Streaming Voice",
    category: "mic",
    description: "Cut-through presence for live streams",
    eq: {
      enabled: true,
      bands: [
        band("highPass", 90),
        band("peaking", 300, -3),
        band("peaking", 2500, 3.5),
        band("peaking", 5000, 1.5),
        band("highShelf", 12000, -1.5),
      ],
    },
  },
  {
    id: "deEsser",
    name: "De-Esser",
    category: "mic",
    description: "Reduce sibilance (s/t/sh sounds)",
    eq: {
      enabled: true,
      bands: [
        band("peaking", 5500, -6, 3),
        band("peaking", 7500, -4, 2.5),
        band("peaking", 9000, -2, 2),
        band("peaking", 9000, -2, 2),
        band("peaking", 9000, -2, 2),
      ],
    },
  },
  {
    id: "proximityFix",
    name: "Proximity Fix",
    category: "mic",
    description: "Counteract proximity effect bass buildup",
    eq: {
      enabled: true,
      bands: [
        band("highPass", 120),
        band("peaking", 200, -4),
        band("peaking", 400, -1.5),
        band("peaking", 400, -1.5),
        band("peaking", 400, -1.5),
      ],
    },
  },
];

// ---------------------------------------------------------------------------
// Mix presets (4)
// ---------------------------------------------------------------------------

const MIX_PRESETS: PresetDef[] = [
  {
    id: "referenceFlat",
    name: "Reference Flat",
    category: "mix",
    description: "No processing — transparent pass-through",
    eq: { enabled: true, bands: [] },
  },
  {
    id: "streamMaster",
    name: "Stream Master",
    category: "mix",
    description: "Broadcast-ready stream output polish",
    eq: {
      enabled: true,
      bands: [
        band("highPass", 30),
        band("peaking", 150, -1),
        band("peaking", 3000, 1.5),
        band("peaking", 3000, 1.5),
        band("highShelf", 14000, -1),
      ],
    },
  },
  {
    id: "headphoneMonitor",
    name: "Headphone Monitor",
    category: "mix",
    description: "Tamed highs for extended listening comfort",
    eq: {
      enabled: true,
      bands: [
        band("lowShelf", 80, -2),
        band("peaking", 2500, -1.5),
        band("peaking", 5000, 1),
        band("peaking", 5000, 1),
        band("highShelf", 10000, -1),
      ],
    },
  },
  {
    id: "bassBoostMonitor",
    name: "Bass Boost Monitor",
    category: "mix",
    description: "Heavy sub-bass boost for monitoring",
    eq: {
      enabled: true,
      bands: [
        band("lowShelf", 100, 5),
        band("peaking", 60, 3),
        band("peaking", 250, 1),
        band("peaking", 250, 1),
        band("peaking", 250, 1),
      ],
    },
  },
];

// ---------------------------------------------------------------------------
// Cell presets (reuse app presets where applicable) — 11 presets
// ---------------------------------------------------------------------------

const CELL_PRESETS: PresetDef[] = [
  {
    id: "cellFlat",
    name: "Flat",
    category: "cell",
    description: "No per-route processing",
    eq: {
      enabled: true,
      bands: [
        band("lowShelf", 80),
        band("peaking", 250),
        band("peaking", 1000),
        band("peaking", 3500),
        band("highShelf", 10000),
      ],
    },
  },
  {
    id: "cellVoiceBoost",
    name: "Voice Boost",
    category: "cell",
    description: "Presence boost for this route",
    eq: {
      enabled: true,
      bands: [
        band("highPass", 80),
        band("peaking", 200, -2),
        band("peaking", 1000, 1),
        band("peaking", 3000, 3),
        band("highShelf", 8000, 1.5),
      ],
    },
  },
  {
    id: "cellBassHeavy",
    name: "Bass Heavy",
    category: "cell",
    description: "Boosted low end for this route",
    eq: {
      enabled: true,
      bands: [
        band("lowShelf", 60, 6),
        band("peaking", 125, 4),
        band("peaking", 250, 1),
        band("peaking", 1000, -1),
        band("highShelf", 8000, 2),
      ],
    },
  },
  {
    id: "cellTrebleBoost",
    name: "Treble Boost",
    category: "cell",
    description: "Bright top end for this route",
    eq: {
      enabled: true,
      bands: [
        band("lowShelf", 80, -1),
        band("peaking", 2500, 2),
        band("peaking", 5000, 3),
        band("peaking", 5000, 3),
        band("highShelf", 10000, 4),
      ],
    },
  },
  {
    id: "cellGaming",
    name: "Gaming",
    category: "cell",
    description: "Spatial cues for this route",
    eq: {
      enabled: true,
      bands: [
        band("lowShelf", 60, -3),
        band("peaking", 1500, 2),
        band("peaking", 4000, 4),
        band("peaking", 4000, 4),
        band("highShelf", 8000, 2),
      ],
    },
  },
  {
    id: "cellMusic",
    name: "Music",
    category: "cell",
    description: "Balanced warm sound for this route",
    eq: {
      enabled: true,
      bands: [
        band("lowShelf", 60, 3),
        band("peaking", 1000, -1),
        band("peaking", 4000, 2),
        band("peaking", 4000, 2),
        band("highShelf", 12000, 2),
      ],
    },
  },
];

// ---------------------------------------------------------------------------
// Combined + helpers
// ---------------------------------------------------------------------------

export const BUILT_IN_PRESETS: PresetDef[] = [
  ...APP_PRESETS,
  ...MIC_PRESETS,
  ...MIX_PRESETS,
  ...CELL_PRESETS,
];

export const DEFAULT_FAVORITES = [
  "flat",
  "voiceBoost",
  "bassHeavy",
  "broadcastVoice",
  "streamMaster",
];

export function getPresetsForCategory(category: PresetDef["category"]): PresetDef[] {
  return [
    ...BUILT_IN_PRESETS.filter((p) => p.category === category),
    ...getCustomPresets(category),
  ];
}

// ---------------------------------------------------------------------------
// Custom presets (localStorage)
// ---------------------------------------------------------------------------

const CUSTOM_PRESETS_KEY = "osg-custom-presets";

export function getCustomPresets(category?: PresetDef["category"]): PresetDef[] {
  try {
    const stored = localStorage.getItem(CUSTOM_PRESETS_KEY);
    if (!stored) return [];
    const all = JSON.parse(stored) as PresetDef[];
    return category ? all.filter((p) => p.category === category) : all;
  } catch {
    return [];
  }
}

export function saveCustomPreset(preset: PresetDef): void {
  const existing = getCustomPresets();
  const idx = existing.findIndex((p) => p.id === preset.id);
  if (idx >= 0) {
    existing[idx] = preset;
  } else {
    existing.push(preset);
  }
  localStorage.setItem(CUSTOM_PRESETS_KEY, JSON.stringify(existing));
}

export function deleteCustomPreset(id: string): void {
  const existing = getCustomPresets().filter((p) => p.id !== id);
  localStorage.setItem(CUSTOM_PRESETS_KEY, JSON.stringify(existing));
}
