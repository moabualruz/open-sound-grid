/**
 * Band detail popup — appears when a band dot is selected.
 * Shows: enable toggle, filter type selector, frequency/gain/Q fields.
 * Modeled after Sonar's per-band popup.
 */
import { Show } from "solid-js";
import type { EqBand, FilterType } from "./math";
import { formatFreq } from "./math";

const FILTER_TYPES: { type: FilterType; label: string; icon: string }[] = [
  { type: "peaking", label: "Peaking EQ", icon: "∿" },
  { type: "lowShelf", label: "Low Shelf", icon: "⌊" },
  { type: "highShelf", label: "High Shelf", icon: "⌈" },
  { type: "lowPass", label: "Low Pass", icon: "╲" },
  { type: "highPass", label: "High Pass", icon: "╱" },
  { type: "notch", label: "Notch", icon: "⋁" },
];

interface EqBandPopupProps {
  band: EqBand;
  onToggleEnabled: (id: number) => void;
  onChangeType: (id: number, type: FilterType) => void;
  onChangeFreq: (id: number, freq: number) => void;
  onChangeGain: (id: number, gain: number) => void;
  onChangeQ: (id: number, q: number) => void;
  onRemove?: (id: number) => void;
}

export default function EqBandPopup(props: EqBandPopupProps) {
  const currentType = () => FILTER_TYPES.find((f) => f.type === props.band.type) ?? FILTER_TYPES[0];

  return (
    <div
      class="rounded-lg border px-3 py-2 text-xs"
      style={{
        "background-color": "var(--color-bg-elevated)",
        "border-color": props.band.color,
        "border-width": "1.5px",
        "min-width": "240px",
      }}
    >
      {/* Header: enable toggle + filter type */}
      <div class="flex items-center justify-between gap-2 mb-2">
        <button
          class="flex items-center gap-1.5 rounded px-1.5 py-0.5 transition-colors"
          style={{
            background: props.band.enabled ? props.band.color : "var(--color-bg-hover)",
            color: props.band.enabled ? "var(--color-text-primary)" : "var(--color-text-muted)",
          }}
          onClick={() => props.onToggleEnabled(props.band.id)}
        >
          <span class="text-[10px] font-medium">{props.band.enabled ? "Enabled" : "Disabled"}</span>
        </button>

        <div class="flex items-center gap-1">
          <span class="text-base">{currentType().icon}</span>
          <select
            class="rounded px-1 py-0.5 text-xs"
            style={{
              "background-color": "var(--color-bg-primary)",
              color: "var(--color-text-secondary)",
              border: "1px solid var(--color-border)",
            }}
            value={props.band.type}
            onChange={(e) => props.onChangeType(props.band.id, e.currentTarget.value as FilterType)}
          >
            {FILTER_TYPES.map((ft) => (
              <option value={ft.type}>{ft.label}</option>
            ))}
          </select>
        </div>

        <Show when={props.onRemove}>
          <button
            class="rounded px-1 py-0.5 text-[10px] transition-colors"
            style={{ color: "var(--color-vu-hot)" }}
            onClick={() => props.onRemove?.(props.band.id)}
            title="Remove band"
          >
            ✕
          </button>
        </Show>
      </div>

      {/* Parameter fields */}
      <div class="grid grid-cols-3 gap-2">
        <NumericField
          label="Gain"
          value={props.band.gain}
          unit="dB"
          min={-12}
          max={12}
          step={0.1}
          color={props.band.color}
          onChange={(v) => props.onChangeGain(props.band.id, v)}
        />
        <NumericField
          label="Freq"
          value={props.band.frequency}
          displayValue={formatFreq(props.band.frequency)}
          min={20}
          max={20000}
          step={1}
          color={props.band.color}
          onChange={(v) => props.onChangeFreq(props.band.id, v)}
        />
        <NumericField
          label="Q"
          value={props.band.q}
          min={0.1}
          max={10}
          step={0.01}
          color={props.band.color}
          onChange={(v) => props.onChangeQ(props.band.id, v)}
        />
      </div>
    </div>
  );
}

// ---------------------------------------------------------------------------
// Numeric field with inline editing
// ---------------------------------------------------------------------------

interface NumericFieldProps {
  label: string;
  value: number;
  displayValue?: string;
  unit?: string;
  min: number;
  max: number;
  step: number;
  color: string;
  onChange: (value: number) => void;
}

function NumericField(props: NumericFieldProps) {
  return (
    <div class="flex flex-col items-center gap-0.5">
      <input
        type="number"
        value={props.value}
        min={props.min}
        max={props.max}
        step={props.step}
        class="w-full rounded px-1.5 py-1 text-center text-xs font-mono tabular-nums"
        style={{
          "background-color": "var(--color-bg-primary)",
          color: "var(--color-text-primary)",
          border: `1px solid ${props.color}`,
        }}
        onInput={(e) => {
          const v = parseFloat(e.currentTarget.value);
          if (!isNaN(v)) props.onChange(Math.max(props.min, Math.min(props.max, v)));
        }}
      />
      <span
        class="text-[9px] uppercase tracking-wider"
        style={{ color: "var(--color-text-muted)" }}
      >
        {props.label}
      </span>
    </div>
  );
}
