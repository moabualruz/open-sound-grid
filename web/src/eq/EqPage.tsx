/**
 * Full-page EQ & Effects view — slides in when user clicks EQ on any node.
 * Contains: back button, monitor toggle, EQ panel, effects blocks.
 * Monitor mode: solo this audio path, muting everything else.
 */
import { onCleanup, Show } from "solid-js";
import { Headphones, ArrowLeft } from "lucide-solid";
import EqPanel from "./EqPanel";
import EffectsBlock from "./EffectsBlock";
import type { SourceType } from "./EffectsBlock";
import type { EndpointDescriptor, EqConfig, EffectsConfig, Command } from "../types";
import { useSession } from "../stores/sessionStore";
import { useMonitor } from "../stores/monitorStore";
import { computeMutedLinks, computeRestoreVolumes } from "./monitorLogic";

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
  const monitor = useMonitor();
  /** Track which links were muted by monitoring (for restore). Only stores source+target IDs. */
  let mutedLinkIds: { source: EndpointDescriptor; target: EndpointDescriptor }[] = [];
  /** Track endpoint mute states for mix monitoring. */
  let mutedEndpoints: { endpoint: EndpointDescriptor; wasMuted: boolean }[] = [];

  // Auto-disable monitoring when leaving the page
  onCleanup(() => {
    if (monitor.state.monitoredCell !== null && isMonitoringActive()) {
      disableMonitoring();
    }
  });

  function isMonitoringActive() {
    const t = props.target;
    const sinkDesc = t.sinkDescriptor;
    if (!sinkDesc) return false;
    if (t.cellSource) {
      return (
        monitor.state.monitoredCell !== null &&
        descriptorsEqual(monitor.state.monitoredCell!.source, t.cellSource) &&
        descriptorsEqual(monitor.state.monitoredCell!.target, sinkDesc)
      );
    }
    // For mix monitoring, check if any monitored cell targets this mix
    return monitor.state.monitoredCell !== null;
  }

  function toggleMonitoring() {
    if (monitor.state.monitoredCell !== null && isMonitoringActive()) {
      disableMonitoring();
    } else {
      enableMonitoring();
    }
  }

  function enableMonitoring() {
    const t = props.target;
    const sinkDesc = t.sinkDescriptor;
    if (!sinkDesc) return;

    if (t.cellSource) {
      // Cell monitoring: mute ALL other links across ALL mixes
      const sourceDesc = t.cellSource;
      const result = computeMutedLinks(state.session.links, sourceDesc, sinkDesc);

      // Boost the monitored cell to 100%
      if (result.monitoredLink) {
        props.send({
          type: "setLinkVolume",
          source: result.monitoredLink.source,
          target: result.monitoredLink.target,
          volume: 1,
        });
      }

      // Mute all other links
      for (const m of result.linksToMute) {
        props.send({ type: "setLinkVolume", source: m.source, target: m.target, volume: 0 });
      }

      // Track muted link IDs for restore (not volumes — we'll re-read at restore time)
      mutedLinkIds = result.linksToMute.map((m) => ({ source: m.source, target: m.target }));

      monitor.startMonitoring(sourceDesc, sinkDesc);
    } else if (t.endpoint) {
      // Mix monitoring: mute ALL OTHER mix endpoints, unmute this one
      const thisMixDesc = sinkDesc;
      mutedEndpoints = [];
      for (const [desc, ep] of state.session.endpoints) {
        if (!("channel" in desc)) continue;
        const ch = state.session.channels[desc.channel];
        if (!ch || ch.kind !== "sink") continue;
        const isMuted =
          ep.volumeLockedMuted === "mutedLocked" || ep.volumeLockedMuted === "mutedUnlocked";
        if (descriptorsEqual(desc, thisMixDesc)) {
          if (isMuted) {
            mutedEndpoints.push({ endpoint: desc, wasMuted: true });
            props.send({ type: "setMute", endpoint: desc, muted: false });
          }
        } else {
          mutedEndpoints.push({ endpoint: desc, wasMuted: isMuted });
          if (!isMuted) {
            props.send({ type: "setMute", endpoint: desc, muted: true });
          }
        }
      }
      monitor.startMonitoring(t.endpoint, sinkDesc);
    }
  }

  function disableMonitoring() {
    // Restore cell link volumes by re-reading current state
    const restore = computeRestoreVolumes(state.session.links, mutedLinkIds);
    for (const cmd of restore.commands) {
      props.send({
        type: "setLinkVolume",
        source: cmd.source,
        target: cmd.target,
        volume: cmd.volume,
      });
    }
    mutedLinkIds = [];

    // Restore mix mute states
    for (const { endpoint, wasMuted } of mutedEndpoints) {
      props.send({ type: "setMute", endpoint, muted: wasMuted });
    }
    mutedEndpoints = [];

    monitor.stopMonitoring();
  }

  function handleBack() {
    if (monitor.state.monitoredCell !== null && isMonitoringActive()) disableMonitoring();
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
