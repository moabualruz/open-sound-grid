/**
 * EQ Panel — uniform parametric EQ for all node types.
 * Same 10-band EQ, macros, and presets everywhere.
 * No position-based feature branching.
 */
import { createSignal, createEffect, Show, For, createMemo } from "solid-js";
import { Save } from "lucide-solid";
import type { EqBand } from "./math";
import { createDefaultBands, createDefaultBand } from "./math";
import type { EqConfig } from "../types";
import type { PresetDef } from "./presets";
import {
  getPresetsForCategory,
  DEFAULT_FAVORITES,
  BUILT_IN_PRESETS,
  getCustomPresets,
} from "./presets";
import EqGraph from "./EqGraph";
import EqBandPopup from "./EqBandPopup";
import EqMacroSliders from "./EqMacroSliders";
import PresetGallery from "./PresetGallery";
import PresetSaveDialog from "./PresetSaveDialog";
import { BAND_COLORS } from "./bandColors";

const MAX_BANDS = 10;
const FAVORITES_STORAGE_KEY = "osg-favorite-presets";

interface EqPanelProps {
  label: string;
  color?: string;
  readonly?: boolean;
  /** Initial EQ config from backend (loaded on mount). */
  initialEq?: EqConfig;
  /** Called whenever EQ config changes (bands, enabled). Debounced by the caller. */
  onEqChange?: (eq: EqConfig) => void;
  /** Category determines which presets appear in the dropdown. */
  category?: "app" | "mic" | "mix" | "cell";
}

/** Convert internal EqBand (with id/color) to serialized EqBand. */
function toSerializedBands(bands: EqBand[]): EqConfig["bands"] {
  return bands.map((b) => ({
    enabled: b.enabled,
    filterType: b.type,
    frequency: b.frequency,
    gain: b.gain,
    q: b.q,
  }));
}

