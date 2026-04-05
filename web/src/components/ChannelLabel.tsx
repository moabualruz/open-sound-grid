import { Show, createEffect, createMemo, createSignal } from "solid-js";
import type { JSX } from "solid-js";
import {
  Bell,
  EyeOff,
  Gamepad2,
  Globe,
  MessageCircle,
  Music,
  Power,
  Speaker,
  Volume2,
  VolumeX,
} from "lucide-solid";
import { useGraph } from "../stores/graphStore";
import { useMixerSettings } from "../stores/mixerSettings";
import { useSession } from "../stores/sessionStore";
import { useVolumeDebounce } from "../hooks/useVolumeDebounce";
import { useSmoothedAggregatePeak } from "../hooks/useSmoothedPeak";
import type { App, Channel, Endpoint, EndpointDescriptor, SourceType } from "../types/session";
import AppAssignment from "./AppAssignment";
import ContextMenu from "./ContextMenu";
import VuSlider from "./VuSlider";

const PRESET_CHANNEL_NAMES = ["Music", "Browser", "System", "Game", "SFX", "Voice Chat", "Aux 1"];

interface ChannelLabelProps {
  descriptor: EndpointDescriptor;
  endpoint: Endpoint;
  channel?: Channel;
  apps?: App[];
  dragHandle?: () => JSX.Element;
  onOpenEffects?: () => void;
}

function channelIcon(displayName: string) {
  switch (displayName) {
    case "Music":
      return <Music size={16} class="text-text-muted" />;
    case "Browser":
      return <Globe size={16} class="text-text-muted" />;
    case "System":
      return <Bell size={16} class="text-text-muted" />;
    case "Game":
      return <Gamepad2 size={16} class="text-text-muted" />;
    case "Voice Chat":
    case "Chat":
      return <MessageCircle size={16} class="text-text-muted" />;
    default:
      return <Speaker size={16} class="text-text-muted" />;
  }
}

function channelAccentColor(sourceType?: SourceType): string {
  switch (sourceType) {
    case "hardwareMic":
      return "var(--color-source-mic)";
    case "appStream":
      return "var(--color-source-app)";
    default:
      return "var(--color-source-cell)";
  }
}

