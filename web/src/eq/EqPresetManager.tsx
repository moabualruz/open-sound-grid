/**
 * EqPresetManager — preset toolbar (dropdown, save, import/export, reset, gallery).
 * Receives all preset-related state and callbacks from useEqState.
 */
import { Show, For } from "solid-js";
import { Save } from "lucide-solid";
import type { EqState } from "./useEqState";
import { toSerializedBands, buildMacroBands } from "./useEqState";
import PresetGallery from "./PresetGallery";
import PresetSaveDialog from "./PresetSaveDialog";

interface EqPresetManagerProps {
  label: string;
  category: "app" | "mic" | "mix" | "cell";
  readonly?: boolean;
  state: EqState;
}

export default function EqPresetManager(props: EqPresetManagerProps) {
  const s = () => props.state;

  const handleExport = () => {
    const data = JSON.stringify(
      {
        bands: s().bands(),
        bass: s().bass(),
        voice: s().voice(),
        treble: s().treble(),
      },
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
  };

  const handleImport = () => {
    const input = document.createElement("input");
    input.type = "file";
    input.accept = ".json";
    input.onchange = () => {
      const file = input.files?.[0];
      if (!file) return;
      file.text().then((text) => {
        const data = JSON.parse(text);
        if (data.bands) s().setBands(() => data.bands);
        if (data.bass != null) s().setBass(data.bass);
        if (data.voice != null) s().setVoice(data.voice);
        if (data.treble != null) s().setTreble(data.treble);
      });
    };
    input.click();
  };

  return (
    <>
      {/* Preset dropdown */}
      <select
        class="rounded px-1.5 py-0.5 text-[10px]"
        style={{
          "background-color": "var(--color-bg-primary)",
          color: "var(--color-text-secondary)",
          border: "1px solid var(--color-border)",
        }}
        value={s().lastPresetId() ?? ""}
        onChange={(e) => s().handlePresetSelect(e)}
      >
        <option value="">Preset...</option>
        <For each={s().availablePresets()}>
          {(preset) => (
            <option value={preset.id} title={preset.description}>
              {preset.name}
            </option>
          )}
        </For>
        <option value="__more__">More...</option>
      </select>

      {/* Reset to last preset */}
      <Show when={s().lastPresetId()}>
        <button
          class="rounded px-1.5 py-0.5 text-[10px] transition-colors"
          style={{ color: "var(--color-text-muted)", background: "transparent" }}
          onClick={() => s().resetToLastPreset()}
          title="Reset to last applied preset"
        >
          Reset
        </button>
      </Show>

      {/* Save as custom preset */}
      <button
        class="rounded px-1 py-0.5 transition-colors"
        style={{ color: "var(--color-text-muted)", background: "transparent" }}
        onClick={() => s().setShowSaveDialog(true)}
        title="Save as custom preset"
      >
        <Save size={11} />
      </button>

      {/* Export */}
      <button
        class="rounded px-1.5 py-0.5 text-[10px] transition-colors"
        style={{ color: "var(--color-text-muted)", background: "transparent" }}
        onClick={handleExport}
        title="Export EQ preset"
      >
        ↓
      </button>

      {/* Import */}
      <button
        class="rounded px-1.5 py-0.5 text-[10px] transition-colors"
        style={{ color: "var(--color-text-muted)", background: "transparent" }}
        onClick={handleImport}
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

      {/* Preset gallery modal */}
      <Show when={s().showGallery()}>
        <PresetGallery
          category={props.category}
          onApply={(preset) => {
            s().applyPreset(preset);
            s().setShowGallery(false);
          }}
          onClose={() => s().setShowGallery(false)}
          favorites={s().favoritesSet()}
          onToggleFavorite={s().toggleFavorite}
        />
      </Show>

      {/* Save preset dialog */}
      <Show when={s().showSaveDialog()}>
        <PresetSaveDialog
          currentEq={{
            enabled: s().enabled(),
            bands: [
              ...toSerializedBands(s().bands()),
              ...buildMacroBands(s().bass(), s().voice(), s().treble()),
            ],
          }}
          defaultCategory={props.category}
          onClose={() => s().setShowSaveDialog(false)}
          onSaved={(preset) => {
            s().setLastPresetId(preset.id);
            s().setShowSaveDialog(false);
          }}
        />
      </Show>
    </>
  );
}
