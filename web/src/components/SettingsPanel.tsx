import { Show, createSignal } from "solid-js";
import type { JSX } from "solid-js";
import { useSession } from "../stores/sessionStore";
import { useGraph } from "../stores/graphStore";
import { X, Monitor, Sun, Moon, SlidersVertical } from "lucide-solid";

interface SettingsPanelProps {
  open: boolean;
  onClose: () => void;
}

export default function SettingsPanel(props: SettingsPanelProps): JSX.Element {
  const { state } = useSession();
  const graphState = useGraph();
  const [presetName, setPresetName] = createSignal("");
  // TODO(backend): persist stereo mode to settings.toml via command
  const [stereoMode, setStereoMode] = createSignal<"mono" | "stereo">("mono");
  // TODO(backend): persist latency setting via command
  const [latency, setLatency] = createSignal("0");

  const channelCount = () => state.session.endpoints.filter(([desc]) => "channel" in desc).length;

  return (
    <Show when={props.open}>
      <div class="fixed inset-0 z-40 flex items-center justify-center">
        <div class="absolute inset-0 bg-black/50" onClick={() => props.onClose()} />
        <div
          class="relative z-50 w-full max-w-md rounded-lg border border-border bg-bg-elevated shadow-2xl"
          onKeyDown={(e: KeyboardEvent) => e.key === "Escape" && props.onClose()}
        >
          <div class="flex items-center justify-between border-b border-border px-5 py-3">
            <h2 class="text-sm font-semibold text-text-primary">Settings</h2>
            <button
              onClick={() => props.onClose()}
              aria-label="Close settings"
              class="text-text-muted transition-colors duration-150 hover:text-text-primary"
            >
              <X size={16} />
            </button>
          </div>

          <div class="max-h-[70vh] space-y-5 overflow-y-auto px-5 py-4">
            {/* Connection */}
            <section>
              <h3 class="mb-2 text-[11px] font-semibold uppercase tracking-widest text-text-muted">
                Connection
              </h3>
              <div class="space-y-1.5 text-xs text-text-secondary">
                <div class="flex justify-between">
                  <span>PipeWire</span>
                  <span class="flex items-center gap-1.5">
                    <span
                      class={`inline-block h-1.5 w-1.5 rounded-full ${graphState.connected ? "bg-vu-safe" : "bg-vu-hot"}`}
                    />
                    {graphState.connected ? "Connected" : "Disconnected"}
                  </span>
                </div>
                <div class="flex justify-between">
                  <span>Channels</span>
                  <span>{channelCount()}</span>
                </div>
                <div class="flex justify-between">
                  <span>Nodes</span>
                  <span>{Object.keys(graphState.graph.nodes).length}</span>
                </div>
                <div class="flex justify-between">
                  <span>Devices</span>
                  <span>{Object.keys(graphState.graph.devices).length}</span>
                </div>
              </div>
            </section>

            {/* Mixer */}
            <section>
              <h3 class="mb-2 text-[11px] font-semibold uppercase tracking-widest text-text-muted">
                Mixer
              </h3>
              <div class="space-y-3">
                {/* Stereo / Mono toggle */}
                <div>
                  <div class="mb-1.5 text-xs text-text-secondary">Volume sliders</div>
                  <div class="flex gap-2">
                    <button
                      onClick={() => setStereoMode("mono")}
                      class={`flex flex-1 items-center justify-center gap-2 rounded-md border px-3 py-2 text-xs transition-colors duration-150 ${
                        stereoMode() === "mono"
                          ? "border-border-active bg-bg-hover text-text-primary"
                          : "border-border text-text-muted hover:text-text-secondary"
                      }`}
                    >
                      <SlidersVertical size={14} />
                      Single (Mono)
                    </button>
                    <button
                      onClick={() => setStereoMode("stereo")}
                      class={`flex flex-1 items-center justify-center gap-2 rounded-md border px-3 py-2 text-xs transition-colors duration-150 ${
                        stereoMode() === "stereo"
                          ? "border-border-active bg-bg-hover text-text-primary"
                          : "border-border text-text-muted hover:text-text-secondary"
                      }`}
                    >
                      <SlidersVertical size={14} />
                      L/R (Stereo)
                    </button>
                  </div>
                  <p class="mt-1 text-[10px] text-text-muted">
                    {stereoMode() === "stereo"
                      ? "Independent left/right volume control per channel"
                      : "Single volume slider controls both channels"}
                  </p>
                </div>

                {/* Latency */}
                <div>
                  <div class="mb-1.5 text-xs text-text-secondary">Latency (ms)</div>
                  <div class="flex items-center gap-2">
                    <input
                      type="number"
                      min="0"
                      max="500"
                      step="1"
                      value={latency()}
                      onInput={(e) => setLatency(e.currentTarget.value)}
                      class="w-20 rounded-md border border-border bg-bg-primary px-2.5 py-1.5 text-xs text-text-primary focus:border-border-active focus:outline-none"
                    />
                    <span class="text-[10px] text-text-muted">
                      PipeWire quantum controls actual latency
                    </span>
                  </div>
                </div>
              </div>
            </section>

            {/* Appearance */}
            <section>
              <h3 class="mb-2 text-[11px] font-semibold uppercase tracking-widest text-text-muted">
                Appearance
              </h3>
              <div class="flex gap-2">
                <button class="flex flex-1 items-center justify-center gap-2 rounded-md border border-border-active bg-bg-hover px-3 py-2 text-xs text-text-primary">
                  <Moon size={14} />
                  Dark
                </button>
                <button
                  disabled
                  class="flex flex-1 items-center justify-center gap-2 rounded-md border border-border px-3 py-2 text-xs text-text-muted opacity-50"
                >
                  <Sun size={14} />
                  Light
                </button>
                <button
                  disabled
                  class="flex flex-1 items-center justify-center gap-2 rounded-md border border-border px-3 py-2 text-xs text-text-muted opacity-50"
                >
                  <Monitor size={14} />
                  System
                </button>
              </div>
            </section>

            {/* Presets */}
            <section>
              <h3 class="mb-2 text-[11px] font-semibold uppercase tracking-widest text-text-muted">
                Presets
              </h3>
              <div class="flex gap-2">
                <input
                  type="text"
                  placeholder="Preset name..."
                  value={presetName()}
                  onInput={(e) => setPresetName(e.currentTarget.value)}
                  class="flex-1 rounded-md border border-border bg-bg-primary px-2.5 py-1.5 text-xs text-text-primary placeholder:text-text-muted focus:border-border-active focus:outline-none"
                />
                <button
                  disabled
                  class="rounded-md border border-border px-3 py-1.5 text-xs text-text-muted opacity-50"
                >
                  Save
                </button>
              </div>
              <p class="mt-1.5 text-[10px] text-text-muted">Preset save/load coming soon</p>
            </section>

            {/* Paths */}
            <section>
              <h3 class="mb-2 text-[11px] font-semibold uppercase tracking-widest text-text-muted">
                Paths
              </h3>
              <div class="space-y-1 text-[11px]">
                <div class="flex justify-between text-text-secondary">
                  <span>Config</span>
                  <span class="font-mono text-text-muted">~/.config/open-sound-grid/</span>
                </div>
                <div class="flex justify-between text-text-secondary">
                  <span>State</span>
                  <span class="font-mono text-text-muted">~/.local/share/open-sound-grid/</span>
                </div>
              </div>
            </section>

            {/* Backend TODOs */}
            <section>
              <h3 class="mb-2 text-[11px] font-semibold uppercase tracking-widest text-text-muted">
                Not Yet Implemented
              </h3>
              <div class="space-y-1 text-[10px] text-text-muted">
                <p>Real-time VU meters (needs /ws/levels backend endpoint)</p>
                <p>Stereo L/R independent control (needs backend stereo commands)</p>
                <p>Per-mix output device selection</p>
                <p>Preset save/load (needs backend persistence)</p>
                <p>Light/System theme</p>
                <p>EasyEffects mic name resolution</p>
                <p>Keyboard navigation (arrow keys, shortcuts)</p>
              </div>
            </section>
          </div>

          <div class="border-t border-border px-5 py-3 text-center text-[10px] text-text-muted">
            Open Sound Grid v0.1.0
          </div>
        </div>
      </div>
    </Show>
  );
}
