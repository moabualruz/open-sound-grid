import { Show, For, createSignal } from "solid-js";
import type { JSX } from "solid-js";
import { useSession } from "../stores/sessionStore";
import { useGraph } from "../stores/graphStore";
import { Headphones, Radio, Film, MessageCircle, Speaker, X, ChevronDown } from "lucide-solid";
import type { EndpointDescriptor, Endpoint, PwDevice, PwNode } from "../types";

const PRESET_NAMES = ["Monitor", "Stream", "VOD", "Chat", "Aux"];

export interface OutputDevice {
  deviceId: string;
  deviceName: string;
  nodeName: string;
  /** PipeWire node name (ALSA identifier) — used for matching default sink */
  pwNodeName: string | null;
}

interface MixHeaderProps {
  descriptor: EndpointDescriptor;
  endpoint: Endpoint;
  color: string;
  outputDevice: string | null;
  usedDeviceIds: Set<string>;
  onRemove: () => void;
  onSelectOutput: (deviceId: string | null) => void;
  dragHandle?: () => JSX.Element;
}

function pickIcon(displayName: string, color: string): JSX.Element {
  const name = displayName.toLowerCase();
  const cls = "w-5 h-5 flex-shrink-0";

  if (name.includes("monitor") || name.includes("personal"))
    return <Headphones class={cls} style={{ color }} />;
  if (name.includes("chat")) return <MessageCircle class={cls} style={{ color }} />;
  if (name.includes("stream")) return <Radio class={cls} style={{ color }} />;
  if (name.includes("vod")) return <Film class={cls} style={{ color }} />;
  return <Speaker class={cls} style={{ color }} />;
}

function isPresetName(name: string): boolean {
  return PRESET_NAMES.some((p) => name === p);
}

export function getOutputDevices(
  devices: Record<string, PwDevice>,
  nodes: Record<string, PwNode>,
): OutputDevice[] {
  const result: OutputDevice[] = [];
  for (const [id, dev] of Object.entries(devices) as [string, PwDevice][]) {
    const devNodes = dev.nodes.map((nid) => nodes[String(nid)]).filter(Boolean) as PwNode[];
    for (const node of devNodes) {
      if (node.ports.some(([, kind]) => kind === "sink")) {
        result.push({
          deviceId: `${id}:${node.id}`,
          deviceName: dev.name,
          nodeName:
            node.identifier.nodeDescription ?? node.identifier.nodeName ?? `Node ${node.id}`,
          pwNodeName: node.identifier.nodeName,
        });
      }
    }
  }
  return result;
}

