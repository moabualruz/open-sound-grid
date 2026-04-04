/**
 * useEqState — all reactive state for an EQ panel instance.
 * Manages bands, bypass toggle, macro sliders, preset tracking, and favorites.
 * Notifies parent via onEqChange whenever config changes.
 */
import { createSignal, createEffect, createMemo } from "solid-js";
import type { EqBand } from "./math";
import { createDefaultBands, createDefaultBand } from "./math";
import type { EqConfig } from "../types/eq";
import type { PresetDef } from "./presets";
import {
  getPresetsForCategory,
  DEFAULT_FAVORITES,
  BUILT_IN_PRESETS,
  getCustomPresets,
} from "./presets";
import { BAND_COLORS } from "./bandColors";

export const MAX_BANDS = 10;
const FAVORITES_STORAGE_KEY = "osg-favorite-presets";

/** Convert internal EqBand (with id/color) to serialized EqBand. */
export function toSerializedBands(bands: EqBand[]): EqConfig["bands"] {
  return bands.map((b) => ({
    enabled: b.enabled,
    filterType: b.type,
    frequency: b.frequency,
    gain: b.gain,
    q: b.q,
  }));
}

/** Convert serialized EqConfig bands back to internal EqBand format. */
export function fromSerializedBands(bands: EqConfig["bands"]): EqBand[] {
  return bands.map((b, i) => ({
    id: i,
    enabled: b.enabled,
    type: b.filterType as EqBand["type"],
    frequency: b.frequency,
    gain: b.gain,
    q: b.q,
    color: BAND_COLORS[i % BAND_COLORS.length],
  }));
}

/** Build macro bands from slider values. These are additional biquad bands. */
export function buildMacroBands(bass: number, voice: number, treble: number): EqConfig["bands"] {
  const macroBands: EqConfig["bands"] = [];
  if (bass !== 0) {
    macroBands.push({ enabled: true, filterType: "lowShelf", frequency: 200, gain: bass, q: 0.7 });
  }
  if (voice !== 0) {
    macroBands.push({ enabled: true, filterType: "peaking", frequency: 2500, gain: voice, q: 0.8 });
  }
  if (treble !== 0) {
    macroBands.push({
      enabled: true,
      filterType: "highShelf",
      frequency: 6000,
      gain: treble,
      q: 0.7,
    });
  }
  return macroBands;
}

export function loadFavoriteIds(): string[] {
  try {
    const stored = localStorage.getItem(FAVORITES_STORAGE_KEY);
    if (stored) return JSON.parse(stored) as string[];
  } catch {
    /* ignore parse errors */
  }
  return DEFAULT_FAVORITES;
}

export interface EqStateProps {
  initialEq?: EqConfig;
  category?: "app" | "mic" | "mix" | "cell";
  onEqChange?: (eq: EqConfig) => void;
}

export interface EqState {
  enabled: () => boolean;
  setEnabled: (v: boolean) => void;
  bands: () => EqBand[];
  setBands: (fn: (prev: EqBand[]) => EqBand[]) => void;
  selectedBandId: () => number | null;
  setSelectedBandId: (id: number | null) => void;
  bass: () => number;
  setBass: (v: number) => void;
  voice: () => number;
  setVoice: (v: number) => void;
  treble: () => number;
  setTreble: (v: number) => void;
  lastPresetId: () => string | null;
  setLastPresetId: (id: string | null) => void;
  showGallery: () => boolean;
  setShowGallery: (v: boolean) => void;
  showSaveDialog: () => boolean;
  setShowSaveDialog: (v: boolean) => void;
  favoriteIds: () => string[];
  favoritesSet: () => Set<string>;
  toggleFavorite: (id: string) => void;
  selectedBand: () => EqBand | undefined;
  canAddBand: () => boolean;
  availablePresets: () => PresetDef[];
  updateBand: (id: number, patch: Partial<EqBand>) => void;
  addBand: () => void;
  removeBand: (id: number) => void;
  applyPreset: (preset: PresetDef) => void;
  resetToLastPreset: () => void;
  handlePresetSelect: (e: Event) => void;
}

