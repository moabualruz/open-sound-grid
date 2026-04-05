/**
 * Canvas-based spectrum analyzer component.
 *
 * X axis: log scale, 20Hz–20kHz
 * Y axis: dB scale, -60dB to 0dB
 * Rendering: gradient fill (green → yellow → red)
 * Peak hold: white lines, decay after 1.5s
 *
 * overlay=true: semi-transparent fill, no axes, no background.
 * Designed to sit behind the EQ curve.
 */
import { onMount, onCleanup, createEffect } from "solid-js";
import { spectrumStore, SPECTRUM_BINS } from "./spectrumStore";

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

const MIN_FREQ = 20;
const MAX_FREQ = 20_000;
const MIN_DB = -60;
const MAX_DB = 0;
const DB_RANGE = MAX_DB - MIN_DB; // 60

const PEAK_HOLD_MS = 1500;
const PEAK_DECAY_RATE = 0.015; // dB per frame at 60fps ≈ 0.9 dB/s

const FREQ_LABELS = [50, 100, 200, 500, 1_000, 2_000, 5_000, 10_000, 20_000];
const DB_TICKS = [-60, -50, -40, -30, -20, -10, 0];

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/** Map linear bin index (0–255) to log-frequency x position [0, width]. */
function binToX(bin: number, width: number): number {
  // Bins are already on a log scale covering 20Hz–20kHz
  return (bin / (SPECTRUM_BINS - 1)) * width;
}

/** Map frequency to x position [0, width]. */
function freqToX(freq: number, width: number): number {
  const logMin = Math.log10(MIN_FREQ);
  const logMax = Math.log10(MAX_FREQ);
  return ((Math.log10(freq) - logMin) / (logMax - logMin)) * width;
}

/** Map dB value to y position [0, height] (0dB at top, -60dB at bottom). */
function dbToY(db: number, height: number): number {
  return ((MAX_DB - db) / DB_RANGE) * height;
}

/** Convert linear magnitude [0, 1] to dB. */
function magToDb(mag: number): number {
  if (mag <= 0) return MIN_DB;
  return Math.max(MIN_DB, 20 * Math.log10(mag));
}

function formatFreq(freq: number): string {
  return freq >= 1000 ? `${freq / 1000}k` : `${freq}`;
}

// ---------------------------------------------------------------------------
// Component
// ---------------------------------------------------------------------------

export interface SpectrumAnalyzerProps {
  nodeKey: string;
  width?: number;
  height?: number;
  showLabels?: boolean;
  /** Semi-transparent fill, no axes, no background. For EQ overlay. */
  overlay?: boolean;
}

