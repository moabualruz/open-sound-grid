/**
 * Biquad filter math for parametric EQ visualization.
 * Based on Robert Bristow-Johnson's Audio EQ Cookbook.
 * Pure math — no Web Audio API dependency. The browser is a remote control;
 * actual audio processing happens in PipeWire filter-chain on the server.
 */

export type FilterType = "peaking" | "lowShelf" | "highShelf" | "lowPass" | "highPass" | "notch";

export interface EqBand {
  id: number;
  enabled: boolean;
  type: FilterType;
  frequency: number; // Hz (20–20000)
  gain: number; // dB (±12)
  q: number; // 0.1–10
  color: string;
}

export interface BiquadCoeffs {
  b0: number;
  b1: number;
  b2: number;
  a0: number;
  a1: number;
  a2: number;
}

const TWO_PI = 2 * Math.PI;

/** Compute biquad coefficients for a given filter type (RBJ cookbook). */
export function computeCoefficients(
  type: FilterType,
  freq: number,
  gain: number,
  q: number,
  sampleRate: number,
): BiquadCoeffs {
  const A = Math.pow(10, gain / 40); // sqrt of linear gain
  const w0 = (TWO_PI * freq) / sampleRate;
  const sinW0 = Math.sin(w0);
  const cosW0 = Math.cos(w0);
  const alpha = sinW0 / (2 * q);

  let b0: number, b1: number, b2: number;
  let a0: number, a1: number, a2: number;

  switch (type) {
    case "peaking":
      b0 = 1 + alpha * A;
      b1 = -2 * cosW0;
      b2 = 1 - alpha * A;
      a0 = 1 + alpha / A;
      a1 = -2 * cosW0;
      a2 = 1 - alpha / A;
      break;

    case "lowShelf": {
      const sqrtA = Math.sqrt(A);
      const twoSqrtAAlpha = 2 * sqrtA * alpha;
      b0 = A * (A + 1 - (A - 1) * cosW0 + twoSqrtAAlpha);
      b1 = 2 * A * (A - 1 - (A + 1) * cosW0);
      b2 = A * (A + 1 - (A - 1) * cosW0 - twoSqrtAAlpha);
      a0 = A + 1 + (A - 1) * cosW0 + twoSqrtAAlpha;
      a1 = -2 * (A - 1 + (A + 1) * cosW0);
      a2 = A + 1 + (A - 1) * cosW0 - twoSqrtAAlpha;
      break;
    }

    case "highShelf": {
      const sqrtA = Math.sqrt(A);
      const twoSqrtAAlpha = 2 * sqrtA * alpha;
      b0 = A * (A + 1 + (A - 1) * cosW0 + twoSqrtAAlpha);
      b1 = -2 * A * (A - 1 + (A + 1) * cosW0);
      b2 = A * (A + 1 + (A - 1) * cosW0 - twoSqrtAAlpha);
      a0 = A + 1 - (A - 1) * cosW0 + twoSqrtAAlpha;
      a1 = 2 * (A - 1 - (A + 1) * cosW0);
      a2 = A + 1 - (A - 1) * cosW0 - twoSqrtAAlpha;
      break;
    }

    case "lowPass":
      b0 = (1 - cosW0) / 2;
      b1 = 1 - cosW0;
      b2 = (1 - cosW0) / 2;
      a0 = 1 + alpha;
      a1 = -2 * cosW0;
      a2 = 1 - alpha;
      break;

    case "highPass":
      b0 = (1 + cosW0) / 2;
      b1 = -(1 + cosW0);
      b2 = (1 + cosW0) / 2;
      a0 = 1 + alpha;
      a1 = -2 * cosW0;
      a2 = 1 - alpha;
      break;

    case "notch":
      b0 = 1;
      b1 = -2 * cosW0;
      b2 = 1;
      a0 = 1 + alpha;
      a1 = -2 * cosW0;
      a2 = 1 - alpha;
      break;
  }

  return { b0, b1, b2, a0, a1, a2 };
}

/** Evaluate magnitude response in dB at a given frequency. */
export function magnitudeAt(coeffs: BiquadCoeffs, freq: number, sampleRate: number): number {
  const w = (TWO_PI * freq) / sampleRate;
  const cosW = Math.cos(w);
  const cos2W = Math.cos(2 * w);
  const sinW = Math.sin(w);
  const sin2W = Math.sin(2 * w);

  const { b0, b1, b2, a0, a1, a2 } = coeffs;
  const numReal = b0 + b1 * cosW + b2 * cos2W;
  const numImag = -(b1 * sinW + b2 * sin2W);
  const denReal = a0 + a1 * cosW + a2 * cos2W;
  const denImag = -(a1 * sinW + a2 * sin2W);

  const numMag = numReal * numReal + numImag * numImag;
  const denMag = denReal * denReal + denImag * denImag;

  if (denMag === 0) return 0;
  return 10 * Math.log10(numMag / denMag);
}

