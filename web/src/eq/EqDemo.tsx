/**
 * EQ + Effects Demo Page — shows how audio processing looks per source type.
 * Navigate to /#eq-demo to see this in the browser.
 *
 * All nodes get the same EQ (10-band, macros, presets).
 * Effects blocks vary by source type:
 *   App:    Compressor + Smart Volume
 *   Cell:   Compressor
 *   Mix:    Compressor + Limiter + Smart Volume
 */
import { For } from "solid-js";
import EqPanel from "./EqPanel";
import EffectsBlock from "./EffectsBlock";
import type { SourceType } from "./EffectsBlock";

interface SourceDemo {
  type: SourceType;
  title: string;
  subtitle: string;
  color: string;
  label: string;
}

/** Resolve CSS variable to its computed value for inline style props. */
function cssVar(name: string): string {
  return `var(${name})`;
}

const SOURCES: SourceDemo[] = [
  {
    type: "app",
    title: "Application / Channel",
    subtitle: "Software audio source routed through a channel filter",
    color: cssVar("--color-source-app"),
    label: "Game Audio",
  },
  {
    type: "cell",
    title: "Cell (Route)",
    subtitle: "Per-route intersection — Game → Headphones",
    color: cssVar("--color-source-cell"),
    label: "Game → Headphones",
  },
  {
    type: "mix",
    title: "Mix (Output Bus)",
    subtitle: "Final output — gets limiter and loudness control",
    color: cssVar("--color-source-mix"),
    label: "Headphones",
  },
];

export default function EqDemo() {
  return (
    <div class="min-h-screen p-4" style={{ "background-color": "var(--color-bg-primary)" }}>
      {/* Header */}
      <div class="mb-6">
        <h1 class="text-lg font-semibold mb-1" style={{ color: "var(--color-text-primary)" }}>
          Audio Processing — Feature Preview
        </h1>
        <p class="text-xs" style={{ color: "var(--color-text-muted)" }}>
          Same EQ everywhere. Effects vary by source type. Drag dots to adjust bands, scroll wheel
          for Q.
        </p>
      </div>

      {/* Signal flow */}
      <div
        class="rounded-lg border px-4 py-3 mb-6 text-xs font-mono"
        style={{
          "background-color": "var(--color-bg-secondary)",
          "border-color": "var(--color-border)",
          color: "var(--color-text-secondary)",
        }}
      >
        <div
          class="mb-1 font-sans font-medium uppercase tracking-wider text-[10px]"
          style={{ color: "var(--color-text-muted)" }}
        >
          Signal Flow
        </div>
        <div class="flex flex-wrap items-center gap-1">
          <span style={{ color: "var(--color-text-muted)" }}>Mic/App →</span>
          <Chip color={cssVar("--color-source-app")}>Channel (EQ + FX)</Chip>
          <span style={{ color: "var(--color-text-muted)" }}>→ vol →</span>
          <Chip color={cssVar("--color-source-cell")}>Cell (EQ + FX)</Chip>
          <span style={{ color: "var(--color-text-muted)" }}>→ vol →</span>
          <Chip color={cssVar("--color-source-mix")}>Mix (EQ + FX)</Chip>
          <span style={{ color: "var(--color-text-muted)" }}>→ Output</span>
        </div>
      </div>

      {/* Per-source-type panels */}
      <div class="grid grid-cols-1 xl:grid-cols-2 gap-6">
        <For each={SOURCES}>
          {(src) => (
            <div>
              <SourceHeader
                title={src.title}
                subtitle={src.subtitle}
                color={src.color}
                type={src.type}
              />
              <div class="flex flex-col gap-2">
                <EqPanel label={src.label} color={src.color} />
                <EffectsBlock sourceType={src.type} color={src.color} />
              </div>
            </div>
          )}
        </For>
      </div>

      {/* Effects availability table */}
      <div class="mt-8">
        <h2 class="text-sm font-semibold mb-2" style={{ color: "var(--color-text-primary)" }}>
          Effects Availability by Source Type
        </h2>
        <div
          class="rounded-lg border overflow-hidden text-xs"
          style={{
            "background-color": "var(--color-bg-secondary)",
            "border-color": "var(--color-border)",
          }}
        >
          <table class="w-full">
            <thead>
              <tr style={{ "background-color": "var(--color-bg-elevated)" }}>
                <Th>Effect</Th>
                <Th color={cssVar("--color-source-mic")}>Mic</Th>
                <Th color={cssVar("--color-source-app")}>App/Channel</Th>
                <Th color={cssVar("--color-source-cell")}>Cell</Th>
                <Th color={cssVar("--color-source-mix")}>Mix</Th>
              </tr>
            </thead>
            <tbody>
              <Row label="Parametric EQ (10-band)" vals={["Yes", "Yes", "Yes", "Yes"]} />
              <Row label="Macros (Bass/Voice/Treble)" vals={["Yes", "Yes", "Yes", "Yes"]} />
              <Row label="Presets (import/export)" vals={["Yes", "Yes", "Yes", "Yes"]} />
              <Row label="Test Sound" vals={["Yes", "Yes", "Yes", "Yes"]} />
              <Row label="Background Noise" vals={["Yes", "—", "—", "—"]} />
              <Row label="Impact Noise" vals={["Yes", "—", "—", "—"]} />
              <Row label="AI Noise Cancellation" vals={["Yes", "—", "—", "—"]} />
              <Row label="Noise Gate" vals={["Yes", "—", "—", "—"]} />
              <Row label="Compressor" vals={["Yes", "Yes", "Yes", "Yes"]} />
              <Row label="Limiter" vals={["—", "—", "—", "Yes"]} />
              <Row label="Smart Volume" vals={["—", "Yes", "—", "Yes"]} />
              <Row label="Volume Boost" vals={["—", "Yes", "Yes", "Yes"]} />
              <Row label="Spatial Audio (HRTF)" vals={["—", "—", "—", "Yes"]} />
            </tbody>
          </table>
        </div>
      </div>
    </div>
  );
}

