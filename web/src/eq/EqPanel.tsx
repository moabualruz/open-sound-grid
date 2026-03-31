/**
 * EQ Panel — uniform parametric EQ for all node types.
 * Same 10-band EQ, macros, and presets everywhere.
 * No position-based feature branching.
 */
import { createSignal, Show } from "solid-js";
import type { EqBand } from "./math";
import { createDefaultBands, createDefaultBand } from "./math";
import EqGraph from "./EqGraph";
import EqBandPopup from "./EqBandPopup";
import EqMacroSliders from "./EqMacroSliders";

const MAX_BANDS = 10;

interface EqPanelProps {
  label: string;
  color?: string;
  readonly?: boolean;
}

export default function EqPanel(props: EqPanelProps) {
  const [enabled, setEnabled] = createSignal(true);
  const [bands, setBands] = createSignal<EqBand[]>(createDefaultBands());
  const [selectedBandId, setSelectedBandId] = createSignal<number | null>(null);
  const [bass, setBass] = createSignal(0);
  const [voice, setVoice] = createSignal(0);
  const [treble, setTreble] = createSignal(0);

  const selectedBand = () => bands().find((b) => b.id === selectedBandId());
  const canAddBand = () => bands().length < MAX_BANDS;

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
          <select
            class="rounded px-1.5 py-0.5 text-[10px]"
            style={{
              "background-color": "var(--color-bg-primary)",
              color: "var(--color-text-secondary)",
              border: "1px solid var(--color-border)",
            }}
          >
            <option>Default</option>
            <option>Flat</option>
            <option>Voice Boost</option>
            <option>Bass Heavy</option>
            <option>Treble Boost</option>
          </select>
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
