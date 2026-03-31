/**
 * Interactive SVG parametric EQ frequency response graph.
 * Sonar-style draggable colored dots on a log-frequency / dB grid.
 */
import { createMemo, For, Show } from "solid-js";
import type { EqBand } from "./math";
import {
  freqToX,
  xToFreq,
  dbToY,
  yToDb,
  bandCurvePath,
  compositeCurvePath,
  formatFreq,
  FREQ_REGIONS,
  FREQ_GRIDLINES,
} from "./math";

const DB_RANGE = 12;
const DB_LINES = [-12, -6, 0, 6, 12];

/** Padding around the plot area for axis labels. */
const PAD = { left: 38, right: 6, top: 0, bottom: 18 };
/** Height of the frequency region header bar. */
const HEADER_H = 22;

interface EqGraphProps {
  bands: EqBand[];
  selectedBandId: number | null;
  onBandMove: (id: number, freq: number, gain: number) => void;
  onBandSelect: (id: number | null) => void;
  onBandQChange?: (id: number, q: number) => void;
  width?: number;
  height?: number;
  readonly?: boolean;
}

export default function EqGraph(props: EqGraphProps) {
  const totalW = () => props.width ?? 720;
  const totalH = () => props.height ?? 280;
  /** Inner plot area dimensions (where curves and dots live). */
  const plotW = () => totalW() - PAD.left - PAD.right;
  const plotH = () => totalH() - PAD.top - HEADER_H - PAD.bottom;
  const plotX = () => PAD.left;
  const plotY = () => PAD.top + HEADER_H;

  let svgRef: SVGSVGElement | undefined;
  let dragState: { bandId: number; active: boolean } | null = null;

  // --- Computed paths (in plot-local coordinates) ---
  const bandPaths = createMemo(() =>
    props.bands.map((b) => ({
      id: b.id,
      d: bandCurvePath(b, plotW(), plotH(), DB_RANGE),
      color: b.color,
      enabled: b.enabled,
    })),
  );

  const compositePath = createMemo(() =>
    compositeCurvePath(props.bands, plotW(), plotH(), DB_RANGE),
  );

  // --- Drag handlers ---
  const onPointerDown = (bandId: number, e: PointerEvent) => {
    if (props.readonly) return;
    (e.target as SVGElement).setPointerCapture(e.pointerId);
    dragState = { bandId, active: true };
    props.onBandSelect(bandId);
    e.preventDefault();
  };

  const onPointerMove = (e: PointerEvent) => {
    if (!dragState?.active || !svgRef) return;
    const rect = svgRef.getBoundingClientRect();
    const scaleX = totalW() / rect.width;
    const scaleY = totalH() / rect.height;
    // Convert screen coords → SVG coords → plot-local coords, clamped to plot area
    const svgX = (e.clientX - rect.left) * scaleX;
    const svgY = (e.clientY - rect.top) * scaleY;
    const localX = Math.max(0, Math.min(plotW(), svgX - plotX()));
    const localY = Math.max(0, Math.min(plotH(), svgY - plotY()));
    const freq = Math.max(20, Math.min(20000, xToFreq(localX, plotW())));
    const gain = Math.max(-DB_RANGE, Math.min(DB_RANGE, yToDb(localY, plotH(), DB_RANGE)));
    props.onBandMove(dragState.bandId, Math.round(freq * 10) / 10, Math.round(gain * 10) / 10);
  };

  const onPointerUp = () => {
    dragState = null;
  };

  const onWheel = (bandId: number, e: WheelEvent) => {
    if (props.readonly || !props.onBandQChange) return;
    e.preventDefault();
    const band = props.bands.find((b) => b.id === bandId);
    if (!band) return;
    const delta = e.deltaY > 0 ? -0.1 : 0.1;
    const newQ = Math.max(0.1, Math.min(10, band.q + delta));
    props.onBandQChange(bandId, Math.round(newQ * 100) / 100);
  };

  return (
    <svg
      ref={svgRef}
      viewBox={`0 0 ${totalW()} ${totalH()}`}
      class="w-full select-none"
      style={{ "aspect-ratio": `${totalW()} / ${totalH()}` }}
      onPointerMove={onPointerMove}
      onPointerUp={onPointerUp}
      onPointerLeave={onPointerUp}
    >
      <defs>
        <linearGradient id="eq-composite-fill" x1="0" y1="0" x2="0" y2="1">
          <stop offset="0%" stop-color="var(--color-accent)" stop-opacity="0.15" />
          <stop offset="50%" stop-color="var(--color-accent)" stop-opacity="0.02" />
          <stop offset="100%" stop-color="var(--color-accent)" stop-opacity="0.15" />
        </linearGradient>
        <clipPath id="eq-plot-clip">
          <rect x={0} y={0} width={plotW()} height={plotH()} />
        </clipPath>
      </defs>

      {/* Full background */}
      <rect width={totalW()} height={totalH()} rx="6" fill="var(--color-bg-secondary)" />

      {/* Frequency region header (spans full width above plot) */}
      <For each={FREQ_REGIONS}>
        {(region) => {
          const x1 = plotX() + freqToX(region.start, plotW());
          const x2 = plotX() + freqToX(region.end, plotW());
          return (
            <>
              <rect
                x={x1}
                y={PAD.top}
                width={x2 - x1}
                height={HEADER_H}
                fill="var(--color-bg-elevated)"
                stroke="var(--color-border)"
                stroke-width="0.5"
              />
              <text
                x={(x1 + x2) / 2}
                y={PAD.top + 14}
                text-anchor="middle"
                fill="var(--color-text-muted)"
                font-size="9"
                font-family="var(--font-sans)"
                letter-spacing="0.5"
              >
                {region.label}
              </text>
            </>
          );
        }}
      </For>

      {/* dB grid lines (horizontal) + labels in left margin */}
      <For each={DB_LINES}>
        {(db) => {
          const y = plotY() + dbToY(db, plotH(), DB_RANGE);
          return (
            <>
              <line
                x1={plotX()}
                y1={y}
                x2={plotX() + plotW()}
                y2={y}
                stroke={db === 0 ? "var(--color-text-muted)" : "var(--color-border)"}
                stroke-width={db === 0 ? 1 : 0.5}
                opacity={db === 0 ? 0.4 : 1}
              />
              <text
                x={PAD.left - 4}
                y={y + 3}
                text-anchor="end"
                fill="var(--color-text-muted)"
                font-size="8"
                font-family="var(--font-mono)"
              >
                {db > 0 ? `+${db}` : db} dB
              </text>
            </>
          );
        }}
      </For>

      {/* Frequency grid lines (vertical) + labels in bottom margin */}
      <For each={FREQ_GRIDLINES}>
        {(freq) => {
          const x = plotX() + freqToX(freq, plotW());
          return (
            <>
              <line
                x1={x}
                y1={plotY()}
                x2={x}
                y2={plotY() + plotH()}
                stroke="var(--color-border)"
                stroke-width="0.5"
              />
              <text
                x={x}
                y={plotY() + plotH() + 12}
                text-anchor="middle"
                fill="var(--color-text-muted)"
                font-size="8"
                font-family="var(--font-mono)"
              >
                {formatFreq(freq)}
              </text>
            </>
          );
        }}
      </For>

      {/* Clipped plot area for curves and dots */}
      <g transform={`translate(${plotX()},${plotY()})`} clip-path="url(#eq-plot-clip)">
        {/* Per-band curves */}
        <For each={bandPaths()}>
          {(bp) => (
            <Show when={bp.enabled && bp.d}>
              <path
                d={bp.d}
                fill="none"
                stroke={bp.color}
                stroke-width="1.5"
                opacity={props.selectedBandId === bp.id ? 0.9 : 0.4}
              />
            </Show>
          )}
        </For>

        {/* Composite curve fill */}
        <path
          d={`${compositePath()}L${plotW()},${dbToY(0, plotH(), DB_RANGE)}L0,${dbToY(0, plotH(), DB_RANGE)}Z`}
          fill="url(#eq-composite-fill)"
        />
        {/* Composite curve stroke */}
        <path
          d={compositePath()}
          fill="none"
          stroke="var(--color-text-primary)"
          stroke-width="1.5"
          opacity="0.8"
        />

        {/* Draggable band dots */}
        <For each={props.bands}>
          {(band) => {
            const cx = () => freqToX(band.frequency, plotW());
            const cy = () => dbToY(band.gain, plotH(), DB_RANGE);
            const isSelected = () => props.selectedBandId === band.id;
            return (
              <Show when={band.enabled}>
                <g
                  style={{ cursor: props.readonly ? "default" : "grab" }}
                  onPointerDown={[onPointerDown, band.id]}
                  onWheel={[onWheel, band.id]}
                >
                  {/* Hover/selection ring */}
                  <circle
                    cx={cx()}
                    cy={cy()}
                    r={isSelected() ? 14 : 10}
                    fill={band.color}
                    opacity={isSelected() ? 0.15 : 0}
                    class="transition-opacity duration-150"
                  />
                  {/* Main dot */}
                  <circle
                    cx={cx()}
                    cy={cy()}
                    r={isSelected() ? 7 : 5.5}
                    fill={band.color}
                    stroke={isSelected() ? "var(--color-text-primary)" : "none"}
                    stroke-width="1.5"
                    class="transition-all duration-100"
                  />
                </g>
              </Show>
            );
          }}
        </For>
      </g>
    </svg>
  );
}
