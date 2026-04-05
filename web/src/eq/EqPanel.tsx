/**
 * EQ Panel — uniform parametric EQ for all node types.
 * Same 10-band EQ, macros, and presets everywhere.
 * No position-based feature branching.
 *
 * Layout composition only — state lives in useEqState, preset UI in EqPresetManager.
 */
import { Show, lazy } from "solid-js";
import type { EqConfig } from "../types/eq";
import { useEqState, MAX_BANDS } from "./useEqState";
import EqGraph from "./EqGraph";
import EqBandPopup from "./EqBandPopup";
import EqMacroSliders from "./EqMacroSliders";
import EqPresetManager from "./EqPresetManager";

const SpectrumAnalyzer = lazy(() => import("../spectrum/SpectrumAnalyzer"));

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
  /**
   * When provided, renders a SpectrumAnalyzer in overlay mode behind the EQ graph.
   * Must be a valid /ws/spectrum node key.
   */
  spectrumNodeKey?: string;
}

export default function EqPanel(props: EqPanelProps) {
  const s = useEqState({
    get initialEq() {
      return props.initialEq;
    },
    get category() {
      return props.category;
    },
    get onEqChange() {
      return props.onEqChange;
    },
  });

  const category = () => props.category ?? "app";

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
          {/* Bypass toggle */}
          <button
            class="w-8 h-4 rounded-full relative transition-colors"
            style={{
              "background-color": s.enabled()
                ? (props.color ?? "var(--color-accent)")
                : "var(--color-bg-hover)",
            }}
            onClick={() => s.setEnabled(!s.enabled())}
          >
            <div
              class="absolute top-0.5 w-3 h-3 rounded-full transition-all duration-150"
              style={{
                left: s.enabled() ? "17px" : "2px",
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
          <Show when={s.canAddBand() && !props.readonly}>
            <button
              class="rounded px-1.5 py-0.5 text-[10px] transition-colors"
              style={{ color: "var(--color-accent)", background: "transparent" }}
              onClick={s.addBand}
              title={`Add band (${s.bands().length}/${MAX_BANDS})`}
            >
              + Band
            </button>
          </Show>
          <span class="text-[10px] font-mono" style={{ color: "var(--color-text-muted)" }}>
            {s.bands().length}/{MAX_BANDS}
          </span>

          <EqPresetManager
            label={props.label}
            category={category()}
            readonly={props.readonly}
            state={s}
          />
        </div>
      </div>

      {/* EQ Graph — optional spectrum overlay sits behind the SVG */}
      <div
        style={{ opacity: s.enabled() ? 1 : 0.3, position: "relative" }}
        class="transition-opacity duration-200"
      >
        <Show when={props.spectrumNodeKey}>
          {(nodeKey) => (
            <div
              style={{
                position: "absolute",
                inset: "0",
                "pointer-events": "none",
                overflow: "hidden",
              }}
              aria-hidden="true"
            >
              <SpectrumAnalyzer
                nodeKey={nodeKey()}
                overlay={true}
                width={720}
                height={280}
              />
            </div>
          )}
        </Show>
        <EqGraph
          bands={s.bands()}
          selectedBandId={s.selectedBandId()}
          onBandMove={(id, freq, gain) => s.updateBand(id, { frequency: freq, gain })}
          onBandSelect={s.setSelectedBandId}
          onBandQChange={(id, q) => s.updateBand(id, { q })}
          readonly={props.readonly || !s.enabled()}
        />
      </div>

      {/* Band detail popup */}
      <Show when={s.enabled() ? s.selectedBand() : undefined}>
        {(band) => (
          <div class="px-2 py-1.5">
            <EqBandPopup
              band={band()}
              onToggleEnabled={(id) => s.updateBand(id, { enabled: !band().enabled })}
              onChangeType={(id, type) => s.updateBand(id, { type })}
              onChangeFreq={(id, freq) => s.updateBand(id, { frequency: freq })}
              onChangeGain={(id, gain) => s.updateBand(id, { gain })}
              onChangeQ={(id, q) => s.updateBand(id, { q })}
              onRemove={s.bands().length > 1 ? s.removeBand : undefined}
            />
          </div>
        )}
      </Show>

      {/* Macro sliders */}
      <Show when={s.enabled()}>
        <EqMacroSliders
          bass={s.bass()}
          voice={s.voice()}
          treble={s.treble()}
          onBassChange={s.setBass}
          onVoiceChange={s.setVoice}
          onTrebleChange={s.setTreble}
          readonly={props.readonly}
        />
      </Show>
    </div>
  );
}
