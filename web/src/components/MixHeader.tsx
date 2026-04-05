import { Show, For, createSignal, createEffect } from "solid-js";
import type { JSX } from "solid-js";
import { useSession } from "../stores/sessionStore";
import { useGraph } from "../stores/graphStore";
import { useVolumeDebounce } from "../hooks/useVolumeDebounce";
import MeterSlider from "./MeterSlider";
import { useSmoothedAggregatePeak } from "../hooks/useSmoothedPeak";
import {
  Headphones,
  Radio,
  Film,
  MessageCircle,
  Speaker,
  Volume2,
  VolumeX,
  X,
  ChevronDown,
  ChevronUp,
  SlidersVertical,
} from "lucide-solid";
import type { EndpointDescriptor, Endpoint } from "../types/session";
import type { PwDevice, PwNode } from "../types/graph";

const PRESET_NAMES = ["Monitor", "Stream", "VOD", "Chat", "Aux"];

export interface OutputDevice {
  /** Stable ALSA node name (e.g. "alsa_output.usb-..."). Falls back to "pw:<nodeId>" for non-ALSA nodes. */
  deviceId: string;
  deviceName: string;
  nodeName: string;
  /** PipeWire numeric node ID — used for backend SetMixOutput command. */
  pwNodeId: number;
}

interface MixHeaderProps {
  descriptor: EndpointDescriptor;
  endpoint: Endpoint;
  color: string;
  outputDevice: string | null;
  usedDeviceIds: Set<string>;
  onRemove: () => void;
  onSelectOutput: (deviceId: string | null) => void;
  onOpenEq?: () => void;
  dragHandle?: () => JSX.Element;
  /** Whether the effects row for this mix is currently expanded. */
  expanded?: boolean;
  /** Called when the user clicks the header to toggle expand. */
  onToggleExpand?: () => void;
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
  for (const [_id, dev] of Object.entries(devices) as [string, PwDevice][]) {
    const devNodes = dev.nodes.map((nid) => nodes[String(nid)]).filter(Boolean) as PwNode[];
    for (const node of devNodes) {
      const name = node.identifier.nodeName ?? "";
      // Skip virtual/internal sinks (EasyEffects, OSG channels)
      if (
        name.startsWith("easyeffects_") ||
        name.startsWith("ee_") ||
        name.startsWith("osg.group.")
      )
        continue;
      if (node.ports.some(([, kind]) => kind === "sink")) {
        result.push({
          deviceId: node.identifier.nodeName ?? `pw:${node.id}`,
          deviceName: dev.name,
          nodeName:
            node.identifier.nodeDescription ?? node.identifier.nodeName ?? `Node ${node.id}`,
          pwNodeId: node.id,
        });
      }
    }
  }
  return result;
}

