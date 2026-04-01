/**
 * Full-page EQ & Effects view — slides in when user clicks EQ on any node.
 * Contains: back button, monitor toggle, EQ panel, effects blocks.
 * Monitor mode: solo this audio path, muting everything else.
 */
import { createSignal, onCleanup, Show } from "solid-js";
import { Headphones, ArrowLeft } from "lucide-solid";
import EqPanel from "./EqPanel";
import EffectsBlock from "./EffectsBlock";
import type { SourceType } from "./EffectsBlock";
import type { EndpointDescriptor, EqConfig, EffectsConfig, Command } from "../types";
import { useSession } from "../stores/sessionStore";

export interface EqPageTarget {
  label: string;
  sourceType: SourceType;
  color: string;
  /** Endpoint descriptor for SetEq command. */
  endpoint?: EndpointDescriptor;
  /** For cell EQ: source + target descriptors for SetCellEq. */
  cellSource?: EndpointDescriptor;
  cellTarget?: EndpointDescriptor;
  /** Current EQ config from backend (for restoring state). */
  initialEq?: EqConfig;
  /** Current effects config from backend (for restoring state). */
  initialEffects?: EffectsConfig;
  /** Sink descriptor for monitor (solo) functionality. */
  sinkDescriptor?: EndpointDescriptor;
}

interface EqPageProps {
  target: EqPageTarget;
  onBack: () => void;
  /** Send a command to the backend via WebSocket. */
  send: (cmd: Command) => void;
}

function descriptorsEqual(a: EndpointDescriptor, b: EndpointDescriptor): boolean {
  return JSON.stringify(a) === JSON.stringify(b);
}

export default function EqPage(props: EqPageProps) {
  const { state } = useSession();
  const [monitoring, setMonitoring] = createSignal(false);
  /** Links muted by monitor — stores [source, target, previousVolume]. */
  let mutedLinks: { source: EndpointDescriptor; target: EndpointDescriptor; prevVolume: number }[] = [];

  // Auto-disable monitoring when leaving the page
  onCleanup(() => {
    if (monitoring()) {
      disableMonitoring();
    }
  });

  function toggleMonitoring() {
    if (monitoring()) {
      disableMonitoring();
    } else {
      enableMonitoring();
    }
  }

  function enableMonitoring() {
    const t = props.target;
    const sinkDesc = t.sinkDescriptor;
    const sourceDesc = t.cellSource ?? t.endpoint;
    if (!sinkDesc || !sourceDesc) return;

    setMonitoring(true);
    mutedLinks = [];

    // Find all other links going to the same sink, mute them
    for (const link of state.session.links) {
      if (descriptorsEqual(link.end, sinkDesc) && !descriptorsEqual(link.start, sourceDesc)) {
        mutedLinks.push({ source: link.start, target: link.end, prevVolume: link.cellVolume });
        props.send({ type: "setLinkVolume", source: link.start, target: link.end, volume: 0 });
      }
    }
  }

  function disableMonitoring() {
    setMonitoring(false);

    // Restore all muted links to their previous volume
    for (const { source, target, prevVolume } of mutedLinks) {
      props.send({ type: "setLinkVolume", source, target, volume: prevVolume > 0 ? prevVolume : 1 });
    }
    mutedLinks = [];
  }

  function handleBack() {
    if (monitoring()) disableMonitoring();
    props.onBack();
  }

  function handleEqChange(eq: EqConfig) {
    const t = props.target;
    if (t.cellSource && t.cellTarget) {
      props.send({ type: "setCellEq", source: t.cellSource, target: t.cellTarget, eq });
    } else if (t.endpoint) {
      props.send({ type: "setEq", endpoint: t.endpoint, eq });
    }
  }

  function handleEffectsChange(effects: EffectsConfig) {
    const t = props.target;
    if (t.cellSource && t.cellTarget) {
      props.send({ type: "setCellEffects", source: t.cellSource, target: t.cellTarget, effects });
    } else if (t.endpoint) {
      props.send({ type: "setEffects", endpoint: t.endpoint, effects });
    }
  }

  return (
    <div
      class="flex flex-col h-full overflow-y-auto"
      style={{ "background-color": "var(--color-bg-primary)" }}
    >
      {/* Top bar */}
      <div
        class="flex items-center justify-between px-4 py-2.5 border-b sticky top-0 z-10"
        style={{
          "background-color": "var(--color-bg-secondary)",
          "border-color": "var(--color-border)",
        }}
      >
        {/* Left: back + label */}
        <div class="flex items-center gap-3">
          <button
            class="flex items-center gap-1.5 rounded px-2 py-1 text-xs transition-colors"
            style={{ color: "var(--color-text-secondary)" }}
            onClick={handleBack}
          >
            <ArrowLeft size={14} />
            <span>Grid</span>
          </button>
          <div class="flex items-center gap-2">
            <div class="w-3 h-3 rounded-full" style={{ "background-color": props.target.color }} />
            <span class="text-sm font-semibold" style={{ color: "var(--color-text-primary)" }}>
              {props.target.label}
            </span>
            <span
              class="rounded px-1.5 py-0.5 text-[9px] uppercase font-mono"
              style={{
                background: `${props.target.color}20`,
                color: props.target.color,
              }}
            >
              {props.target.sourceType}
            </span>
          </div>
        </div>

        {/* Right: monitor toggle */}
        <button
          class="flex items-center gap-1.5 rounded px-3 py-1.5 text-xs font-medium transition-all duration-150"
          style={{
            "background-color": monitoring() ? props.target.color : "var(--color-bg-elevated)",
            color: monitoring() ? "var(--color-text-primary)" : "var(--color-text-secondary)",
            border: `1px solid ${monitoring() ? props.target.color : "var(--color-border)"}`,
          }}
          onClick={toggleMonitoring}
          title={
            monitoring()
              ? "Stop monitoring — unmute all other audio paths"
              : "Monitor — solo this audio path, mute everything else"
          }
        >
          <Headphones size={14} />
          <span>{monitoring() ? "Monitoring" : "Monitor"}</span>
          <Show when={monitoring()}>
            <span
              class="w-1.5 h-1.5 rounded-full animate-pulse"
              style={{ "background-color": "var(--color-text-primary)" }}
            />
          </Show>
        </button>
      </div>

      {/* Content */}
      <div class="flex-1 p-4 max-w-4xl mx-auto w-full flex flex-col gap-3">
        <EqPanel
          label={props.target.label}
          color={props.target.color}
          initialEq={props.target.initialEq}
          onEqChange={handleEqChange}
        />
        <EffectsBlock
          sourceType={props.target.sourceType}
          color={props.target.color}
          initialEffects={props.target.initialEffects}
          onEffectsChange={handleEffectsChange}
        />
      </div>
    </div>
  );
}