export function useEqState(props: EqStateProps): EqState {
  const initBands = props.initialEq?.bands?.length
    ? fromSerializedBands(props.initialEq.bands)
    : createDefaultBands();

  const [enabled, setEnabled] = createSignal(props.initialEq?.enabled ?? true);
  const [bands, setBandsRaw] = createSignal<EqBand[]>(initBands);
  const [selectedBandId, setSelectedBandId] = createSignal<number | null>(null);
  const [bass, setBass] = createSignal(0);
  const [voice, setVoice] = createSignal(0);
  const [treble, setTreble] = createSignal(0);
  const [lastPresetId, setLastPresetId] = createSignal<string | null>(null);
  const [showGallery, setShowGallery] = createSignal(false);
  const [showSaveDialog, setShowSaveDialog] = createSignal(false);
  const [favoriteIds, setFavoriteIds] = createSignal<string[]>(loadFavoriteIds());

  const favoritesSet = createMemo(() => new Set(favoriteIds()));

  const toggleFavorite = (id: string) => {
    const current = favoriteIds();
    const next = current.includes(id) ? current.filter((f) => f !== id) : [...current, id];
    setFavoriteIds(next);
    localStorage.setItem(FAVORITES_STORAGE_KEY, JSON.stringify(next));
  };

  const selectedBand = () => bands().find((b) => b.id === selectedBandId());
  const canAddBand = () => bands().length < MAX_BANDS;

  const availablePresets = createMemo(() => {
    const category = props.category ?? "app";
    const categoryPresets = getPresetsForCategory(category);
    const favIds = loadFavoriteIds();

    const favoritesInCategory = categoryPresets.filter((p) => favIds.includes(p.id));
    const others = categoryPresets.filter((p) => !favIds.includes(p.id));
    const combined = [...favoritesInCategory];
    for (const p of others) {
      if (combined.length >= 5) break;
      combined.push(p);
    }
    return combined;
  });

  // Notify parent whenever EQ config changes (includes macro bands)
  createEffect(() => {
    const userBands = toSerializedBands(bands());
    const macroBands = buildMacroBands(bass(), voice(), treble());
    const eq: EqConfig = { enabled: enabled(), bands: [...userBands, ...macroBands] };
    props.onEqChange?.(eq);
  });

  const setBands = (fn: (prev: EqBand[]) => EqBand[]) => setBandsRaw(fn);

  const updateBand = (id: number, patch: Partial<EqBand>) => {
    setBands((prev) => prev.map((b) => (b.id === id ? { ...b, ...patch } : b)));
  };

  const addBand = () => {
    if (!canAddBand()) return;
    const nextId = Math.max(0, ...bands().map((b) => b.id)) + 1;
    setBands((prev) => [...prev, createDefaultBand(nextId, 1000)]);
    setSelectedBandId(nextId);
  };

  const removeBand = (id: number) => {
    setBands((prev) => prev.filter((b) => b.id !== id));
    if (selectedBandId() === id) setSelectedBandId(null);
  };

  const applyPreset = (preset: PresetDef) => {
    if (preset.eq.bands.length > 0) {
      setBands(() => fromSerializedBands(preset.eq.bands));
    } else {
      setBands(() => createDefaultBands());
    }
    setEnabled(preset.eq.enabled);
    setLastPresetId(preset.id);
    setBass(0);
    setVoice(0);
    setTreble(0);
  };

  const resetToLastPreset = () => {
    const presetId = lastPresetId();
    if (!presetId) return;
    const preset =
      BUILT_IN_PRESETS.find((p) => p.id === presetId) ??
      getCustomPresets().find((p) => p.id === presetId);
    if (preset) applyPreset(preset);
  };

  const handlePresetSelect = (e: Event) => {
    const value = (e.target as HTMLSelectElement).value;
    if (value === "__more__") {
      setShowGallery(true);
      (e.target as HTMLSelectElement).value = lastPresetId() ?? "";
      return;
    }
    if (!value) return;
    const preset =
      BUILT_IN_PRESETS.find((p) => p.id === value) ??
      getCustomPresets().find((p) => p.id === value);
    if (preset) applyPreset(preset);
  };

  return {
    enabled,
    setEnabled,
    bands,
    setBands,
    selectedBandId,
    setSelectedBandId,
    bass,
    setBass,
    voice,
    setVoice,
    treble,
    setTreble,
    lastPresetId,
    setLastPresetId,
    showGallery,
    setShowGallery,
    showSaveDialog,
    setShowSaveDialog,
    favoriteIds,
    favoritesSet,
    toggleFavorite,
    selectedBand,
    canAddBand,
    availablePresets,
    updateBand,
    addBand,
    removeBand,
    applyPreset,
    resetToLastPreset,
    handlePresetSelect,
  };
}
