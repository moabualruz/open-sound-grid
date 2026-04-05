/**
 * EqThumbnail — mini canvas-based frequency response curve (~60×30px).
 * Renders composite EQ curve from EqConfig band data using biquad math.
 * Canvas-based for performance (many instances visible simultaneously).
 * Shows a flat line when EQ is disabled/bypassed or has no bands.
 */
import { onMount, onCleanup, createEffect } from "solid-js";
import type { JSX } from "solid-js";
import type { EqConfig } from "../types/eq";
import { computeCoefficients, magnitudeAt, xToFreq } from "../eq/math";

interface EqThumbnailProps {
  eq: EqConfig | undefined;
  /** Canvas width in px. Default: 60. */
  width?: number;
  /** Canvas height in px. Default: 30. */
  height?: number;
  /** Accent color for the curve stroke. Default: var(--color-accent). */
  color?: string;
  /** Click handler — e.g. navigate to full EQ page. */
  onClick?: () => void;
  "aria-label"?: string;
}

const SAMPLE_RATE = 48000;
const DB_RANGE = 12;
const CURVE_POINTS = 64; // reduced vs full EqGraph — enough for a thumbnail

function drawThumbnail(
  canvas: HTMLCanvasElement,
  eq: EqConfig | undefined,
  color: string,
  w: number,
  h: number,
): void {
  const ctx = canvas.getContext("2d");
  if (!ctx) return;

  const dpr = window.devicePixelRatio || 1;
  if (canvas.width !== w * dpr || canvas.height !== h * dpr) {
    canvas.width = w * dpr;
    canvas.height = h * dpr;
    ctx.scale(dpr, dpr);
  }

  ctx.clearRect(0, 0, w, h);

  // Background
  ctx.fillStyle = "rgba(0,0,0,0.0)";
  ctx.fillRect(0, 0, w, h);

  // Zero-line (subtle)
  const zeroY = h / 2;
  ctx.beginPath();
  ctx.strokeStyle = "rgba(128,128,128,0.2)";
  ctx.lineWidth = 0.5;
  ctx.moveTo(0, zeroY);
  ctx.lineTo(w, zeroY);
  ctx.stroke();

  const active = eq?.enabled && eq.bands.length > 0;
  if (!active) {
    // Flat line
    ctx.beginPath();
    ctx.strokeStyle = color;
    ctx.lineWidth = 1;
    ctx.globalAlpha = 0.35;
    ctx.moveTo(0, zeroY);
    ctx.lineTo(w, zeroY);
    ctx.stroke();
    ctx.globalAlpha = 1;
    return;
  }

  const enabledBands = eq!.bands.filter((b) => b.enabled);
  if (enabledBands.length === 0) {
    ctx.beginPath();
    ctx.strokeStyle = color;
    ctx.lineWidth = 1;
    ctx.globalAlpha = 0.35;
    ctx.moveTo(0, zeroY);
    ctx.lineTo(w, zeroY);
    ctx.stroke();
    ctx.globalAlpha = 1;
    return;
  }

  const allCoeffs = enabledBands.map((b) =>
    computeCoefficients(b.filterType, b.frequency, b.gain, b.q, SAMPLE_RATE),
  );

  ctx.beginPath();
  ctx.strokeStyle = color;
  ctx.lineWidth = 1.5;
  ctx.globalAlpha = 0.9;

  for (let i = 0; i <= CURVE_POINTS; i++) {
    const x = (i / CURVE_POINTS) * w;
    const freq = xToFreq(x, w);
    let totalDb = 0;
    for (const c of allCoeffs) {
      totalDb += magnitudeAt(c, freq, SAMPLE_RATE);
    }
    const clampedDb = Math.max(-DB_RANGE, Math.min(DB_RANGE, totalDb));
    const y = zeroY - (clampedDb / DB_RANGE) * (h / 2 - 1);

    if (i === 0) ctx.moveTo(x, y);
    else ctx.lineTo(x, y);
  }

  ctx.stroke();
  ctx.globalAlpha = 1;

  // Subtle fill under/over curve
  ctx.globalAlpha = 0.12;
  ctx.fillStyle = color;
  ctx.lineTo(w, zeroY);
  ctx.lineTo(0, zeroY);
  ctx.closePath();
  ctx.fill();
  ctx.globalAlpha = 1;
}

export default function EqThumbnail(props: EqThumbnailProps): JSX.Element {
  const w = () => props.width ?? 60;
  const h = () => props.height ?? 30;

  let canvasRef: HTMLCanvasElement | undefined;

  const resolveColor = (): string => {
    if (props.color) return props.color;
    // Resolve CSS variable at runtime for canvas (canvas can't use CSS vars directly)
    if (typeof window !== "undefined") {
      const val = getComputedStyle(document.documentElement)
        .getPropertyValue("--color-accent")
        .trim();
      return val || "#da7756";
    }
    return "#da7756";
  };

  function redraw() {
    if (!canvasRef) return;
    drawThumbnail(canvasRef, props.eq, resolveColor(), w(), h());
  }

  onMount(() => {
    redraw();

    // Re-draw when color scheme changes (dark/light toggle)
    const observer = new MutationObserver(redraw);
    observer.observe(document.documentElement, { attributes: true, attributeFilter: ["class"] });
    onCleanup(() => observer.disconnect());
  });

  createEffect(() => {
    // Re-draw whenever eq prop changes
    // eslint-disable-next-line no-unused-expressions
    props.eq;
    redraw();
  });

  const label = () => props["aria-label"] ?? "EQ frequency response thumbnail";

  return (
    <canvas
      ref={canvasRef}
      width={w()}
      height={h()}
      style={{
        width: `${w()}px`,
        height: `${h()}px`,
        cursor: props.onClick ? "pointer" : "default",
        display: "block",
        "border-radius": "3px",
      }}
      aria-label={label()}
      role={props.onClick ? "button" : "img"}
      tabIndex={props.onClick ? 0 : undefined}
      onClick={props.onClick}
      onKeyDown={(e) => {
        if (props.onClick && (e.key === "Enter" || e.key === " ")) {
          e.preventDefault();
          props.onClick();
        }
      }}
    />
  );
}