export default function MixHeader(props: MixHeaderProps): JSX.Element {
  const { send } = useSession();
  const graphState = useGraph();
  const [editing, setEditing] = createSignal(false);
  const [editValue, setEditValue] = createSignal("");
  const [showOutputPicker, setShowOutputPicker] = createSignal(false);
  const [dropdownPos, setDropdownPos] = createSignal({ top: 0, left: 0 });

  function openOutputPicker(e: MouseEvent) {
    const btn = e.currentTarget as HTMLElement;
    const rect = btn.getBoundingClientRect();
    setDropdownPos({ top: rect.bottom + 4, left: rect.left });
    setShowOutputPicker((v) => !v);
  }

  const label = () => props.endpoint.customName ?? props.endpoint.displayName;
  const isCustom = () => !isPresetName(props.endpoint.displayName);

  function startEdit() {
    if (!isCustom()) return;
    setEditValue(label());
    setEditing(true);
  }

  function commitEdit() {
    const val = editValue().trim();
    if (val && val !== label()) {
      send({ type: "renameEndpoint", endpoint: props.descriptor, name: val });
    }
    setEditing(false);
  }

  const allOutputDevices = () => getOutputDevices(graphState.graph.devices, graphState.graph.nodes);

  const availableDevices = () =>
    allOutputDevices().filter(
      (d) => d.deviceId === props.outputDevice || !props.usedDeviceIds.has(d.deviceId),
    );

  const outputLabel = () => {
    if (!props.outputDevice) return "No output";
    const dev = allOutputDevices().find((d) => d.deviceId === props.outputDevice);
    return dev?.nodeName ?? "No output";
  };

  return (
    <div class="relative flex min-w-[10rem] flex-1 flex-col rounded-t-lg bg-bg-elevated">
      <div class="h-[3px] w-full rounded-t-lg" style={{ "background-color": props.color }} />
      <div class="flex items-center gap-1.5 px-2 py-2">
        <Show when={props.dragHandle}>{(handle) => handle()()}</Show>
        {pickIcon(props.endpoint.displayName, props.color)}
        <div class="flex min-w-0 flex-1 flex-col">
          <Show
            when={editing()}
            fallback={
              <span
                class="truncate text-[13px] font-semibold"
                style={{ color: props.color }}
                onDblClick={startEdit}
                title={isCustom() ? "Double-click to rename" : undefined}
              >
                {label()}
              </span>
            }
          >
            <input
              type="text"
              value={editValue()}
              onInput={(e) => setEditValue(e.currentTarget.value)}
              onBlur={commitEdit}
              onKeyDown={(e) => {
                if (e.key === "Enter") commitEdit();
                if (e.key === "Escape") setEditing(false);
              }}
              autofocus
              class="w-full rounded border border-border-active bg-bg-primary px-1 text-[13px] font-semibold text-text-primary focus:outline-none"
            />
          </Show>
          <button
            onClick={openOutputPicker}
            class="flex items-center gap-0.5 text-[10px] text-text-muted transition-colors duration-150 hover:text-text-secondary"
            title="Select output device"
          >
            <span class="truncate">{outputLabel()}</span>
            <ChevronDown size={10} class="shrink-0" />
          </button>
        </div>
        <button
          type="button"
          onClick={() => props.onRemove()}
          class="ml-auto flex-shrink-0 text-text-muted transition-colors duration-150 hover:text-vu-hot"
          aria-label="Remove mix"
        >
          <X class="h-[14px] w-[14px]" />
        </button>
      </div>

      <Show when={showOutputPicker()}>
        <div class="fixed inset-0 z-40" onClick={() => setShowOutputPicker(false)} />
        <div
          class="fixed z-50 w-56 rounded-lg border border-border bg-bg-elevated shadow-xl"
          style={{ top: `${dropdownPos().top}px`, left: `${dropdownPos().left}px` }}
          onKeyDown={(e: KeyboardEvent) => e.key === "Escape" && setShowOutputPicker(false)}
        >
          <div class="p-2">
            <div class="px-2 pb-1 text-[10px] font-semibold uppercase tracking-widest text-text-muted">
              Output Device
            </div>
            <button
              onClick={() => {
                props.onSelectOutput(null);
                setShowOutputPicker(false);
              }}
              class={`flex w-full items-center gap-2 rounded-md px-2 py-1.5 text-left text-xs transition-colors duration-150 hover:bg-bg-hover ${
                !props.outputDevice ? "text-accent" : "text-text-secondary"
              }`}
            >
              None
            </button>
            <For each={availableDevices()}>
              {(dev) => (
                <button
                  onClick={() => {
                    props.onSelectOutput(dev.deviceId);
                    setShowOutputPicker(false);
                  }}
                  class={`flex w-full items-center gap-2 rounded-md px-2 py-1.5 text-left text-xs transition-colors duration-150 hover:bg-bg-hover ${
                    props.outputDevice === dev.deviceId ? "text-accent" : "text-text-secondary"
                  }`}
                >
                  <Speaker size={14} class="shrink-0 text-text-muted" />
                  <span class="flex-1 truncate">{dev.nodeName}</span>
                </button>
              )}
            </For>
          </div>
        </div>
      </Show>
    </div>
  );
}
