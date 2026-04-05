import { For, Show, createEffect, createMemo, createSignal } from "solid-js";
import type { JSX } from "solid-js";
import {
  ChevronDown,
  ChevronUp,
  Film,
  Headphones,
  MessageCircle,
  Radio,
  SlidersVertical,
  Speaker,
  Volume2,
  VolumeX,
  X,
} from "lucide-solid";
import { useGraph } from "../stores/graphStore";
import { useSession } from "../stores/sessionStore";
import { useVolumeDebounce } from "../hooks/useVolumeDebounce";
import { useSmoothedPeak } from "../hooks/useSmoothedPeak";
import type { PwDevice, PwNode } from "../types/graph";
import type { Endpoint, EndpointDescriptor } from "../types/session";
import ContextMenu from "./ContextMenu";
import VuSlider from "./VuSlider";

const PRESET_NAMES = ["Monitor", "Stream", "VOD", "Chat", "Aux", "Music", "Game", "Browser", "System"];

export interface OutputDevice {
  deviceId: string;
  deviceName: string;
  nodeName: string;
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
  expanded?: boolean;
  onToggleExpand?: () => void;
}

function pickIcon(displayName: string, color: string): JSX.Element {
  const name = displayName.toLowerCase();
  const className = "h-5 w-5 shrink-0";

  if (name.includes("monitor") || name.includes("personal")) {
    return <Headphones class={className} style={{ color }} />;
  }
  if (name.includes("chat")) return <MessageCircle class={className} style={{ color }} />;
  if (name.includes("stream")) return <Radio class={className} style={{ color }} />;
  if (name.includes("vod")) return <Film class={className} style={{ color }} />;
  return <Speaker class={className} style={{ color }} />;
}

function isPresetName(name: string): boolean {
  return PRESET_NAMES.some((preset) => name === preset);
}

