import { Show, createEffect, createSignal, onCleanup } from "solid-js";
import type { JSX } from "solid-js";
import { useSession } from "../stores/sessionStore";
import { useMixerSettings } from "../stores/mixerSettings";
import {
  Volume2,
  VolumeX,
  X,
  SlidersVertical,
  Music,
  Globe,
  Bell,
  Gamepad2,
  MessageCircle,
  Speaker,
} from "lucide-solid";
import type { EndpointDescriptor, Endpoint } from "../types";

const PRESET_CHANNEL_NAMES = ["Music", "Browser", "System", "Game", "SFX", "Voice Chat", "Aux 1"];

interface ChannelLabelProps {
  descriptor: EndpointDescriptor;
  endpoint: Endpoint;
  dragHandle?: () => JSX.Element;
}

const DEBOUNCE_MS = 16;

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
  let debounceTimer: ReturnType<typeof setTimeout> | null = null;
  let userDragging = false;

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
    if (debounceTimer) clearTimeout(debounceTimer);
    debounceTimer = setTimeout(() => {
      send({ type: "setVolume", endpoint: props.descriptor, volume: value });
      userDragging = false;
    }, DEBOUNCE_MS);
  }

  function handleStereoInput(channel: "left" | "right", value: number) {
    userDragging = true;
    if (channel === "left") setLocalL(value);
    else setLocalR(value);
    setLocal((localL() + localR()) / 2);
    if (debounceTimer) clearTimeout(debounceTimer);
    debounceTimer = setTimeout(() => {
      send({
        type: "setStereoVolume",
        endpoint: props.descriptor,
        left: localL(),
        right: localR(),
      });
      userDragging = false;
    }, DEBOUNCE_MS);
  }

  onCleanup(() => {
    if (debounceTimer) clearTimeout(debounceTimer);
  });

  const pct = () => Math.round(local() * 100);
  const pctL = () => Math.round(localL() * 100);
  const pctR = () => Math.round(localR() * 100);

  return (
    <div class="w-48 shrink-0 rounded-lg border border-border bg-bg-elevated">
      {/* Row 1: drag handle + icon + name + mute + remove */}
      <div class="flex items-center gap-1.5 px-2 pt-2">
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

        <button
          class="text-text-muted/40 transition-colors duration-150 hover:text-text-muted"
          title="Effects (coming soon)"
          aria-label="Effects"
          disabled
        >
          <SlidersVertical size={12} />
        </button>

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
      </div>

      {/* Row 2: master volume slider(s) */}
      <div class="px-3 pb-2 pt-1">
        <Show
          when={isStereo()}
          fallback={
            <div class="flex items-center gap-1">
              <div class="relative flex-1">
                <div
                  class="pointer-events-none absolute top-1/2 left-0 h-1.5 -translate-y-1/2 rounded-full"
                  style={{
                    width: `${pct()}%`,
                    background: isMuted() ? "var(--color-text-muted)" : "var(--color-accent)",
                    opacity: isMuted() ? 0.1 : 0.2,
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
                />
              </div>
              <span class="w-7 text-right font-mono text-[11px] text-text-secondary">{pct()}</span>
            </div>
          }
        >
          <div class="flex flex-col gap-1.5">
            <div class="flex items-center gap-1">
              <span class="w-2 text-[9px] font-bold text-text-muted">L</span>
              <input
                type="range"
                min="0"
                max="1"
                step="0.01"
                value={localL()}
                aria-label="Left volume"
                onInput={(e) => handleStereoInput("left", parseFloat(e.currentTarget.value))}
                class="w-full"
              />
              <span class="w-7 text-right font-mono text-[10px] text-text-secondary">{pctL()}</span>
            </div>
            <div class="flex items-center gap-1">
              <span class="w-2 text-[9px] font-bold text-text-muted">R</span>
              <input
                type="range"
                min="0"
                max="1"
                step="0.01"
                value={localR()}
                aria-label="Right volume"
                onInput={(e) => handleStereoInput("right", parseFloat(e.currentTarget.value))}
                class="w-full"
              />
              <span class="w-7 text-right font-mono text-[10px] text-text-secondary">{pctR()}</span>
            </div>
          </div>
        </Show>
      </div>
    </div>
  );
}