// ---------------------------------------------------------------------------
// Coordinate mapping — log-scale frequency, linear dB
// ---------------------------------------------------------------------------

const MIN_FREQ = 20;
const MAX_FREQ = 20000;
const LOG_MIN = Math.log10(MIN_FREQ);
const LOG_MAX = Math.log10(MAX_FREQ);
const LOG_RANGE = LOG_MAX - LOG_MIN;

export function freqToX(freq: number, width: number): number {
  return ((Math.log10(Math.max(MIN_FREQ, Math.min(MAX_FREQ, freq))) - LOG_MIN) / LOG_RANGE) * width;
}

export function xToFreq(x: number, width: number): number {
  return Math.pow(10, LOG_MIN + (x / width) * LOG_RANGE);
}

export function dbToY(db: number, height: number, range: number): number {
  return height / 2 - (db / range) * (height / 2);
}

export function yToDb(y: number, height: number, range: number): number {
  return -((y - height / 2) / (height / 2)) * range;
}

// ---------------------------------------------------------------------------
// Curve generation
// ---------------------------------------------------------------------------

const SAMPLE_RATE = 48000;
const CURVE_POINTS = 256;

/** Generate SVG path data for a single band's frequency response. */
export function bandCurvePath(
  band: EqBand,
  width: number,
  height: number,
  dbRange: number,
): string {
  if (!band.enabled) return "";
  const coeffs = computeCoefficients(band.type, band.frequency, band.gain, band.q, SAMPLE_RATE);
  const parts: string[] = [];
  for (let i = 0; i <= CURVE_POINTS; i++) {
    const x = (i / CURVE_POINTS) * width;
    const freq = xToFreq(x, width);
    const db = magnitudeAt(coeffs, freq, SAMPLE_RATE);
    const y = dbToY(db, height, dbRange);
    parts.push(`${i === 0 ? "M" : "L"}${x.toFixed(1)},${y.toFixed(1)}`);
  }
  return parts.join("");
}

/** Generate SVG path data for the composite response of all bands. */
export function compositeCurvePath(
  bands: EqBand[],
  width: number,
  height: number,
  dbRange: number,
): string {
  const enabledBands = bands.filter((b) => b.enabled);
  if (enabledBands.length === 0) {
    const zeroY = dbToY(0, height, dbRange);
    return `M0,${zeroY.toFixed(1)}L${width},${zeroY.toFixed(1)}`;
  }

  const allCoeffs = enabledBands.map((b) =>
    computeCoefficients(b.type, b.frequency, b.gain, b.q, SAMPLE_RATE),
  );

  const parts: string[] = [];
  for (let i = 0; i <= CURVE_POINTS; i++) {
    const x = (i / CURVE_POINTS) * width;
    const freq = xToFreq(x, width);
    let totalDb = 0;
    for (const c of allCoeffs) {
      totalDb += magnitudeAt(c, freq, SAMPLE_RATE);
    }
    const y = dbToY(totalDb, height, dbRange);
    parts.push(`${i === 0 ? "M" : "L"}${x.toFixed(1)},${y.toFixed(1)}`);
  }
  return parts.join("");
}

/** Frequency region labels for the grid header. */
export const FREQ_REGIONS = [
  { label: "SUB BASS", start: 20, end: 60 },
  { label: "BASS", start: 60, end: 250 },
  { label: "LOW MIDS", start: 250, end: 500 },
  { label: "MID RANGE", start: 500, end: 2000 },
  { label: "UPPER MIDS", start: 2000, end: 6000 },
  { label: "HIGHS", start: 6000, end: 20000 },
] as const;

/** Major grid lines for frequency axis. */
export const FREQ_GRIDLINES = [20, 50, 100, 200, 500, 1000, 2000, 5000, 10000, 20000];

/** Format frequency for display. */
export function formatFreq(hz: number): string {
  if (hz >= 1000) return `${(hz / 1000).toFixed(hz >= 10000 ? 0 : 1)}kHz`;
  return `${Math.round(hz)}Hz`;
}

/** Default band colors (Sonar-style). */
export const BAND_COLORS = [
  "#b07fe0", // purple
  "#e08850", // orange
  "#5090e0", // blue
  "#60c060", // green
  "#40b0a0", // teal
  "#e06090", // pink
  "#50c8e0", // cyan
  "#e0c050", // yellow
  "#e05050", // red
  "#a0d050", // lime
];

/** Create a default band at a given frequency. */
export function createDefaultBand(id: number, freq: number): EqBand {
  return {
    id,
    enabled: true,
    type: "peaking",
    frequency: freq,
    gain: 0,
    q: 0.707,
    color: BAND_COLORS[id % BAND_COLORS.length],
  };
}

/** Default 5-band preset frequencies (Sonar starts with 5, expandable to 10). */
export const DEFAULT_FREQUENCIES = [80, 250, 1000, 3500, 10000];

export function createDefaultBands(): EqBand[] {
  return DEFAULT_FREQUENCIES.map((f, i) => createDefaultBand(i, f));
}