export function getOutputDevices(
  devices: Record<string, PwDevice>,
  nodes: Record<string, PwNode>,
): OutputDevice[] {
  const result: OutputDevice[] = [];

  for (const device of Object.values(devices) as PwDevice[]) {
    const deviceNodes = device.nodes
      .map((nodeId) => nodes[String(nodeId)])
      .filter(Boolean) as PwNode[];

    for (const node of deviceNodes) {
      const nodeName = node.identifier.nodeName ?? "";
      if (
        nodeName.startsWith("easyeffects_") ||
        nodeName.startsWith("ee_") ||
        nodeName.startsWith("osg.group.")
      ) {
        continue;
      }

      if (node.ports.some(([, kind]) => kind === "sink")) {
        result.push({
          deviceId: node.identifier.nodeName ?? `pw:${node.id}`,
          deviceName: device.name,
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
  const { send } = useSession();
  const graphState = useGraph();

  const mixGroupNodeId = createMemo(() =>
    "channel" in props.descriptor
      ? (graphState.graph.groupNodes?.[props.descriptor.channel]?.id ?? null)
      : null,
  );
  const mixPeak = useSmoothedPeak(() => mixGroupNodeId());

  const [editing, setEditing] = createSignal(false);
  const [editValue, setEditValue] = createSignal("");
  const [showOutputPicker, setShowOutputPicker] = createSignal(false);
  const [dropdownPos, setDropdownPos] = createSignal({ top: 0, left: 0 });
  const [activeDescendant, setActiveDescendant] = createSignal<string | undefined>(undefined);
  const [localVol, setLocalVol] = createSignal(1);
  const [userDragging, setUserDragging] = createSignal(false);
  const [contextMenuPosition, setContextMenuPosition] = createSignal<{
    x: number;
    y: number;
  } | null>(null);
  let dropdownRef: HTMLDivElement | undefined;

  const sendVolDebounced = useVolumeDebounce((value) => {
    send({ type: "setVolume", endpoint: props.descriptor, volume: value });
    setUserDragging(false);
  });

  createEffect(() => {
    if (!userDragging()) setLocalVol(props.endpoint.volume);
  });

  function handleVolumeInput(value: number) {
    setUserDragging(true);
    setLocalVol(value);
    sendVolDebounced(value);
  }

  function openOutputPickerAt(top: number, left: number) {
    setDropdownPos({ top, left });
    setShowOutputPicker(true);
    setActiveDescendant(undefined);
  }

  function openOutputPicker(event: MouseEvent) {
    const button = event.currentTarget as HTMLElement;
    const rect = button.getBoundingClientRect();
    if (showOutputPicker()) {
      closeOutputPicker();
      return;
    }
    openOutputPickerAt(rect.bottom + 4, rect.left);
  }

  function closeOutputPicker() {
    setShowOutputPicker(false);
    setActiveDescendant(undefined);
  }

  function handleDropdownKeyDown(event: KeyboardEvent) {
    if (!showOutputPicker()) return;

    const devices = availableDevices();
    const optionIds = ["output-opt-none", ...devices.map((_, index) => `output-opt-${index}`)];
    const currentId = activeDescendant();
    const currentIndex = currentId ? optionIds.indexOf(currentId) : -1;

    if (event.key === "ArrowDown") {
      event.preventDefault();
      const nextIndex = currentIndex < optionIds.length - 1 ? currentIndex + 1 : 0;
      setActiveDescendant(optionIds[nextIndex]);
      return;
    }

    if (event.key === "ArrowUp") {
      event.preventDefault();
      const prevIndex = currentIndex > 0 ? currentIndex - 1 : optionIds.length - 1;
      setActiveDescendant(optionIds[prevIndex]);
      return;
    }

    if (event.key === "Enter" && currentId) {
      event.preventDefault();
      if (currentId === "output-opt-none") {
        props.onSelectOutput(null);
      } else {
        const deviceIndex = optionIds.indexOf(currentId) - 1;
        const device = devices[deviceIndex];
        if (device) props.onSelectOutput(device.deviceId);
      }
      closeOutputPicker();
      return;
    }

    if (event.key === "Tab") {
      event.preventDefault();
      if (event.shiftKey) {
        const prevIndex = currentIndex > 0 ? currentIndex - 1 : optionIds.length - 1;
        setActiveDescendant(optionIds[prevIndex]);
      } else {
        const nextIndex = currentIndex < optionIds.length - 1 ? currentIndex + 1 : 0;
        setActiveDescendant(optionIds[nextIndex]);
      }
      return;
    }

    if (event.key === "Escape") closeOutputPicker();
  }

  const isMuted = () =>
    props.endpoint.volumeLockedMuted === "mutedUnlocked" ||
    props.endpoint.volumeLockedMuted === "mutedLocked";
  const volPct = () => Math.round(localVol() * 100);
  const label = () => props.endpoint.customName ?? props.endpoint.displayName;
  const isCustom = () => !isPresetName(props.endpoint.displayName);

  function startEdit() {
    if (!isCustom()) return;
    setEditValue(label());
    setEditing(true);
  }

  function commitEdit() {
    const value = editValue().trim();
    if (value && value !== label()) {
      send({ type: "renameEndpoint", endpoint: props.descriptor, name: value });
    }
    setEditing(false);
  }

  const allOutputDevices = () => getOutputDevices(graphState.graph.devices, graphState.graph.nodes);
  const availableDevices = () =>
    allOutputDevices().filter(
      (device) => device.deviceId === props.outputDevice || !props.usedDeviceIds.has(device.deviceId),
    );
  const outputLabel = () => {
    if (!props.outputDevice) return "No output";
    const device = allOutputDevices().find((candidate) => candidate.deviceId === props.outputDevice);
    return device?.nodeName ?? "No output";
  };

  function openContextMenu(event: MouseEvent) {
    event.preventDefault();
    setContextMenuPosition({ x: event.clientX, y: event.clientY });
  }

  return (
    <>
      <div
        class="relative flex flex-col rounded-t-lg border-b border-border bg-bg-elevated"
        style={{ cursor: props.onToggleExpand ? "pointer" : "default" }}
        onClick={(event) => {
          const target = event.target as HTMLElement;
          if (target.closest("button") || target.closest("input") || target.closest("select")) return;
          props.onToggleExpand?.();
        }}
        onContextMenu={openContextMenu}
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
                onInput={(event) => setEditValue(event.currentTarget.value)}
                onBlur={commitEdit}
                onKeyDown={(event) => {
                  if (event.key === "Enter") commitEdit();
                  if (event.key === "Escape") setEditing(false);
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
            class="shrink-0 text-text-muted/60 transition-colors duration-150 hover:text-accent"
            aria-label="EQ & Effects"
            title="EQ & Effects"
          >
            <SlidersVertical class="h-[12px] w-[12px]" />
          </button>

          <Show when={props.onToggleExpand}>
            <button
              type="button"
              onClick={(event) => {
                event.stopPropagation();
                props.onToggleExpand?.();
              }}
              class="shrink-0 text-text-muted/50 transition-colors duration-150 hover:text-text-secondary"
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
            class="ml-auto shrink-0 text-text-muted transition-colors duration-150 hover:text-vu-hot"
            aria-label="Remove mix"
          >
            <X class="h-[14px] w-[14px]" />
          </button>
        </div>

        <div class="flex items-center gap-1.5 px-2 pb-2">
          <button
            type="button"
            onClick={() => send({ type: "setMute", endpoint: props.descriptor, muted: !isMuted() })}
            class={`flex h-6 w-6 shrink-0 items-center justify-center rounded transition-colors duration-150 ${
              isMuted() ? "text-vu-hot" : "text-text-muted hover:text-text-secondary"
            }`}
            aria-label={isMuted() ? "Unmute mix" : "Mute mix"}
            title={isMuted() ? "Unmute" : "Mute"}
          >
            <Show when={isMuted()} fallback={<Volume2 size={12} />}>
              <VolumeX size={12} />
            </Show>
          </button>

          <div class="flex-1">
            <VuSlider
              value={localVol()}
              peakLeft={mixPeak.left()}
              peakRight={mixPeak.right()}
              onInput={handleVolumeInput}
              muted={isMuted()}
              label={`${label()} master volume`}
              valueText={`${volPct()}%`}
              accentColor={props.color}
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
                {(device, index) => (
                  <button
                    id={`output-opt-${index()}`}
                    role="option"
                    aria-selected={props.outputDevice === device.deviceId}
                    onClick={() => {
                      props.onSelectOutput(device.deviceId);
                      closeOutputPicker();
                    }}
                    class={`flex w-full items-center gap-2 rounded-md px-2 py-1.5 text-left text-xs transition-colors duration-150 hover:bg-bg-hover ${
                      activeDescendant() === `output-opt-${index()}` ? "bg-bg-hover" : ""
                    } ${props.outputDevice === device.deviceId ? "text-accent" : "text-text-secondary"}`}
                  >
                    <Speaker size={14} class="shrink-0 text-text-muted" />
                    <span class="flex-1">{device.nodeName}</span>
                  </button>
                )}
              </For>
            </div>
          </div>
        </Show>
      </div>

      <ContextMenu
        open={contextMenuPosition() !== null}
        position={contextMenuPosition()}
        onClose={() => setContextMenuPosition(null)}
        items={[
          { label: "Rename", onSelect: startEdit, disabled: isPresetName(props.endpoint.displayName) },
          {
            label: "Change Output",
            onSelect: () => {
              const position = contextMenuPosition();
              openOutputPickerAt(position?.y ?? 0, position?.x ?? 0);
            },
          },
          { label: "Remove", onSelect: () => props.onRemove(), danger: true },
        ]}
      />
    </>
  );
}