/** Convert serialized EqConfig bands back to internal EqBand format. */
function fromSerializedBands(bands: EqConfig["bands"]): EqBand[] {
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
function buildMacroBands(bass: number, voice: number, treble: number): EqConfig["bands"] {
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

function loadFavoriteIds(): string[] {
  try {
    const stored = localStorage.getItem(FAVORITES_STORAGE_KEY);
    if (stored) return JSON.parse(stored) as string[];
  } catch {
    /* ignore parse errors */
  }
  return DEFAULT_FAVORITES;
}

export default function EqPanel(props: EqPanelProps) {
  const initBands = props.initialEq?.bands?.length
    ? fromSerializedBands(props.initialEq.bands)
    : createDefaultBands();
  const [enabled, setEnabled] = createSignal(props.initialEq?.enabled ?? true);
  const [bands, setBands] = createSignal<EqBand[]>(initBands);
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

  // Presets for this category — top 5 + favorites
  const availablePresets = createMemo(() => {
    const category = props.category ?? "app";
    const categoryPresets = getPresetsForCategory(category);
    const favoriteIds = loadFavoriteIds();

    // Favorites that belong to this category
    const favoritesInCategory = categoryPresets.filter((p) => favoriteIds.includes(p.id));
    // Non-favorite category presets
    const others = categoryPresets.filter((p) => !favoriteIds.includes(p.id));
    // Combine: favorites first, then fill to 5 from others
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
      setBands(fromSerializedBands(preset.eq.bands));
    } else {
      setBands(createDefaultBands());
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
      // Reset select to current value
      (e.target as HTMLSelectElement).value = lastPresetId() ?? "";
      return;
    }
    if (!value) return;
    const preset =
      BUILT_IN_PRESETS.find((p) => p.id === value) ??
      getCustomPresets().find((p) => p.id === value);
    if (preset) applyPreset(preset);
  };

  return (
    <div
      class="rounded-lg border overflow-hidden"
      style={{
        "border-color": props.color ?? "var(--color-border)",
        "background-color": "var(--color-bg-primary)",
      }}
    >
      {/* Header */}
      <div
        class="flex items-center justify-between px-3 py-1.5"
        style={{ "background-color": "var(--color-bg-elevated)" }}
      >
        <div class="flex items-center gap-2">
          <button
            class="w-8 h-4 rounded-full relative transition-colors"
            style={{
              "background-color": enabled()
                ? (props.color ?? "var(--color-accent)")
                : "var(--color-bg-hover)",
            }}
            onClick={() => setEnabled(!enabled())}
          >
            <div
              class="absolute top-0.5 w-3 h-3 rounded-full transition-all duration-150"
              style={{
                left: enabled() ? "17px" : "2px",
                "background-color": "var(--color-text-primary)",
              }}
            />
          </button>
          <span
            class="text-xs font-medium uppercase tracking-wide"
            style={{ color: "var(--color-text-secondary)" }}
          >
            {props.label}
          </span>
        </div>

        <div class="flex items-center gap-1.5">
          <Show when={canAddBand() && !props.readonly}>
            <button
              class="rounded px-1.5 py-0.5 text-[10px] transition-colors"
              style={{ color: "var(--color-accent)", background: "transparent" }}
              onClick={addBand}
              title={`Add band (${bands().length}/${MAX_BANDS})`}
            >
              + Band
            </button>
          </Show>
          <span class="text-[10px] font-mono" style={{ color: "var(--color-text-muted)" }}>
            {bands().length}/{MAX_BANDS}
          </span>

          {/* Preset dropdown */}
          <select
            class="rounded px-1.5 py-0.5 text-[10px]"
            style={{
              "background-color": "var(--color-bg-primary)",
              color: "var(--color-text-secondary)",
              border: "1px solid var(--color-border)",
            }}
            value={lastPresetId() ?? ""}
            onChange={handlePresetSelect}
          >
            <option value="">Preset...</option>
            <For each={availablePresets()}>
              {(preset) => (
                <option value={preset.id} title={preset.description}>
                  {preset.name}
                </option>
              )}
            </For>
            <option value="__more__">More...</option>
          </select>

          {/* Reset to last preset */}
          <Show when={lastPresetId()}>
            <button
              class="rounded px-1.5 py-0.5 text-[10px] transition-colors"
              style={{ color: "var(--color-text-muted)", background: "transparent" }}
              onClick={resetToLastPreset}
              title="Reset to last applied preset"
            >
              Reset
            </button>
          </Show>

          {/* Save as custom preset */}
          <button
            class="rounded px-1 py-0.5 transition-colors"
            style={{ color: "var(--color-text-muted)", background: "transparent" }}
            onClick={() => setShowSaveDialog(true)}
            title="Save as custom preset"
          >
            <Save size={11} />
          </button>

          {/* Import / Export */}
          <button
            class="rounded px-1.5 py-0.5 text-[10px] transition-colors"
            style={{ color: "var(--color-text-muted)", background: "transparent" }}
            onClick={() => {
              const data = JSON.stringify(
                { bands: bands(), bass: bass(), voice: voice(), treble: treble() },
                null,
                2,
              );
              const blob = new Blob([data], { type: "application/json" });
              const url = URL.createObjectURL(blob);
              const a = document.createElement("a");
              a.href = url;
              a.download = `eq-${props.label.toLowerCase().replace(/\s+/g, "-")}.json`;
              a.click();
              URL.revokeObjectURL(url);
            }}
            title="Export EQ preset"
          >
            ↓
          </button>
          <button
            class="rounded px-1.5 py-0.5 text-[10px] transition-colors"
            style={{ color: "var(--color-text-muted)", background: "transparent" }}
            onClick={() => {
              const input = document.createElement("input");
              input.type = "file";
              input.accept = ".json";
              input.onchange = () => {
                const file = input.files?.[0];
                if (!file) return;
                file.text().then((text) => {
                  const data = JSON.parse(text);
                  if (data.bands) setBands(data.bands);
                  if (data.bass != null) setBass(data.bass);
                  if (data.voice != null) setVoice(data.voice);
                  if (data.treble != null) setTreble(data.treble);
                });
              };
              input.click();
            }}
            title="Import EQ preset"
          >
            ↑
          </button>
          {/* Test sound */}
          <button
            class="rounded px-1.5 py-0.5 text-[10px] transition-colors"
            style={{ color: "var(--color-text-muted)", background: "transparent" }}
            title="Play test sound through this EQ"
          >
            ▶
          </button>
        </div>
      </div>

      {/* Preset gallery modal */}
      <Show when={showGallery()}>
        <PresetGallery
          category={props.category ?? "app"}
          onApply={(preset) => {
            applyPreset(preset);
            setShowGallery(false);
          }}
          onClose={() => setShowGallery(false)}
          favorites={favoritesSet()}
          onToggleFavorite={toggleFavorite}
        />
      </Show>

      {/* Save preset dialog */}
      <Show when={showSaveDialog()}>
        <PresetSaveDialog
          currentEq={{
            enabled: enabled(),
            bands: [...toSerializedBands(bands()), ...buildMacroBands(bass(), voice(), treble())],
          }}
          defaultCategory={props.category ?? "app"}
          onClose={() => setShowSaveDialog(false)}
          onSaved={(preset) => {
            setLastPresetId(preset.id);
            setShowSaveDialog(false);
          }}
        />
      </Show>

      {/* EQ Graph */}
      <div style={{ opacity: enabled() ? 1 : 0.3 }} class="transition-opacity duration-200">
        <EqGraph
          bands={bands()}
          selectedBandId={selectedBandId()}
          onBandMove={(id, freq, gain) => updateBand(id, { frequency: freq, gain })}
          onBandSelect={setSelectedBandId}
          onBandQChange={(id, q) => updateBand(id, { q })}
          readonly={props.readonly || !enabled()}
        />
      </div>

      {/* Band detail popup */}
      <Show when={enabled() ? selectedBand() : undefined}>
        {(band) => (
          <div class="px-2 py-1.5">
            <EqBandPopup
              band={band()}
              onToggleEnabled={(id) => updateBand(id, { enabled: !band().enabled })}
              onChangeType={(id, type) => updateBand(id, { type })}
              onChangeFreq={(id, freq) => updateBand(id, { frequency: freq })}
              onChangeGain={(id, gain) => updateBand(id, { gain })}
              onChangeQ={(id, q) => updateBand(id, { q })}
              onRemove={bands().length > 1 ? removeBand : undefined}
            />
          </div>
        )}
      </Show>

      {/* Macro sliders */}
      <Show when={enabled()}>
        <EqMacroSliders
          bass={bass()}
          voice={voice()}
          treble={treble()}
          onBassChange={setBass}
          onVoiceChange={setVoice}
          onTrebleChange={setTreble}
          readonly={props.readonly}
        />
      </Show>
    </div>
  );
}