function Chip(props: { color: string; children: string }) {
  return (
    <span
      class="rounded px-1.5 py-0.5"
      style={{ background: `${props.color}20`, color: props.color }}
    >
      {props.children}
    </span>
  );
}

function SourceHeader(props: { title: string; subtitle: string; color: string; type: SourceType }) {
  return (
    <div class="mb-2">
      <div class="flex items-baseline gap-2 mb-0.5">
        <div
          class="w-2.5 h-2.5 rounded-full shrink-0"
          style={{ "background-color": props.color }}
        />
        <h2 class="text-sm font-semibold" style={{ color: "var(--color-text-primary)" }}>
          {props.title}
        </h2>
        <span
          class="rounded px-1.5 py-0.5 text-[9px] uppercase font-mono"
          style={{ background: `${props.color}20`, color: props.color }}
        >
          {props.type}
        </span>
      </div>
      <p class="text-[10px] ml-4.5" style={{ color: "var(--color-text-muted)" }}>
        {props.subtitle}
      </p>
    </div>
  );
}

function Th(props: { children: string; color?: string }) {
  return (
    <th
      class="px-3 py-2 text-center font-medium"
      style={{ color: props.color ?? "var(--color-text-muted)" }}
    >
      {props.children}
    </th>
  );
}

function Row(props: { label: string; vals: string[] }) {
  return (
    <tr style={{ "border-top": "1px solid var(--color-border)" }}>
      <td
        class="px-3 py-1.5 font-medium text-left"
        style={{ color: "var(--color-text-secondary)" }}
      >
        {props.label}
      </td>
      <For each={props.vals}>
        {(v) => (
          <td
            class="px-3 py-1.5 text-center"
            style={{ color: v === "—" ? "var(--color-text-muted)" : "var(--color-text-secondary)" }}
          >
            {v}
          </td>
        )}
      </For>
    </tr>
  );
}
