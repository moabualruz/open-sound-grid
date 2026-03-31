/**
 * Bass / Voice / Treble macro sliders — Sonar-style simple controls
 * that adjust predefined frequency ranges across multiple bands.
 */

interface MacroSliderProps {
  label: string;
  value: number; // dB
  color: string;
  onChange: (value: number) => void;
  readonly?: boolean;
}

function MacroSlider(props: MacroSliderProps) {
  return (
    <div class="flex items-center gap-2 flex-1 min-w-0">
      <span
        class="text-[10px] font-medium w-10 text-right shrink-0"
        style={{ color: "var(--color-text-secondary)" }}
      >
        {props.label}
      </span>
      <input
        type="range"
        min={-12}
        max={12}
        step={0.1}
        value={props.value}
        disabled={props.readonly}
        class="flex-1 min-w-0"
        style={{ "accent-color": props.color }}
        onInput={(e) => props.onChange(parseFloat(e.currentTarget.value))}
      />
      <span
        class="text-[10px] font-mono tabular-nums w-12 text-right shrink-0"
        style={{ color: "var(--color-text-muted)" }}
      >
        {props.value > 0 ? "+" : ""}
        {props.value.toFixed(1)} dB
      </span>
    </div>
  );
}

interface EqMacroSlidersProps {
  bass: number;
  voice: number;
  treble: number;
  onBassChange: (v: number) => void;
  onVoiceChange: (v: number) => void;
  onTrebleChange: (v: number) => void;
  readonly?: boolean;
}

export default function EqMacroSliders(props: EqMacroSlidersProps) {
  return (
    <div
      class="flex gap-3 px-2 py-1.5 rounded-b-lg"
      style={{ "background-color": "var(--color-bg-secondary)" }}
    >
      <MacroSlider
        label="Bass"
        value={props.bass}
        color="var(--color-accent-secondary)"
        onChange={props.onBassChange}
        readonly={props.readonly}
      />
      <MacroSlider
        label="Voice"
        value={props.voice}
        color="var(--color-vu-safe)"
        onChange={props.onVoiceChange}
        readonly={props.readonly}
      />
      <MacroSlider
        label="Treble"
        value={props.treble}
        color="var(--color-mix-aux)"
        onChange={props.onTrebleChange}
        readonly={props.readonly}
      />
    </div>
  );
}
