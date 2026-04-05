/**
 * Full-page EQ & Effects view — slides in when user clicks EQ on any node.
 * Contains: back button, monitor toggle, EQ panel, effects blocks.
 * Monitor mode: solo this audio path, muting everything else.
 */
import { Show } from "solid-js";
import { Headphones, ArrowLeft } from "lucide-solid";
import EqPanel from "./EqPanel";
import EffectsBlock from "./EffectsBlock";
import type { SourceType } from "./EffectsBlock";
import type { EndpointDescriptor } from "../types/session";
import type { EqConfig } from "../types/eq";
import type { EffectsConfig } from "../types/effects";
import type { Command } from "../types/commands";
import { useSession } from "../stores/sessionStore";
import { useMonitor } from "../stores/monitorStore";
import { useMonitorOrchestration } from "../hooks/useMonitorOrchestration";

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
  /**
   * When provided, a SpectrumAnalyzer overlay is rendered behind the EQ graph.
   * Must be a valid /ws/spectrum node key.
   */
  spectrumNodeKey?: string;
}

interface EqPageProps {
  target: EqPageTarget;
  onBack: () => void;
  /** Send a command to the backend via WebSocket. */
  send: (cmd: Command) => void;
}

export default function EqPage(props: EqPageProps) {
  const { state } = useSession();
  const monitorStore = useMonitor();

  const { isMonitoringActive, toggleMonitoring, disableMonitoring } = useMonitorOrchestration({
    getSession: () => state.session,
    monitorStore,
    getSend: () => props.send,
    getSinkDescriptor: () => props.target.sinkDescriptor,
    getCellSource: () => props.target.cellSource,
    getEndpoint: () => props.target.endpoint,
  });

  function handleBack() {
    if (monitorStore.state.monitoredCell !== null && isMonitoringActive()) disableMonitoring();
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
            "background-color": isMonitoringActive()
              ? props.target.color
              : "var(--color-bg-elevated)",
            color: isMonitoringActive()
              ? "var(--color-text-primary)"
              : "var(--color-text-secondary)",
            border: `1px solid ${isMonitoringActive() ? props.target.color : "var(--color-border)"}`,
          }}
          onClick={toggleMonitoring}
          title={
            isMonitoringActive()
              ? "Stop monitoring — unmute all other audio paths"
              : "Monitor — solo this audio path, mute everything else"
          }
        >
          <Headphones size={14} />
          <span>{isMonitoringActive() ? "Monitoring" : "Monitor"}</span>
          <Show when={isMonitoringActive()}>
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
          category={props.target.sourceType === "mic" ? "mic" : props.target.sourceType}
          spectrumNodeKey={props.target.spectrumNodeKey}
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
