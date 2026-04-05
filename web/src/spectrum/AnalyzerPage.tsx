/**
 * Full-page standalone spectrum analyzer.
 * Node picker at top, full SpectrumAnalyzer with labels below.
 */
import { createSignal, For, Show } from "solid-js";
import { useSession } from "../stores/sessionStore";
import SpectrumAnalyzer from "./SpectrumAnalyzer";

import type { Endpoint } from "../types/session";
import type { EndpointDescriptor } from "../types/session";

/** Derive a display name from an endpoint's displayName / customName. */
function endpointLabel(ep: Endpoint): string {
  return ep.customName ?? ep.displayName;
}

/**
 * Serialize an EndpointDescriptor to a stable string key — matches
 * what the backend uses as nodeFilterKey in /ws/spectrum.
 *
 * The backend uses the endpoint descriptor JSON as the subscription key.
 */
function descToKey(desc: EndpointDescriptor): string {
  return JSON.stringify(desc);
}

export default function AnalyzerPage() {
  const { state } = useSession();

  // All endpoints with their descriptors
  const endpoints = () =>
    state.session.endpoints.map(([desc, ep]) => ({ desc, ep, key: descToKey(desc) }));

  const [selectedKey, setSelectedKey] = createSignal<string | null>(null);

  // Auto-select first endpoint when list loads
  const activeKey = () => {
    const key = selectedKey();
    if (key) return key;
    const eps = endpoints();
    return eps.length > 0 ? eps[0]!.key : null;
  };

  return (
    <div class="flex flex-col h-full" style={{ "background-color": "var(--color-bg-primary)" }}>
      {/* Header */}
      <div
        class="flex items-center gap-4 px-4 py-2.5 border-b"
        style={{
          "background-color": "var(--color-bg-secondary)",
          "border-color": "var(--color-border)",
        }}
      >
        <span class="text-sm font-semibold" style={{ color: "var(--color-text-primary)" }}>
          Spectrum Analyzer
        </span>

        {/* Node picker */}
        <Show
          when={endpoints().length > 0}
          fallback={
            <span class="text-xs" style={{ color: "var(--color-text-muted)" }}>
              No nodes available
            </span>
          }
        >
          <select
            class="rounded px-2 py-1 text-xs"
            style={{
              "background-color": "var(--color-bg-primary)",
              color: "var(--color-text-secondary)",
              border: "1px solid var(--color-border)",
            }}
            value={activeKey() ?? ""}
            onChange={(e: Event) => setSelectedKey((e.target as HTMLSelectElement).value)}
            aria-label="Select node"
          >
            <For each={endpoints()}>
              {(entry) => <option value={entry.key}>{endpointLabel(entry.ep)}</option>}
            </For>
          </select>
        </Show>
      </div>

      {/* Analyzer — fills remaining space */}
      <div class="flex-1 flex items-center justify-center p-4 overflow-hidden">
        <Show
          when={activeKey()}
          fallback={
            <span class="text-sm" style={{ color: "var(--color-text-muted)" }}>
              Connect a PipeWire node to see spectrum data.
            </span>
          }
        >
          {(key) => (
            <div class="w-full h-full">
              <SpectrumAnalyzer
                nodeKey={key()}
                showLabels={true}
                overlay={false}
                width={800}
                height={340}
              />
            </div>
          )}
        </Show>
      </div>
    </div>
  );
}
