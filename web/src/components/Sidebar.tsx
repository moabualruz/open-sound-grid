import { For, Show, createSignal } from "solid-js";
import { Dynamic } from "solid-js/web";
import type { JSX, Component } from "solid-js";
import { useSession } from "../stores/sessionStore";
import { useGraph } from "../stores/graphStore";
import {
  Mic,
  Headphones,
  Radio,
  Film,
  MessageCircle,
  Speaker,
  ChevronLeft,
  ChevronRight,
  Settings,
} from "lucide-solid";
import type { PwDevice, EndpointDescriptor, Endpoint } from "../types";

function findEndpoint(
  endpoints: [EndpointDescriptor, Endpoint][],
  desc: EndpointDescriptor,
): Endpoint | undefined {
  return endpoints.find(([d]) => JSON.stringify(d) === JSON.stringify(desc))?.[1];
}

type IconComponent = Component<{ size: number; class: string }>;

const MIX_ICONS: Record<string, IconComponent> = {
  Monitor: Headphones as IconComponent,
  Stream: Radio as IconComponent,
  VOD: Film as IconComponent,
  Chat: MessageCircle as IconComponent,
};

const MIX_COLORS: Record<string, string> = {
  Monitor: "var(--color-mix-monitor)",
  Stream: "var(--color-mix-stream)",
  VOD: "var(--color-mix-vod)",
  Chat: "var(--color-mix-chat)",
  Aux: "var(--color-mix-aux)",
};

function getMixColor(name: string): string {
  for (const key of Object.keys(MIX_COLORS)) {
    if (name.includes(key)) return MIX_COLORS[key];
  }
  return MIX_COLORS["Monitor"];
}

function getMixIcon(name: string): IconComponent {
  for (const key of Object.keys(MIX_ICONS)) {
    if (name.includes(key)) return MIX_ICONS[key];
  }
  return Speaker as IconComponent;
}

interface SidebarProps {
  onOpenSettings: () => void;
}

export default function Sidebar(props: SidebarProps): JSX.Element {
  const [collapsed, setCollapsed] = createSignal(false);
  const { state } = useSession();
  const graphState = useGraph();

  const hardwareDevices = () =>
    Object.values(graphState.graph.devices).filter((d: PwDevice) => d.nodes.length > 0);

  return (
    <aside
      class="flex flex-col shrink-0 bg-bg-secondary border-r border-border overflow-hidden transition-all duration-200"
      style={{
        width: collapsed() ? "48px" : "200px",
        "transition-timing-function": "var(--ease-out-quart)",
      }}
    >
      {/* Toggle */}
      <div class="flex items-center justify-end px-2 pt-3 pb-2">
        <button
          onClick={() => setCollapsed((v) => !v)}
          class="flex items-center justify-center w-7 h-7 rounded text-text-muted hover:text-text-primary hover:bg-bg-hover transition-colors duration-150"
          aria-label={collapsed() ? "Expand sidebar" : "Collapse sidebar"}
        >
          <Show when={collapsed()} fallback={<ChevronLeft size={16} class="" />}>
            <ChevronRight size={16} class="" />
          </Show>
        </button>
      </div>

      {/* Devices section */}
      <section class="flex flex-col min-h-0">
        <Show when={!collapsed()}>
          <div class="px-3 pb-1">
            <span class="text-[10px] font-semibold uppercase tracking-widest text-text-muted">
              Devices
            </span>
          </div>
        </Show>

        <div class="flex flex-col overflow-y-auto">
          <For each={hardwareDevices()}>
            {(device: PwDevice) => (
              <div class="flex flex-col px-3 py-2 gap-1 hover:bg-bg-hover transition-colors duration-100 cursor-default">
                <div class="flex items-center gap-2 min-w-0">
                  <Mic size={16} class="text-text-muted shrink-0" />
                  <Show when={!collapsed()}>
                    <span class="text-[13px] text-text-primary truncate leading-none">
                      {device.name}
                    </span>
                  </Show>
                </div>
                <Show when={!collapsed()}>
                  <div class="flex items-center gap-1.5 text-[10px] text-text-muted">
                    <span class="inline-block h-1.5 w-1.5 rounded-full bg-vu-safe/60" />
                    <span>
                      {device.nodes.length} {device.nodes.length === 1 ? "node" : "nodes"}
                    </span>
                  </div>
                </Show>
              </div>
            )}
          </For>
        </div>
      </section>

      {/* Mixes & Effects section */}
      <section class="flex flex-col min-h-0 border-t border-border mt-1">
        <Show when={!collapsed()}>
          <div class="px-3 pt-3 pb-1">
            <span class="text-[10px] font-semibold uppercase tracking-widest text-text-muted">
              Mixes &amp; Effects
            </span>
          </div>
        </Show>

        <div class="flex flex-col overflow-y-auto">
          <For each={state.session.activeSinks}>
            {(desc: EndpointDescriptor) => {
              const endpoint = () => findEndpoint(state.session.endpoints, desc);
              const name = () => endpoint()?.customName ?? endpoint()?.displayName ?? "";
              const color = () => getMixColor(name());
              const icon = () => getMixIcon(name());

              return (
                <div
                  class="flex items-center gap-2 px-3 py-2 min-w-0 hover:bg-bg-hover transition-colors duration-100 cursor-default border-l-2"
                  style={{ "border-color": color() }}
                >
                  <Dynamic component={icon()} size={16} class="text-text-muted shrink-0" />
                  <Show when={!collapsed()}>
                    <span class="text-[13px] text-text-primary truncate leading-none">
                      {name()}
                    </span>
                  </Show>
                </div>
              );
            }}
          </For>
        </div>
      </section>

      {/* Spacer */}
      <div class="flex-1" />

      {/* Settings */}
      <div class="border-t border-border">
        <button
          aria-label="Settings"
          onClick={() => props.onOpenSettings()}
          class="flex items-center gap-2 w-full px-3 py-3 text-text-muted hover:text-text-primary hover:bg-bg-hover transition-colors duration-100"
        >
          <Settings size={16} class="shrink-0" />
          <Show when={!collapsed()}>
            <span class="text-[13px] truncate">Settings</span>
          </Show>
        </button>
      </div>
    </aside>
  );
}
