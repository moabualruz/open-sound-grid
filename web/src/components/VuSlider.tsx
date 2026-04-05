import { createSignal, onCleanup, createEffect } from "solid-js";
import type { JSX } from "solid-js";

/**
 * VuSlider — a range input overlaid on a real-time VU level fill.
 *
 * The VU background is purely decorative (aria-hidden). The <input type="range">
 * remains the sole interactive ARIA element. Fill uses the channel accent color
 * at ~30% opacity so it sits behind the slider thumb track.
 *
 * Smoothing: 50 ms attack (fast rise), 300 ms release (slow fall) via rAF.
 * Peak hold: optional indicator line that decays after 1.5 s.
 */

export interface VuSliderProps {
  /** Slider position, 0–1. */
  value: number;
  /** Raw peak level, 0–1, updated at ~30 fps from the backend. */
  peak: number;
  /** Accent color used for the VU fill (hex / rgb / CSS variable). */
  color: string;
  /** Called when the user moves the slider. */
  onInput: (v: number) => void;
  disabled?: boolean;
  orientation?: "vertical" | "horizontal";
  /** aria-label forwarded to the <input>. */
  ariaLabel?: string;
  /** Optional aria-valuetext forwarded to the <input>. */
  ariaValueText?: string;
}

const ATTACK_MS = 50;
const RELEASE_MS = 300;
const PEAK_HOLD_MS = 1500;
const PEAK_DECAY_MS = 400;

const prefersReducedMotion = (): boolean => {
  try {
    return (
      typeof window !== "undefined" && window.matchMedia("(prefers-reduced-motion: reduce)").matches
    );
  } catch {
    return false;
  }
};

export default function VuSlider(props: VuSliderProps): JSX.Element {
  const [smoothedPeak, setSmoothedPeak] = createSignal(0);
  const [peakHold, setPeakHold] = createSignal(0);

  let rafId = 0;
  let lastTimestamp = 0;
  let peakHoldTime = 0; // ms since peakHold was last raised
  let peakDecayStart = 0; // ms since decay began (0 = not decaying)

  function tick(timestamp: number) {
    const dt = lastTimestamp === 0 ? 16 : timestamp - lastTimestamp;
    lastTimestamp = timestamp;

    const target = props.peak;

    if (prefersReducedMotion()) {
      // Skip smoothing — snap directly to current peak, no animation
      setSmoothedPeak(target);
      setPeakHold(target);
      peakHoldTime = timestamp;
      peakDecayStart = 0;
      rafId = requestAnimationFrame(tick);
      return;
    }

    const current = smoothedPeak();

    // Exponential smoothing: different coefficients for attack vs release
    const tau = target > current ? ATTACK_MS : RELEASE_MS;
    const alpha = 1 - Math.exp(-dt / tau);
    const next = current + (target - current) * alpha;
    setSmoothedPeak(next);

    // Peak hold logic
    const hold = peakHold();
    if (next >= hold) {
      setPeakHold(next);
      peakHoldTime = timestamp;
      peakDecayStart = 0;
    } else if (peakDecayStart === 0 && timestamp - peakHoldTime > PEAK_HOLD_MS) {
      peakDecayStart = timestamp;
    } else if (peakDecayStart > 0) {
      const decayAlpha = (timestamp - peakDecayStart) / PEAK_DECAY_MS;
      if (decayAlpha >= 1) {
        setPeakHold(0);
        peakDecayStart = 0;
      } else {
        setPeakHold(hold * (1 - decayAlpha));
      }
    }

    rafId = requestAnimationFrame(tick);
  }

  rafId = requestAnimationFrame(tick);
  onCleanup(() => cancelAnimationFrame(rafId));

  // Reset smoothing when peak jumps to 0 (e.g. muted / stream stopped)
  createEffect(() => {
    if (props.peak === 0) {
      setSmoothedPeak(0);
      setPeakHold(0);
      peakHoldTime = 0;
      peakDecayStart = 0;
      lastTimestamp = 0;
    }
  });

  const fillPct = () => Math.round(smoothedPeak() * 100);
  const holdPct = () => Math.round(peakHold() * 100);

  return (
    <div class="relative flex w-full items-center" data-testid="vu-slider">
      {/* VU fill — decorative, aria-hidden */}
      <div
        aria-hidden="true"
        class="pointer-events-none absolute top-1/2 left-0 h-2.5 -translate-y-1/2 rounded-full"
        style={{
          width: `${fillPct()}%`,
          background: `linear-gradient(to right, ${props.color}4d 0%, ${props.color}80 70%, ${props.color}cc 90%, #f4433680 100%)`,
          opacity: props.disabled ? 0.08 : 0.45,
          transition: "opacity 0.15s",
        }}
        data-testid="vu-fill"
      />

      {/* Peak hold indicator — decorative, aria-hidden */}
      <div
        aria-hidden="true"
        class="pointer-events-none absolute top-1/2 -translate-y-1/2"
        style={{
          left: `${holdPct()}%`,
          width: "2px",
          height: "10px",
          background: holdPct() > 0 ? props.color : "transparent",
          opacity: props.disabled ? 0 : 0.7,
          "border-radius": "1px",
          transition: "opacity 0.15s",
        }}
        data-testid="vu-peak-hold"
      />

      {/* The interactive slider — sole ARIA element */}
      <input
        type="range"
        min="0"
        max="1"
        step="0.01"
        value={props.value}
        disabled={props.disabled}
        onInput={(e) => props.onInput(parseFloat(e.currentTarget.value))}
        aria-label={props.ariaLabel ?? "Volume"}
        aria-valuetext={props.ariaValueText}
        class="relative z-10 w-full"
        style={{ "--value-pct": `${Math.round(props.value * 100)}%` }}
        data-testid="vu-input"
      />
    </div>
  );
}
