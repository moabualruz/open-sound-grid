/**
 * Full-page EQ & Effects view — slides in when user clicks EQ on any node.
 * Contains: back button, monitor toggle, EQ panel, effects blocks.
 * Monitor mode: solo this audio path, muting everything else.
 */
import { createSignal, onCleanup, Show, For } from "solid-js";
import { Headphones, ArrowLeft } from "lucide-solid";
import EqPanel from "./EqPanel";
import EffectsBlock from "./EffectsBlock";
import type { SourceType } from "./EffectsBlock";
import type { EndpointDescriptor, EqConfig, Command } from "../types";

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
}

interface EqPageProps {
  target: EqPageTarget;
  onBack: () => void;
  /** Send a command to the backend via WebSocket. */
  send: (cmd: Command) => void;
}

export default function EqPage(props: EqPageProps) {
  const [monitoring, setMonitoring] = createSignal(false);

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
    setMonitoring(true);
    // TODO: send solo command to backend
    // send({ type: "soloNode", nodeId: props.target.nodeId, solo: true })
  }

  function disableMonitoring() {
    setMonitoring(false);
    // TODO: send unsolo command to backend
    // send({ type: "soloNode", nodeId: props.target.nodeId, solo: false })
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
        {/* Mix gets pre/post fader toggle with two EQ instances */}
        <Show
          when={props.target.sourceType === "mix"}
          fallback={
            <EqPanel
              label={props.target.label}
              color={props.target.color}
              initialEq={props.target.initialEq}
              onEqChange={handleEqChange}
            />
          }
        >
          <MixEqTabs
            label={props.target.label}
            color={props.target.color}
            onEqChange={handleEqChange}
          />
        </Show>
        <EffectsBlock sourceType={props.target.sourceType} color={props.target.color} />
      </div>
    </div>
  );
}

// ---------------------------------------------------------------------------
// Mix EQ: pre-fader / post-fader tab switcher with two independent EQ panels
// ---------------------------------------------------------------------------

const MIX_EQ_TABS = ["Pre-Fader", "Post-Fader"] as const;

function MixEqTabs(props: { label: string; color: string; onEqChange?: (eq: EqConfig) => void }) {
  const [activeTab, setActiveTab] = createSignal<(typeof MIX_EQ_TABS)[number]>("Post-Fader");

  return (
    <div>
      {/* Tab bar */}
      <div
        class="flex rounded-t-lg overflow-hidden border-b"
        style={{
          "background-color": "var(--color-bg-elevated)",
          "border-color": "var(--color-border)",
        }}
      >
        <For each={[...MIX_EQ_TABS]}>
          {(tab) => (
            <button
              class="flex-1 px-4 py-2 text-xs font-medium uppercase tracking-wide transition-colors duration-150"
              style={{
                "background-color": activeTab() === tab ? "var(--color-bg-primary)" : "transparent",
                color:
                  activeTab() === tab ? "var(--color-text-primary)" : "var(--color-text-muted)",
                "border-bottom":
                  activeTab() === tab ? `2px solid ${props.color}` : "2px solid transparent",
              }}
              onClick={() => setActiveTab(tab)}
            >
              {tab} EQ
            </button>
          )}
        </For>
      </div>

      {/* Active EQ panel */}
      <Show when={activeTab() === "Pre-Fader"}>
        <EqPanel
          label={`${props.label} — Pre-Fader`}
          color={props.color}
          onEqChange={props.onEqChange}
        />
      </Show>
      <Show when={activeTab() === "Post-Fader"}>
        <EqPanel
          label={`${props.label} — Post-Fader`}
          color={props.color}
          onEqChange={props.onEqChange}
        />
      </Show>
    </div>
  );
}