export default function MixHeader(props: MixHeaderProps): JSX.Element {
  const { state, send } = useSession();
  const graphState = useGraph();

  const mixPeak = useSmoothedAggregatePeak(() => {
    if (!("channel" in props.descriptor)) return [];
    const chId = props.descriptor.channel;
    const ch = state.session.channels[chId];
    // Prefer the direct output node; fall back to cell sinks feeding this mix
    if (ch?.outputNodeId) return [ch.outputNodeId];
    return (state.session.links ?? [])
      .filter((link) => "channel" in link.end && link.end.channel === chId)
      .map((link) => link.cellNodeId);
  });

  const [editing, setEditing] = createSignal(false);
  const [editValue, setEditValue] = createSignal("");
  const [showOutputPicker, setShowOutputPicker] = createSignal(false);
  const [dropdownPos, setDropdownPos] = createSignal({ top: 0, left: 0 });
  const [activeDescendant, setActiveDescendant] = createSignal<string | undefined>(undefined);
  const [localVol, setLocalVol] = createSignal(1);
  let userDragging = false;
  let dropdownRef: HTMLDivElement | undefined;

  const sendVolDebounced = useVolumeDebounce((v) => {
    send({ type: "setVolume", endpoint: props.descriptor, volume: v });
    userDragging = false;
  });

  createEffect(() => {
    if (!userDragging) setLocalVol(props.endpoint.volume);
  });

  function handleVolumeInput(v: number) {
    userDragging = true;
    setLocalVol(v);
    sendVolDebounced(v);
  }

  const volPct = () => Math.round(localVol() * 100);

  function openOutputPicker(e: MouseEvent) {
    const btn = e.currentTarget as HTMLElement;
    const rect = btn.getBoundingClientRect();
    setDropdownPos({ top: rect.bottom + 4, left: rect.left });
    setShowOutputPicker((v) => !v);
    setActiveDescendant(undefined);
  }

  function closeOutputPicker() {
    setShowOutputPicker(false);
    setActiveDescendant(undefined);
  }

  function handleDropdownKeyDown(e: KeyboardEvent) {
    if (!showOutputPicker()) return;
    const devices = availableDevices();
    // Build option ids: "none" + device ids by index
    const optionIds = ["output-opt-none", ...devices.map((_, i) => `output-opt-${i}`)];
    const currentId = activeDescendant();
    const currentIndex = currentId ? optionIds.indexOf(currentId) : -1;

    if (e.key === "ArrowDown") {
      e.preventDefault();
      const next = currentIndex < optionIds.length - 1 ? currentIndex + 1 : 0;
      setActiveDescendant(optionIds[next]);
    } else if (e.key === "ArrowUp") {
      e.preventDefault();
      const prev = currentIndex > 0 ? currentIndex - 1 : optionIds.length - 1;
      setActiveDescendant(optionIds[prev]);
    } else if (e.key === "Enter" && currentId) {
      e.preventDefault();
      if (currentId === "output-opt-none") {
        props.onSelectOutput(null);
      } else {
        const idx = optionIds.indexOf(currentId) - 1; // subtract 1 for "none"
        const dev = devices[idx];
        if (dev) props.onSelectOutput(dev.deviceId);
      }
      closeOutputPicker();
    } else if (e.key === "Tab") {
      // Trap Tab within the dropdown
      e.preventDefault();
      if (e.shiftKey) {
        const prev = currentIndex > 0 ? currentIndex - 1 : optionIds.length - 1;
        setActiveDescendant(optionIds[prev]);
      } else {
        const next = currentIndex < optionIds.length - 1 ? currentIndex + 1 : 0;
        setActiveDescendant(optionIds[next]);
      }
    } else if (e.key === "Escape") {
      closeOutputPicker();
    }
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
    <div
      class="relative flex flex-col rounded-t-lg border-b border-border bg-bg-elevated"
      style={{ cursor: props.onToggleExpand ? "pointer" : "default" }}
      onClick={(e) => {
        // Only toggle when clicking the header background (not buttons/inputs inside)
        const target = e.target as HTMLElement;
        if (target.closest("button") || target.closest("input") || target.closest("select")) return;
        props.onToggleExpand?.();
      }}
      title={
        props.onToggleExpand
          ? props.expanded
            ? "Collapse effects view"
            : "Expand effects view"
          : undefined
      }
    >
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
          onClick={() => props.onOpenEq?.()}
          class="flex-shrink-0 text-text-muted/60 transition-colors duration-150 hover:text-accent"
          aria-label="EQ & Effects"
          title="EQ & Effects"
        >
          <SlidersVertical class="h-[12px] w-[12px]" />
        </button>
        <Show when={props.onToggleExpand}>
          <button
            type="button"
            onClick={(e) => {
              e.stopPropagation();
              props.onToggleExpand!();
            }}
            class="flex-shrink-0 text-text-muted/50 transition-colors duration-150 hover:text-text-secondary"
            aria-label={props.expanded ? "Collapse effects view" : "Expand effects view"}
            aria-expanded={props.expanded}
          >
            <Show when={props.expanded} fallback={<ChevronDown class="h-[12px] w-[12px]" />}>
              <ChevronUp class="h-[12px] w-[12px]" />
            </Show>
          </button>
        </Show>
        <button
          type="button"
          onClick={() => props.onRemove()}
          class="ml-auto flex-shrink-0 text-text-muted transition-colors duration-150 hover:text-vu-hot"
          aria-label="Remove mix"
        >
          <X class="h-[14px] w-[14px]" />
        </button>
      </div>

      {/* Mix master volume with VU meter + mute */}
      <div class="flex items-center gap-1.5 px-2 pb-2">
        <button
          type="button"
          onClick={() => {
            const muted = props.endpoint.volumeLockedMuted === "mutedUnlocked" || props.endpoint.volumeLockedMuted === "mutedLocked";
            send({ type: "setMute", endpoint: props.descriptor, muted: !muted });
          }}
          class={`flex h-6 w-6 shrink-0 items-center justify-center rounded transition-colors duration-150 ${
            props.endpoint.volumeLockedMuted === "mutedUnlocked" || props.endpoint.volumeLockedMuted === "mutedLocked"
              ? "text-vu-hot"
              : "text-text-muted hover:text-text-secondary"
          }`}
          aria-label={
            props.endpoint.volumeLockedMuted === "mutedUnlocked" || props.endpoint.volumeLockedMuted === "mutedLocked"
              ? "Unmute mix"
              : "Mute mix"
          }
          title={
            props.endpoint.volumeLockedMuted === "mutedUnlocked" || props.endpoint.volumeLockedMuted === "mutedLocked"
              ? "Unmute"
              : "Mute"
          }
        >
          <Show
            when={props.endpoint.volumeLockedMuted === "mutedUnlocked" || props.endpoint.volumeLockedMuted === "mutedLocked"}
            fallback={<Volume2 size={12} />}
          >
            <VolumeX size={12} />
          </Show>
        </button>
        <div class="flex-1">
          <MeterSlider
            value={localVol()}
            peakLeft={mixPeak.left()}
            peakRight={mixPeak.right()}
            onInput={handleVolumeInput}
            muted={props.endpoint.volumeLockedMuted === "mutedUnlocked" || props.endpoint.volumeLockedMuted === "mutedLocked"}
            label={`${label()} master volume`}
            valueText={`${volPct()}%`}
          />
        </div>
        <span class="w-7 shrink-0 text-right font-mono text-[10px] text-text-muted">
          {volPct()}
        </span>
      </div>

      <Show when={showOutputPicker()}>
        <div class="fixed inset-0 z-40" onClick={closeOutputPicker} />
        <div
          ref={dropdownRef}
          role="listbox"
          aria-label="Output Device"
          aria-activedescendant={activeDescendant()}
          tabIndex={-1}
          class="fixed z-50 w-max min-w-56 max-w-[calc(100vw-2rem)] rounded-lg border border-border bg-bg-elevated shadow-xl focus:outline-none"
          style={{ top: `${dropdownPos().top}px`, left: `${dropdownPos().left}px` }}
          onKeyDown={handleDropdownKeyDown}
        >
          <div class="p-2">
            <div class="px-2 pb-1 text-[10px] font-semibold uppercase tracking-widest text-text-muted">
              Output Device
            </div>
            <button
              id="output-opt-none"
              role="option"
              aria-selected={!props.outputDevice}
              onClick={() => {
                props.onSelectOutput(null);
                closeOutputPicker();
              }}
              class={`flex w-full items-center gap-2 rounded-md px-2 py-1.5 text-left text-xs transition-colors duration-150 hover:bg-bg-hover ${
                activeDescendant() === "output-opt-none" ? "bg-bg-hover" : ""
              } ${!props.outputDevice ? "text-accent" : "text-text-secondary"}`}
            >
              None
            </button>
            <For each={availableDevices()}>
              {(dev, i) => (
                <button
                  id={`output-opt-${i()}`}
                  role="option"
                  aria-selected={props.outputDevice === dev.deviceId}
                  onClick={() => {
                    props.onSelectOutput(dev.deviceId);
                    closeOutputPicker();
                  }}
                  class={`flex w-full items-center gap-2 rounded-md px-2 py-1.5 text-left text-xs transition-colors duration-150 hover:bg-bg-hover ${
                    activeDescendant() === `output-opt-${i()}` ? "bg-bg-hover" : ""
                  } ${props.outputDevice === dev.deviceId ? "text-accent" : "text-text-secondary"}`}
                >
                  <Speaker size={14} class="shrink-0 text-text-muted" />
                  <span class="flex-1">{dev.nodeName}</span>
                </button>
              )}
            </For>
          </div>
        </div>
      </Show>
    </div>
  );
}