export default function ChannelLabel(props: ChannelLabelProps) {
  const { state, send } = useSession();
  const graphState = useGraph();
  const { settings } = useMixerSettings();

  const channelPeakNodeIds = createMemo(() => {
    const assignedApps = props.channel?.assignedApps ?? [];
    if (assignedApps.length > 0) {
      const nodeIds = new Set<number>();
      for (const node of Object.values(graphState.graph.nodes)) {
        const isAppSource = node.ports.some(([, kind, isMonitor]) => kind === "source" && !isMonitor);
        if (!isAppSource) continue;
        const matchesAssignment = assignedApps.some(
          (assignment) =>
            node.identifier.applicationName === assignment.applicationName &&
            node.identifier.binaryName === assignment.binaryName,
        );
        if (matchesAssignment) nodeIds.add(node.id);
      }
      return [...nodeIds];
    }

    if (!("channel" in props.descriptor)) return [];
    const channelId = props.descriptor.channel;
    return (state.session.links ?? [])
      .filter((link) => "channel" in link.start && link.start.channel === channelId)
      .map((link) => link.cellNodeId)
      .filter((nodeId): nodeId is number => nodeId != null);
  });

  const channelPeak = useSmoothedAggregatePeak(() => channelPeakNodeIds());

  const [local, setLocal] = createSignal(0);
  const [localL, setLocalL] = createSignal(1);
  const [localR, setLocalR] = createSignal(1);
  const [editing, setEditing] = createSignal(false);
  const [editValue, setEditValue] = createSignal("");
  const [userDragging, setUserDragging] = createSignal(false);
  const [contextMenuPosition, setContextMenuPosition] = createSignal<{
    x: number;
    y: number;
  } | null>(null);

  const sendDebounced = useVolumeDebounce((value) => {
    send({ type: "setVolume", endpoint: props.descriptor, volume: value });
    setUserDragging(false);
  });

  const sendStereoDebounced = useVolumeDebounce(() => {
    send({
      type: "setStereoVolume",
      endpoint: props.descriptor,
      left: localL(),
      right: localR(),
    });
    setUserDragging(false);
  });

  const isStereo = () => settings.stereoMode === "stereo";

  createEffect(() => {
    if (userDragging()) return;
    setLocal(props.endpoint.volume);
    setLocalL(props.endpoint.volumeLeft);
    setLocalR(props.endpoint.volumeRight);
  });

  const displayName = () => props.endpoint.customName ?? props.endpoint.displayName;
  const isCustom = () => !PRESET_CHANNEL_NAMES.includes(props.endpoint.displayName);

  function startEdit(force = false) {
    if (!force && !isCustom()) return;
    setEditValue(displayName());
    setEditing(true);
  }

  function commitEdit() {
    const value = editValue().trim();
    if (value && value !== displayName()) {
      send({ type: "renameEndpoint", endpoint: props.descriptor, name: value });
    }
    setEditing(false);
  }

  const isMuted = () => {
    const state = props.endpoint.volumeLockedMuted;
    return state === "mutedLocked" || state === "mutedUnlocked" || state === "muteMixed";
  };

  function handleInput(value: number) {
    setUserDragging(true);
    setLocal(value);
    setLocalL(value);
    setLocalR(value);
    sendDebounced(value);
  }

  function handleStereoInput(channel: "left" | "right", value: number) {
    setUserDragging(true);
    if (channel === "left") setLocalL(value);
    else setLocalR(value);
    setLocal((localL() + localR()) / 2);
    sendStereoDebounced(value);
  }

  function handleWheel(event: WheelEvent) {
    event.preventDefault();
    const step = event.deltaY > 0 ? -0.01 : 0.01;
    handleInput(Math.max(0, Math.min(1, local() + step)));
  }

  function handleWheelStereo(channel: "left" | "right", event: WheelEvent) {
    event.preventDefault();
    const step = event.deltaY > 0 ? -0.01 : 0.01;
    const current = channel === "left" ? localL() : localR();
    handleStereoInput(channel, Math.max(0, Math.min(1, current + step)));
  }

  function openContextMenu(event: MouseEvent) {
    event.preventDefault();
    setContextMenuPosition({ x: event.clientX, y: event.clientY });
  }

  const pct = () => Math.round(local() * 100);
  const pctL = () => Math.round(localL() * 100);
  const pctR = () => Math.round(localR() * 100);

  return (
    <>
      <div
        class={`w-48 shrink-0 rounded-lg border border-border bg-bg-elevated transition-opacity duration-150 ${
          props.endpoint.disabled ? "opacity-50" : ""
        }`}
        onContextMenu={openContextMenu}
      >
        <div class="flex items-center gap-1.5 px-3 pt-2.5">
          <Show when={props.dragHandle}>{(handle) => handle()()}</Show>
          {channelIcon(props.endpoint.displayName)}

          <Show
            when={editing()}
            fallback={
              <span
                class="flex-1 truncate text-[13px] font-medium text-text-primary"
                onDblClick={() => startEdit()}
                title={isCustom() ? "Double-click to rename" : undefined}
              >
                {displayName()}
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
              class="flex-1 rounded border border-border-active bg-bg-primary px-1 text-[13px] font-medium text-text-primary focus:outline-none"
            />
          </Show>

          <button
            onClick={() => send({ type: "setMute", endpoint: props.descriptor, muted: !isMuted() })}
            class={`transition-colors duration-150 ${
              isMuted() ? "text-vu-hot" : "text-text-muted hover:text-text-primary"
            }`}
            title={isMuted() ? "Unmute" : "Mute"}
            aria-label={isMuted() ? "Unmute channel" : "Mute channel"}
          >
            <Show when={isMuted()} fallback={<Volume2 size={14} />}>
              <VolumeX size={14} />
            </Show>
          </button>

          <Show when={!props.channel?.autoApp && !("app" in props.descriptor)}>
            <button
              onClick={() =>
                send({
                  type: "setEndpointDisabled",
                  endpoint: props.descriptor,
                  disabled: !props.endpoint.disabled,
                })
              }
              class={`transition-colors duration-150 ${
                props.endpoint.disabled ? "text-vu-hot" : "text-text-muted hover:text-text-primary"
              }`}
              title={props.endpoint.disabled ? "Enable channel" : "Disable channel"}
              aria-label={props.endpoint.disabled ? "Enable channel" : "Disable channel"}
            >
              <Power size={12} />
            </button>
            <button
              onClick={() =>
                send({ type: "setEndpointVisible", endpoint: props.descriptor, visible: false })
              }
              class="text-text-muted transition-colors duration-150 hover:text-vu-hot"
              title="Hide channel"
              aria-label="Hide channel"
            >
              <EyeOff size={12} />
            </button>
          </Show>
        </div>

        <Show when={props.channel}>
          {(channel) => (
            <div class="px-3 py-1.5">
              <AppAssignment
                channelId={"channel" in props.descriptor ? props.descriptor.channel : ""}
                channel={channel()}
                apps={props.apps ?? []}
              />
            </div>
          )}
        </Show>

        <div class="px-3 pb-3 pt-2">
          <Show
            when={isStereo()}
            fallback={
              <div class="flex items-center gap-1">
                <div class="flex-1" onWheel={handleWheel}>
                  <VuSlider
                    value={local()}
                    peakLeft={channelPeak.left()}
                    peakRight={channelPeak.right()}
                    onInput={handleInput}
                    muted={isMuted()}
                    label="Master volume"
                    valueText={`${pct()}%`}
                    accentColor={channelAccentColor(props.channel?.sourceType)}
                  />
                </div>
                <span class="w-7 text-right font-mono text-[11px] text-text-secondary">
                  {pct()}
                </span>
              </div>
            }
          >
            <div class="flex flex-col gap-1.5">
              <div class="flex items-center gap-1">
                <span class="w-2 text-[9px] font-bold text-text-muted">L</span>
                <div class="flex-1" onWheel={(event) => handleWheelStereo("left", event)}>
                  <VuSlider
                    value={localL()}
                    peakLeft={channelPeak.left()}
                    peakRight={channelPeak.left()}
                    onInput={(value) => handleStereoInput("left", value)}
                    muted={isMuted()}
                    label="Left volume"
                    accentColor={channelAccentColor(props.channel?.sourceType)}
                  />
                </div>
                <span class="w-7 text-right font-mono text-[10px] text-text-secondary">
                  {pctL()}
                </span>
              </div>
              <div class="flex items-center gap-1">
                <span class="w-2 text-[9px] font-bold text-text-muted">R</span>
                <div class="flex-1" onWheel={(event) => handleWheelStereo("right", event)}>
                  <VuSlider
                    value={localR()}
                    peakLeft={channelPeak.right()}
                    peakRight={channelPeak.right()}
                    onInput={(value) => handleStereoInput("right", value)}
                    muted={isMuted()}
                    label="Right volume"
                    accentColor={channelAccentColor(props.channel?.sourceType)}
                  />
                </div>
                <span class="w-7 text-right font-mono text-[10px] text-text-secondary">
                  {pctR()}
                </span>
              </div>
            </div>
          </Show>
        </div>
      </div>

      <ContextMenu
        open={contextMenuPosition() !== null}
        position={contextMenuPosition()}
        onClose={() => setContextMenuPosition(null)}
        items={[
          { label: "Rename", onSelect: () => startEdit(true) },
          {
            label: "Effects",
            onSelect: () => props.onOpenEffects?.(),
            disabled: !props.onOpenEffects,
          },
          {
            label: "Hide",
            onSelect: () =>
              send({ type: "setEndpointVisible", endpoint: props.descriptor, visible: false }),
          },
          {
            label: props.endpoint.disabled ? "Enable" : "Disable",
            onSelect: () =>
              send({
                type: "setEndpointDisabled",
                endpoint: props.descriptor,
                disabled: !props.endpoint.disabled,
              }),
          },
          {
            label: "Remove",
            onSelect: () => send({ type: "removeEndpoint", endpoint: props.descriptor }),
            danger: true,
          },
        ]}
      />
    </>
  );
}
