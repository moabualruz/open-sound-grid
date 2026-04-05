import { createSignal, createEffect, onCleanup, batch } from "solid-js";
import type { JSX } from "solid-js";

export interface MeterSliderProps {
  /** Slider position, 0-1. */
  value: number;
  /** Accessor function returning live peak levels (0-1). */
  peak: () => { left: number; right: number };
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

const ATTACK_COEFF = 0.4; // Fast rise (~30ms equivalent at 30fps)
const RELEASE_COEFF = 0.08; // Slow decay (~200ms equivalent at 30fps)

/** VU color based on peak level: green <70%, amber 70-90%, red >90%. */
function vuColor(level: number): string {
  if (level > 0.9) return "var(--color-vu-hot)";
  if (level > 0.7) return "var(--color-vu-warm)";
  return "var(--color-vu-safe)";
}

export default function MeterSlider(props: MeterSliderProps): JSX.Element {
  const [smoothL, setSmoothedL] = createSignal(0);
  const [smoothR, setSmoothedR] = createSignal(0);

  // Drive VU smoothing from a reactive effect that tracks props.peak().
  // SolidJS tracks the peak accessor read, so this re-runs every time
  // the levels store pushes new data (~30fps from /ws/levels).
  createEffect(() => {
    const raw = props.peak();
    const targetL = raw.left;
    const targetR = raw.right;

    batch(() => {
      // Exponential smoothing: fast attack, slow release
      const curL = smoothL();
      const alphaL = targetL > curL ? ATTACK_COEFF : RELEASE_COEFF;
      setSmoothedL(curL + (targetL - curL) * alphaL);

      const curR = smoothR();
      const alphaR = targetR > curR ? ATTACK_COEFF : RELEASE_COEFF;
      setSmoothedR(curR + (targetR - curR) * alphaR);
    });
  });

  // VU fill capped at slider value
  const cappedL = () => Math.min(smoothL(), props.value);
  const cappedR = () => Math.min(smoothR(), props.value);
  const cappedMono = () => Math.min(Math.max(smoothL(), smoothR()), props.value);

  const vuOpacity = () => (props.muted ? 0.1 : 0.5);
  const valuePct = () => `${Math.round(props.value * 100)}%`;

  return (
    <div class="relative flex w-full items-center" data-testid="meter-slider">
      {/* VU fill layer(s) — decorative, behind the slider */}
      <div
        aria-hidden="true"
        class="pointer-events-none absolute inset-0 flex flex-col justify-center gap-px"
      >
        {props.stereo ? (
          <>
            {/* Left channel VU bar */}
            <div class="h-[5px] w-full overflow-hidden rounded-full bg-transparent">
              <div
                class="h-full rounded-full transition-colors duration-100"
                style={{
                  width: `${Math.round(cappedL() * 100)}%`,
                  "background-color": vuColor(smoothL()),
                  opacity: vuOpacity(),
                }}
                data-testid="vu-fill-left"
              />
            </div>
            {/* Right channel VU bar */}
            <div class="h-[5px] w-full overflow-hidden rounded-full bg-transparent">
              <div
                class="h-full rounded-full transition-colors duration-100"
                style={{
                  width: `${Math.round(cappedR() * 100)}%`,
                  "background-color": vuColor(smoothR()),
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

      {/* The interactive slider — sole ARIA element */}
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
