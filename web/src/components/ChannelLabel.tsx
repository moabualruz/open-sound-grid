import { Show, createEffect, createSignal } from "solid-js";
import type { JSX } from "solid-js";
import { useSession } from "../stores/sessionStore";
import { useMixerSettings } from "../stores/mixerSettings";
import { useVolumeDebounce } from "../hooks/useVolumeDebounce";
import {
  Volume2,
  VolumeX,
  X,
  Music,
  Globe,
  Bell,
  Gamepad2,
  MessageCircle,
  Speaker,
} from "lucide-solid";
import AppAssignment from "./AppAssignment";
import MeterSlider from "./MeterSlider";
import { useSmoothedAggregatePeak } from "../hooks/useSmoothedPeak";
import type { EndpointDescriptor, Endpoint, Channel, App } from "../types/session";

const PRESET_CHANNEL_NAMES = ["Music", "Browser", "System", "Game", "SFX", "Voice Chat", "Aux 1"];

interface ChannelLabelProps {
  descriptor: EndpointDescriptor;
  endpoint: Endpoint;
  channel?: Channel;
  apps?: App[];
  dragHandle?: () => JSX.Element;
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

export default function ChannelLabel(props: ChannelLabelProps) {
  const { state, send } = useSession();
  const { settings } = useMixerSettings();

  const channelPeak = useSmoothedAggregatePeak(() => {
    if (!("channel" in props.descriptor)) return [];
    const chId = props.descriptor.channel;
    return (state.session.links ?? [])
      .filter((link) => "channel" in link.start && link.start.channel === chId)
      .map((link) => link.cellNodeId);
  });

  const [local, setLocal] = createSignal(0);
  const [localL, setLocalL] = createSignal(1);
  const [localR, setLocalR] = createSignal(1);
  const [editing, setEditing] = createSignal(false);
  const [editValue, setEditValue] = createSignal("");
  let userDragging = false;

  const sendDebounced = useVolumeDebounce((v) => {
    send({ type: "setVolume", endpoint: props.descriptor, volume: v });
    userDragging = false;
  });

  const sendStereoDebounced = useVolumeDebounce((_v) => {
    send({
      type: "setStereoVolume",
      endpoint: props.descriptor,
      left: localL(),
      right: localR(),
    });
    userDragging = false;
  });

  const isStereo = () => settings.stereoMode === "stereo";

  // Sync from backend — but not while the user is actively dragging the slider
  createEffect(() => {
    if (userDragging) return;
    setLocal(props.endpoint.volume);
    setLocalL(props.endpoint.volumeLeft);
    setLocalR(props.endpoint.volumeRight);
  });

  const displayName = () => props.endpoint.customName ?? props.endpoint.displayName;
  const isCustom = () => !PRESET_CHANNEL_NAMES.includes(props.endpoint.displayName);

  function startEdit() {
    if (!isCustom()) return;
    setEditValue(displayName());
    setEditing(true);
  }

  function commitEdit() {
    const val = editValue().trim();
    if (val && val !== displayName()) {
      send({ type: "renameEndpoint", endpoint: props.descriptor, name: val });
    }
    setEditing(false);
  }

  const isMuted = () => {
    const s = props.endpoint.volumeLockedMuted;
    return s === "mutedLocked" || s === "mutedUnlocked" || s === "muteMixed";
  };

  function handleInput(value: number) {
    userDragging = true;
    setLocal(value);
    setLocalL(value);
    setLocalR(value);
    sendDebounced(value);
  }

  function handleStereoInput(channel: "left" | "right", value: number) {
    userDragging = true;
    if (channel === "left") setLocalL(value);
    else setLocalR(value);
    setLocal((localL() + localR()) / 2);
    sendStereoDebounced(value);
  }

  function handleWheel(e: WheelEvent) {
    e.preventDefault();
    const step = e.deltaY > 0 ? -0.01 : 0.01;
    const next = Math.max(0, Math.min(1, local() + step));
    handleInput(next);
  }

  function handleWheelStereo(channel: "left" | "right", e: WheelEvent) {
    e.preventDefault();
    const step = e.deltaY > 0 ? -0.01 : 0.01;
    const current = channel === "left" ? localL() : localR();
    const next = Math.max(0, Math.min(1, current + step));
    handleStereoInput(channel, next);
  }

  const pct = () => Math.round(local() * 100);
  const pctL = () => Math.round(localL() * 100);
  const pctR = () => Math.round(localR() * 100);

  return (
    <div class="w-48 shrink-0 rounded-lg border border-border bg-bg-elevated">
      {/* Row 1: drag handle + icon + name + mute + remove */}
      <div class="flex items-center gap-1.5 px-3 pt-2.5">
        <Show when={props.dragHandle}>{(handle) => handle()()}</Show>
        {channelIcon(props.endpoint.displayName)}

        <Show
          when={editing()}
          fallback={
            <span
              class="flex-1 truncate text-[13px] font-medium text-text-primary"
              onDblClick={startEdit}
              title={isCustom() ? "Double-click to rename" : undefined}
            >
              {displayName()}
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
          {isMuted() ? <VolumeX size={14} /> : <Volume2 size={14} />}
        </button>

        <Show when={!props.channel?.autoApp && !("app" in props.descriptor)}>
          <button
            onClick={() =>
              send({ type: "setEndpointVisible", endpoint: props.descriptor, visible: false })
            }
            class="text-text-muted transition-colors duration-150 hover:text-vu-hot"
            title="Hide channel"
            aria-label="Hide channel"
          >
            <X size={12} />
          </button>
        </Show>
      </div>

      {/* Row 2: assigned apps — hidden for auto-created app channels */}
      <Show when={props.channel}>
        {(ch) => (
          <div class="px-3 py-1.5">
            <AppAssignment
              channelId={"channel" in props.descriptor ? props.descriptor.channel : ""}
              channel={ch()}
              apps={props.apps ?? []}
            />
          </div>
        )}
      </Show>

      {/* Row 3: master volume slider(s) */}
      <div class="px-3 pb-3 pt-2">
        <Show
          when={isStereo()}
          fallback={
            <div class="flex items-center gap-1">
              <div class="flex-1" onWheel={handleWheel}>
                <MeterSlider
                  value={local()}
                  peakLeft={channelPeak.left()}
                  peakRight={channelPeak.right()}
                  onInput={handleInput}
                  muted={isMuted()}
                  label="Master volume"
                  valueText={`${pct()}%`}
                />
              </div>
              <span class="w-7 text-right font-mono text-[11px] text-text-secondary">{pct()}</span>
            </div>
          }
        >
          <div class="flex flex-col gap-1.5">
            <div class="flex items-center gap-1">
              <span class="w-2 text-[9px] font-bold text-text-muted">L</span>
              <div class="flex-1" onWheel={(e) => handleWheelStereo("left", e)}>
                <MeterSlider
                  value={localL()}
                  peakLeft={channelPeak.left()}
                  peakRight={channelPeak.left()}
                  onInput={(v) => handleStereoInput("left", v)}
                  muted={isMuted()}
                  label="Left volume"
                />
              </div>
              <span class="w-7 text-right font-mono text-[10px] text-text-secondary">{pctL()}</span>
            </div>
            <div class="flex items-center gap-1">
              <span class="w-2 text-[9px] font-bold text-text-muted">R</span>
              <div class="flex-1" onWheel={(e) => handleWheelStereo("right", e)}>
                <MeterSlider
                  value={localR()}
                  peakLeft={channelPeak.right()}
                  peakRight={channelPeak.right()}
                  onInput={(v) => handleStereoInput("right", v)}
                  muted={isMuted()}
                  label="Right volume"
                />
              </div>
              <span class="w-7 text-right font-mono text-[10px] text-text-secondary">{pctR()}</span>
            </div>
          </div>
        </Show>
      </div>
    </div>
  );
}
