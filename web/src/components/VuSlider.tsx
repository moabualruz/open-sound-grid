import { Show, createSignal } from "solid-js";
import type { JSX } from "solid-js";

export interface VuSliderProps {
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
  /** Per-surface accent color used for the slider track/thumb. */
  accentColor?: string;
}

function vuFillStyle(level: number, muted: boolean): JSX.CSSProperties {
  const width = `${Math.round(Math.max(0, Math.min(1, level)) * 100)}%`;
  return {
    width,
    opacity: muted ? "0.12" : "0.7",
    background:
      "linear-gradient(to right, var(--color-vu-safe) 0 70%, var(--color-vu-warm) 70% 90%, var(--color-vu-hot) 90% 100%)",
    transition:
      "width 90ms linear, opacity 150ms var(--ease-out-quart), filter 150ms var(--ease-out-quart)",
  };
}

/**
 * Combined volume slider + VU background track.
 * Pure display component — all smoothing is done upstream via useSmoothedPeak.
 */
export default function VuSlider(props: VuSliderProps): JSX.Element {
  const monoPeak = () => Math.max(props.peakLeft, props.peakRight);
  const valuePct = () => `${Math.round(props.value * 100)}%`;
  const accentColor = () => props.accentColor ?? "var(--color-accent)";
  const [editing, setEditing] = createSignal(false);
  const [editValue, setEditValue] = createSignal("");

  function startExactEdit() {
    if (props.disabled) return;
    setEditValue(String(Math.round(props.value * 100)));
    setEditing(true);
  }

  const editPlaceholder = () => `${Math.round(props.value * 100)}%`;

  function cancelExactEdit() {
    setEditing(false);
    setEditValue("");
  }

  function commitExactEdit() {
    const parsed = Number.parseFloat(editValue().trim());
    if (Number.isFinite(parsed)) {
      const clamped = Math.max(0, Math.min(100, parsed));
      props.onInput(clamped / 100);
    }
    cancelExactEdit();
  }

  return (
    <div
      class="relative flex w-full items-center"
      data-testid="vu-slider"
      onDblClick={() => startExactEdit()}
    >
      <Show
        when={editing()}
        fallback={
          <>
            <div
              aria-hidden="true"
              class="pointer-events-none absolute inset-0 flex flex-col justify-center gap-px"
            >
              {props.stereo ? (
                <>
                  <div class="h-[5px] w-full overflow-hidden rounded-full bg-transparent">
                    <div
                      class="h-full rounded-full"
                      style={vuFillStyle(props.peakLeft, !!props.muted)}
                      data-testid="vu-fill-left"
                    />
                  </div>
                  <div class="h-[5px] w-full overflow-hidden rounded-full bg-transparent">
                    <div
                      class="h-full rounded-full"
                      style={vuFillStyle(props.peakRight, !!props.muted)}
                      data-testid="vu-fill-right"
                    />
                  </div>
                </>
              ) : (
                <div class="h-[6px] w-full overflow-hidden rounded-full bg-transparent">
                  <div
                    class="h-full rounded-full"
                    style={vuFillStyle(monoPeak(), !!props.muted)}
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
              style={{
                "--value-pct": valuePct(),
                "--slider-accent": accentColor(),
              }}
              data-testid="vu-input"
            />
          </>
        }
      >
        <input
          type="text"
          value={editValue()}
          placeholder={editPlaceholder()}
          inputmode="decimal"
          autofocus
          onInput={(event) => setEditValue(event.currentTarget.value)}
          onBlur={commitExactEdit}
          onKeyDown={(event) => {
            if (event.key === "Enter") commitExactEdit();
            if (event.key === "Escape") {
              event.preventDefault();
              cancelExactEdit();
            }
          }}
          aria-label={`${props.label} exact value (%)`}
          class="w-full rounded border border-border-active bg-bg-primary px-2 py-1 text-center font-mono text-[11px] text-text-primary focus:outline-none"
          data-testid="vu-exact-input"
        />
      </Show>
    </div>
  );
}
