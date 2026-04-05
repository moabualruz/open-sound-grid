/**
 * MixEffectsRow — inline effects overview rendered below each matrix cell row
 * when a mix column is expanded. Shows EQ thumbnail + active effects badges
 * for every channel×mix cell in that column.
 *
 * Layout: one cell per channel, horizontally aligned with the matrix grid.
 * The first column (channel label placeholder) is empty to match the grid offset.
 */
import { For, Show } from "solid-js";
import type { JSX } from "solid-js";
import type { EqConfig } from "../types/eq";
import type { EffectsConfig } from "../types/effects";
import type { EndpointDescriptor } from "../types/session";
import EqThumbnail from "./EqThumbnail";

// ---------------------------------------------------------------------------
// Effects badge definitions — one badge per effect type
// ---------------------------------------------------------------------------

interface EffectBadgeDef {
  key: keyof Omit<EffectsConfig, "boost" | "spatial">;
  label: string;
  /** Color when active/enabled. */
  activeColor: string;
}

const EFFECT_BADGE_DEFS: EffectBadgeDef[] = [
  { key: "compressor", label: "Comp", activeColor: "#5090e0" },
  { key: "gate", label: "Gate", activeColor: "#60c060" },
  { key: "deEsser", label: "DeEss", activeColor: "#e08850" },
  { key: "limiter", label: "Lim", activeColor: "#f44336" },
  { key: "smartVolume", label: "SV", activeColor: "#ffeb3b" },
];

interface EffectsBadgesProps {
  effects: EffectsConfig | undefined;
}

/**
 * Determines if an effect is at its default "off" state (no dot shown)
 * vs. present but disabled (gray dot) vs. active (colored dot).
 */
function getEffectState(
  effects: EffectsConfig | undefined,
  key: keyof Omit<EffectsConfig, "boost" | "spatial">,
): "active" | "inactive" | "off" {
  if (!effects) return "off";
  const cfg = effects[key];
  if (!cfg) return "off";
  // Check if enabled
  return (cfg as { enabled: boolean }).enabled ? "active" : "inactive";
}

function hasBoostActive(effects: EffectsConfig | undefined): boolean {
  return !!effects && effects.boost > 0;
}

export function EffectsBadges(props: EffectsBadgesProps): JSX.Element {
  return (
    <div class="flex items-center gap-0.5" aria-label="Active effects">
      <For each={EFFECT_BADGE_DEFS}>
        {(def) => {
          const state = () => getEffectState(props.effects, def.key);
          return (
            <Show when={state() !== "off"}>
              <span
                title={`${def.label}: ${state() === "active" ? "active" : "disabled"}`}
                aria-label={`${def.label} ${state() === "active" ? "active" : "disabled"}`}
                style={{
                  display: "inline-block",
                  width: "6px",
                  height: "6px",
                  "border-radius": "50%",
                  "background-color":
                    state() === "active" ? def.activeColor : "var(--color-text-muted)",
                  opacity: state() === "active" ? 1 : 0.45,
                  transition: "background-color 150ms ease",
                }}
              />
            </Show>
          );
        }}
      </For>
      {/* Boost badge — yellow dot when boost > 0 */}
      <Show when={hasBoostActive(props.effects)}>
        <span
          title="Volume Boost: active"
          aria-label="Volume Boost active"
          style={{
            display: "inline-block",
            width: "6px",
            height: "6px",
            "border-radius": "50%",
            "background-color": "#e0c050",
            transition: "background-color 150ms ease",
          }}
        />
      </Show>
    </div>
  );
}

// ---------------------------------------------------------------------------
// Cell data passed from parent
// ---------------------------------------------------------------------------

export interface MixEffectsCellData {
  /** Source channel descriptor — for building the EQ page target. */
  sourceDescriptor: EndpointDescriptor;
  /** Cell EQ config (from link.cellEq). */
  cellEq?: EqConfig;
  /** Cell effects config (from link.cellEffects). */
  cellEffects?: EffectsConfig;
  /** Whether the cell is linked at all. */
  linked: boolean;
}

// ---------------------------------------------------------------------------
// MixEffectsRow component
// ---------------------------------------------------------------------------

interface MixEffectsRowProps {
  /** Data for each channel in this mix column. Order matches channel order. */
  cells: MixEffectsCellData[];
  /** Mix accent color — used for EQ thumbnail stroke. */
  mixColor: string;
  /** Grid template columns — must match the parent matrix grid. */
  gridTemplateColumns: string;
  /**
   * Called when the user clicks an EQ thumbnail. Passes the source descriptor
   * so the parent can open EqPage for that cell.
   */
  onOpenCellEq: (sourceDescriptor: EndpointDescriptor) => void;
}

export default function MixEffectsRow(props: MixEffectsRowProps): JSX.Element {
  return (
    <div
      class="grid gap-2 overflow-hidden"
      style={{
        "grid-template-columns": props.gridTemplateColumns,
        animation: "osg-effects-row-expand 200ms ease both",
      }}
      role="row"
      aria-label="Effects overview row"
    >
      {/* Empty first column — matches channel label column */}
      <div aria-hidden="true" />

      {/* One cell per channel in this mix column */}
      <For each={props.cells}>
        {(cell) => (
          <div
            class="flex items-center gap-1.5 rounded-md px-2 py-1"
            style={{
              "background-color": "var(--color-bg-primary)",
              border: "1px solid var(--color-border)",
              "min-height": "40px",
            }}
            role="gridcell"
          >
            <Show
              when={cell.linked}
              fallback={
                <div
                  style={{
                    width: "60px",
                    height: "30px",
                    "background-color": "var(--color-bg-empty-cell)",
                    "border-radius": "3px",
                    opacity: 0.4,
                  }}
                  aria-label="No route"
                />
              }
            >
              <div class="flex flex-col gap-0.5">
                <EqThumbnail
                  eq={cell.cellEq}
                  color={props.mixColor}
                  width={60}
                  height={28}
                  onClick={() => props.onOpenCellEq(cell.sourceDescriptor)}
                  aria-label="Open EQ for this cell"
                />
                <EffectsBadges effects={cell.cellEffects} />
              </div>
            </Show>
          </div>
        )}
      </For>
    </div>
  );
}
