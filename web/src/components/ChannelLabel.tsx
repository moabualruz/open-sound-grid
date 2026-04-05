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
import VuMeter from "./VuMeter";
import AppAssignment from "./AppAssignment";
import type { EndpointDescriptor, Endpoint, Channel, App } from "../types/session";

const PRESET_CHANNEL_NAMES = ["Music", "Browser", "System", "Game", "SFX", "Voice Chat", "Aux 1"];

interface ChannelLabelProps {
  descriptor: EndpointDescriptor;
  endpoint: Endpoint;
  channel?: Channel;
  apps?: App[];
  dragHandle?: () => JSX.Element;
  peakLeft?: number;
  peakRight?: number;
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
  const { send } = useSession();
  const { settings } = useMixerSettings();
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

      {/* VU meter — channel peak level */}
      <div class="px-3 pt-2">
        <VuMeter peakLeft={props.peakLeft} peakRight={props.peakRight} />
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
              <div class="relative flex-1" onWheel={handleWheel}>
                {/* Peak level (behind slider) */}
                <div
                  class="pointer-events-none absolute top-1/2 left-0 h-2.5 -translate-y-1/2 rounded-full transition-all duration-75"
                  style={{
                    width: `${Math.round((props.peakLeft ?? 0) * 100)}%`,
                    background: "var(--color-vu-gradient)",
                    "background-size": `${(props.peakLeft ?? 0) > 0 ? Math.round(100 / (props.peakLeft ?? 0.01)) : 100}% 100%`,
                    opacity: 0.25,
                  }}
                />
                <input
                  type="range"
                  min="0"
                  max="1"
                  step="0.01"
                  value={local()}
                  aria-label="Master volume"
                  aria-valuetext={`${pct()}%`}
                  onInput={(e) => handleInput(parseFloat(e.currentTarget.value))}
                  class="relative z-10 w-full"
                  style={{ "--value-pct": `${pct()}%` }}
                />
              </div>
              <span class="w-7 text-right font-mono text-[11px] text-text-secondary">{pct()}</span>
            </div>
          }
        >
          <div class="flex flex-col gap-1.5">
            <div class="flex items-center gap-1">
              <span class="w-2 text-[9px] font-bold text-text-muted">L</span>
              <div class="relative flex-1" onWheel={(e) => handleWheelStereo("left", e)}>
                <div
                  class="pointer-events-none absolute top-1/2 left-0 h-2.5 -translate-y-1/2 rounded-full transition-all duration-75"
                  style={{
                    width: `${Math.round((props.peakLeft ?? 0) * 100)}%`,
                    background: "var(--color-vu-gradient)",
                    "background-size": `${(props.peakLeft ?? 0) > 0 ? Math.round(100 / (props.peakLeft ?? 0.01)) : 100}% 100%`,
                    opacity: 0.25,
                  }}
                />
                <input
                  type="range"
                  min="0"
                  max="1"
                  step="0.01"
                  value={localL()}
                  aria-label="Left volume"
                  onInput={(e) => handleStereoInput("left", parseFloat(e.currentTarget.value))}
                  class="relative z-10 w-full"
                  style={{ "--value-pct": `${pctL()}%` }}
                />
              </div>
              <span class="w-7 text-right font-mono text-[10px] text-text-secondary">{pctL()}</span>
            </div>
            <div class="flex items-center gap-1">
              <span class="w-2 text-[9px] font-bold text-text-muted">R</span>
              <div class="relative flex-1" onWheel={(e) => handleWheelStereo("right", e)}>
                <div
                  class="pointer-events-none absolute top-1/2 left-0 h-2.5 -translate-y-1/2 rounded-full transition-all duration-75"
                  style={{
                    width: `${Math.round((props.peakRight ?? 0) * 100)}%`,
                    background: "var(--color-vu-gradient)",
                    "background-size": `${(props.peakRight ?? 0) > 0 ? Math.round(100 / (props.peakRight ?? 0.01)) : 100}% 100%`,
                    opacity: 0.25,
                  }}
                />
                <input
                  type="range"
                  min="0"
                  max="1"
                  step="0.01"
                  value={localR()}
                  aria-label="Right volume"
                  onInput={(e) => handleStereoInput("right", parseFloat(e.currentTarget.value))}
                  class="relative z-10 w-full"
                  style={{ "--value-pct": `${pctR()}%` }}
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