export default function SpectrumAnalyzer(props: SpectrumAnalyzerProps) {
  let canvasRef: HTMLCanvasElement | undefined;
  let rafId: number | null = null;

  // Peak hold state per channel (L/R), indexed by bin
  const peakLeft = new Float32Array(SPECTRUM_BINS).fill(MIN_DB);
  const peakRight = new Float32Array(SPECTRUM_BINS).fill(MIN_DB);
  const peakLeftTime = new Float32Array(SPECTRUM_BINS).fill(0);
  const peakRightTime = new Float32Array(SPECTRUM_BINS).fill(0);

  const width = () => props.width ?? 600;
  const height = () => props.height ?? 200;
  const overlay = () => props.overlay ?? false;
  const showLabels = () => props.showLabels ?? !overlay();

  // Subscribe to spectrum data on mount
  onMount(() => {
    spectrumStore.subscribe(props.nodeKey);
    startRender();
  });

  onCleanup(() => {
    spectrumStore.unsubscribe(props.nodeKey);
    if (rafId !== null) cancelAnimationFrame(rafId);
  });

  // Re-subscribe if nodeKey changes
  createEffect(
    (prevKey: string | undefined) => {
      const key = props.nodeKey;
      if (prevKey !== undefined && prevKey !== key) {
        spectrumStore.unsubscribe(prevKey);
        spectrumStore.subscribe(key);
      }
      return key;
    },
    undefined as string | undefined,
  );

  // ---------------------------------------------------------------------------
  // Rendering
  // ---------------------------------------------------------------------------

  function startRender(): void {
    function frame(): void {
      rafId = requestAnimationFrame(frame);
      drawFrame();
    }
    rafId = requestAnimationFrame(frame);
  }

  function drawFrame(): void {
    if (!canvasRef) return;

    // Skip painting when there is no spectrum data for this node
    const bins = spectrumStore.state.bins[props.nodeKey];
    if (!bins) return;

    const ctx = canvasRef.getContext("2d");
    if (!ctx) return;

    const dpr = window.devicePixelRatio || 1;
    const w = width();
    const h = height();

    // Pad for labels
    const padLeft = showLabels() ? 36 : 0;
    const padBottom = showLabels() ? 18 : 0;
    const plotW = w - padLeft;
    const plotH = h - padBottom;

    // Canvas physical size tracks logical size
    const physW = w * dpr;
    const physH = h * dpr;
    if (canvasRef.width !== physW || canvasRef.height !== physH) {
      canvasRef.width = physW;
      canvasRef.height = physH;
    }

    ctx.save();
    ctx.scale(dpr, dpr);

    // Background
    if (!overlay()) {
      ctx.fillStyle =
        getComputedStyle(canvasRef).getPropertyValue("--color-bg-secondary").trim() || "#1e1e1e";
      ctx.fillRect(0, 0, w, h);
    } else {
      ctx.clearRect(0, 0, w, h);
    }

    // Draw plot area starting at (padLeft, 0)
    ctx.save();
    ctx.translate(padLeft, 0);

    drawGrid(ctx, plotW, plotH);
    drawBins(ctx, plotW, plotH);

    ctx.restore();

    if (showLabels()) {
      drawLabels(ctx, padLeft, plotW, plotH, w, h);
    }

    ctx.restore();
  }

  function drawGrid(ctx: CanvasRenderingContext2D, plotW: number, plotH: number): void {
    if (overlay()) return;

    ctx.save();
    ctx.strokeStyle = "rgba(255,255,255,0.06)";
    ctx.lineWidth = 0.5;

    // Horizontal dB lines
    for (const db of DB_TICKS) {
      const y = dbToY(db, plotH);
      ctx.beginPath();
      ctx.moveTo(0, y);
      ctx.lineTo(plotW, y);
      ctx.stroke();
    }

    // Vertical frequency lines
    for (const freq of FREQ_LABELS) {
      const x = freqToX(freq, plotW);
      ctx.beginPath();
      ctx.moveTo(x, 0);
      ctx.lineTo(x, plotH);
      ctx.stroke();
    }

    ctx.restore();
  }

  function drawBins(ctx: CanvasRenderingContext2D, plotW: number, plotH: number): void {
    const bins = spectrumStore.state.bins[props.nodeKey];

    // Build gradient
    const grad = ctx.createLinearGradient(0, 0, 0, plotH);
    if (overlay()) {
      grad.addColorStop(0, "rgba(255, 80, 80, 0.30)");
      grad.addColorStop(0.5, "rgba(255, 220, 60, 0.20)");
      grad.addColorStop(1, "rgba(60, 220, 80, 0.10)");
    } else {
      grad.addColorStop(0, "rgba(255, 80, 80, 0.9)");
      grad.addColorStop(0.5, "rgba(255, 220, 60, 0.85)");
      grad.addColorStop(1, "rgba(60, 220, 80, 0.8)");
    }

    const now = performance.now();

    // Draw left channel (or silence if no data yet)
    const leftBins = bins?.left ?? (new Array(SPECTRUM_BINS).fill(0) as number[]);
    const rightBins = bins?.right ?? leftBins;

    drawChannel(ctx, leftBins, peakLeft, peakLeftTime, plotW, plotH, grad, now);

    // Draw right channel (blended on top, slightly offset — same shape, different color)
    if (bins?.right) {
      ctx.save();
      ctx.globalAlpha = overlay() ? 0.5 : 0.4;
      const rightGrad = ctx.createLinearGradient(0, 0, 0, plotH);
      if (overlay()) {
        rightGrad.addColorStop(0, "rgba(80, 160, 255, 0.25)");
        rightGrad.addColorStop(1, "rgba(80, 160, 255, 0.08)");
      } else {
        rightGrad.addColorStop(0, "rgba(80, 160, 255, 0.7)");
        rightGrad.addColorStop(1, "rgba(80, 160, 255, 0.3)");
      }
      drawChannel(ctx, rightBins, peakRight, peakRightTime, plotW, plotH, rightGrad, now);
      ctx.restore();
    }
  }

  function drawChannel(
    ctx: CanvasRenderingContext2D,
    bins: number[],
    peaks: Float32Array,
    peakTimes: Float32Array,
    plotW: number,
    plotH: number,
    grad: CanvasGradient,
    now: number,
  ): void {
    ctx.beginPath();
    ctx.moveTo(0, plotH);

    for (let i = 0; i < SPECTRUM_BINS; i++) {
      const x = binToX(i, plotW);
      const db = magToDb(bins[i] ?? 0);
      const y = dbToY(db, plotH);

      // Update peak hold
      if (db > peaks[i]!) {
        peaks[i] = db;
        peakTimes[i] = now;
      } else if (now - peakTimes[i]! > PEAK_HOLD_MS) {
        peaks[i] = Math.max(MIN_DB, (peaks[i] ?? MIN_DB) - PEAK_DECAY_RATE * plotH);
      }

      if (i === 0) {
        ctx.moveTo(x, y);
      } else {
        ctx.lineTo(x, y);
      }
    }

    ctx.lineTo(plotW, plotH);
    ctx.closePath();
    ctx.fillStyle = grad;
    ctx.fill();

    // Peak hold lines
    if (!overlay()) {
      ctx.save();
      ctx.strokeStyle = "rgba(255, 255, 255, 0.7)";
      ctx.lineWidth = 1;
      for (let i = 1; i < SPECTRUM_BINS - 1; i++) {
        const x = binToX(i, plotW);
        const py = dbToY(peaks[i]!, plotH);
        ctx.beginPath();
        ctx.moveTo(x - 1, py);
        ctx.lineTo(x + 1, py);
        ctx.stroke();
      }
      ctx.restore();
    }
  }

  function drawLabels(
    ctx: CanvasRenderingContext2D,
    padLeft: number,
    plotW: number,
    plotH: number,
    _w: number,
    _h: number,
  ): void {
    ctx.save();
    ctx.font = "10px var(--font-mono, monospace)";
    ctx.fillStyle = "rgba(255,255,255,0.4)";
    ctx.textBaseline = "middle";

    // dB labels (left margin)
    ctx.textAlign = "right";
    for (const db of DB_TICKS) {
      const y = dbToY(db, plotH);
      ctx.fillText(`${db}`, padLeft - 4, y);
    }

    // Frequency labels (bottom)
    ctx.textAlign = "center";
    ctx.textBaseline = "top";
    for (const freq of FREQ_LABELS) {
      const x = padLeft + freqToX(freq, plotW);
      ctx.fillText(formatFreq(freq), x, plotH + 4);
    }

    ctx.restore();
  }

  return (
    <canvas
      ref={canvasRef}
      data-testid="spectrum-canvas"
      style={{
        width: `${width()}px`,
        height: `${height()}px`,
        display: "block",
        background: overlay() ? "transparent" : undefined,
      }}
      aria-label={`Spectrum analyzer for ${props.nodeKey}`}
      role="img"
    />
  );
}
