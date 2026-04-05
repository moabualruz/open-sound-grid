import type { JSX } from "solid-js";

export interface MeterSliderProps {
  /** Slider position, 0-1. */
  value: number;
  /** Smoothed peak level for left channel (0-1). Already smoothed by the caller. */
  peakLeft: number;
  /** Smoothed peak level for right channel (0-1). Already smoothed by the caller. */
  peakRight: number;
  /** Called when the user moves the slider. */
  onInput: (value: number) => void;
  disabled?: boolean;
  muted?: boolean;
  /** Show L/R split VU bars stacked vertically. */
  stereo?: boolean;
  /** aria-label forwarded to the <input>. */
  label: string;
  /** Optional aria-valuetext forwarded to the <input>. */
  valueText?: string;
}

/** VU color based on peak level: green <70%, amber 70-90%, red >90%. */
function vuColor(level: number): string {
  if (level > 0.9) return "var(--color-vu-hot)";
  if (level > 0.7) return "var(--color-vu-warm)";
  return "var(--color-vu-safe)";
}

/**
 * Combined volume slider + VU meter.
 * Pure display component — all smoothing is done upstream via useSmoothedPeak.
 */
export default function MeterSlider(props: MeterSliderProps): JSX.Element {
  const cappedL = () => Math.min(props.peakLeft, props.value);
  const cappedR = () => Math.min(props.peakRight, props.value);
  const cappedMono = () => Math.min(Math.max(props.peakLeft, props.peakRight), props.value);

  const vuOpacity = () => (props.muted ? 0.1 : 0.5);
  const valuePct = () => `${Math.round(props.value * 100)}%`;

  return (
    <div class="relative flex w-full items-center" data-testid="meter-slider">
      <div
        aria-hidden="true"
        class="pointer-events-none absolute inset-0 flex flex-col justify-center gap-px"
      >
        {props.stereo ? (
          <>
            <div class="h-[5px] w-full overflow-hidden rounded-full bg-transparent">
              <div
                class="h-full rounded-full transition-colors duration-100"
                style={{
                  width: `${Math.round(cappedL() * 100)}%`,
                  "background-color": vuColor(props.peakLeft),
                  opacity: vuOpacity(),
                }}
                data-testid="vu-fill-left"
              />
            </div>
            <div class="h-[5px] w-full overflow-hidden rounded-full bg-transparent">
              <div
                class="h-full rounded-full transition-colors duration-100"
                style={{
                  width: `${Math.round(cappedR() * 100)}%`,
                  "background-color": vuColor(props.peakRight),
                  opacity: vuOpacity(),
                }}
                data-testid="vu-fill-right"
              />
            </div>
          </>
        ) : (
          <div class="h-2.5 w-full overflow-hidden rounded-full bg-transparent">
            <div
              class="h-full rounded-full transition-colors duration-100"
              style={{
                width: `${Math.round(cappedMono() * 100)}%`,
                "background-color": vuColor(cappedMono()),
                opacity: vuOpacity(),
              }}
              data-testid="vu-fill"
            />
          </div>
        )}
      </div>

      <input
        type="range"
        min="0"
        max="1"
        step="0.01"
        value={props.value}
        disabled={props.disabled}
        onInput={(e) => props.onInput(parseFloat(e.currentTarget.value))}
        aria-label={props.label}
        aria-valuetext={props.valueText}
        class="relative z-10 w-full"
        style={{ "--value-pct": valuePct() }}
        data-testid="meter-input"
      />
    </div>
  );
}
