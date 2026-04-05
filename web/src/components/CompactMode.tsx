import { For, Show, createEffect, createMemo, createSignal, onCleanup } from "solid-js";
import type { JSX } from "solid-js";
import { ChevronDown, Plus, SlidersVertical, Volume2, VolumeX } from "lucide-solid";
import { useMixerSettings } from "../stores/mixerSettings";
import { useSession } from "../stores/sessionStore";
import { useVolumeDebounce } from "../hooks/useVolumeDebounce";
import { useSmoothedPeak } from "../hooks/useSmoothedPeak";
import type { Channel, Endpoint, EndpointDescriptor, MixerLink } from "../types/session";
import type { EndpointEntry } from "../hooks/useMixerViewModel";
import { findLink, getMixColor } from "./mixerUtils";
import MixHeader from "./MixHeader";
import VuSlider from "./VuSlider";

interface CompactModeProps {
  channels: EndpointEntry[];
  channelsById: Record<string, Channel>;
  links: MixerLink[];
  mixes: EndpointEntry[];
  selectedMixKey: string | null;
  onSelectMixKey: (mixKey: string) => void;
  descKey: (descriptor: EndpointDescriptor) => string;
  mixOutputs: Record<string, string | null>;
  usedDeviceIds: Set<string>;
  onSelectOutput: (mixKey: string, deviceId: string | null) => void;
  onOpenCellEq: (source: EndpointDescriptor, target: EndpointDescriptor) => void;
  onOpenChannelEffects: (endpoint: Endpoint, descriptor: EndpointDescriptor) => void;
  onOpenMixEq: (endpoint: Endpoint, descriptor: EndpointDescriptor) => void;
  onRemoveMix: (descriptor: EndpointDescriptor) => void;
}

interface CompactChannelRowProps {
  source: EndpointEntry;
  sink: EndpointEntry;
  channel?: Channel;
  link: MixerLink | null;
  mixColor: string;
  onOpenEq: () => void;
  onOpenEffects: () => void;
}

function compactSourceType(channel?: Channel): "mic" | "app" {
  return channel?.sourceType === "hardwareMic" ? "mic" : "app";
}

function CompactChannelRow(props: CompactChannelRowProps): JSX.Element {
  const { send } = useSession();
  const { settings } = useMixerSettings();
  const peak = useSmoothedPeak(() => props.link?.cellNodeId);
  const [cellVol, setCellVol] = createSignal(1);
  const [cellL, setCellL] = createSignal(1);
  const [cellR, setCellR] = createSignal(1);
  const [cellMuted, setCellMuted] = createSignal(false);
  const [userDragging, setUserDragging] = createSignal(false);
  let preMuteVol: { vol: number; left: number; right: number } | null = null;

  const sendDebounced = useVolumeDebounce((value) => {
    send({
      type: "setLinkVolume",
      source: props.source.desc,
      target: props.sink.desc,
      volume: value,
    });
    setUserDragging(false);
  });

  const sendStereoDebounced = useVolumeDebounce(() => {
    send({
      type: "setLinkStereoVolume",
      source: props.source.desc,
      target: props.sink.desc,
      left: cellL(),
      right: cellR(),
    });
    setUserDragging(false);
  });

  createEffect(() => {
    if (userDragging()) return;
    setCellVol(props.link?.cellVolume ?? 1);
    setCellL(props.link?.cellVolumeLeft ?? 1);
    setCellR(props.link?.cellVolumeRight ?? 1);
  });

  const isStereo = () => settings.stereoMode === "stereo";
  const isLinked = () => props.link !== null;
  const displayName = () => props.source.ep.customName ?? props.source.ep.displayName;
  const channelMuted = () => {
    const state = props.source.ep.volumeLockedMuted;
    return state === "mutedLocked" || state === "mutedUnlocked" || state === "muteMixed";
  };
  const isMuted = () => !isLinked() || channelMuted() || cellMuted();
  const effectivePct = () => Math.round(cellVol() * props.source.ep.volume * 100);
  const effectivePctL = () => Math.round(cellL() * props.source.ep.volumeLeft * 100);
  const effectivePctR = () => Math.round(cellR() * props.source.ep.volumeRight * 100);

  function ensureLinked() {
    if (isLinked()) return;
    send({ type: "link", source: props.source.desc, target: props.sink.desc });
  }

  function handleInput(value: number) {
    ensureLinked();
    setUserDragging(true);
    setCellVol(value);
    setCellL(value);
    setCellR(value);
    sendDebounced(value);
  }

  function handleStereoInput(channel: "left" | "right", value: number) {
    ensureLinked();
    setUserDragging(true);
    if (channel === "left") setCellL(value);
    else setCellR(value);
    setCellVol((cellL() + cellR()) / 2);
    sendStereoDebounced(value);
  }

  function toggleCellMute() {
    if (!isLinked()) {
      ensureLinked();
      return;
    }

    if (cellMuted()) {
      const restored = preMuteVol ?? { vol: 1, left: 1, right: 1 };
      preMuteVol = null;
      setCellMuted(false);
      setCellVol(restored.vol);
      setCellL(restored.left);
      setCellR(restored.right);
      send({
        type: "setLinkStereoVolume",
        source: props.source.desc,
        target: props.sink.desc,
        left: restored.left,
        right: restored.right,
      });
      return;
    }

    preMuteVol = { vol: cellVol(), left: cellL(), right: cellR() };
    setCellMuted(true);
    setCellVol(0);
    setCellL(0);
    setCellR(0);
    send({
      type: "setLinkVolume",
      source: props.source.desc,
      target: props.sink.desc,
      volume: 0,
    });
  }

  return (
    <div class="rounded-lg border border-border bg-bg-elevated px-3 py-2">
      <div class="mb-1.5 flex items-center gap-2">
        <button
          type="button"
          onClick={toggleCellMute}
          class={`shrink-0 transition-colors duration-150 ${
            isMuted() ? "text-vu-hot" : "text-text-muted hover:text-text-primary"
          }`}
          aria-label={
            !isLinked()
              ? `Create route for ${displayName()}`
              : isMuted()
                ? `Unmute ${displayName()}`
                : `Mute ${displayName()}`
          }
          title={!isLinked() ? "Create route" : isMuted() ? "Unmute route" : "Mute route"}
        >
          <Show
            when={!isLinked()}
            fallback={
              <Show when={isMuted()} fallback={<Volume2 size={14} />}>
                <VolumeX size={14} />
              </Show>
            }
          >
            <Plus size={14} />
          </Show>
        </button>

        <div class="min-w-0 flex-1">
          <div class="truncate text-[12px] font-medium text-text-primary">{displayName()}</div>
          <div class="text-[10px] uppercase tracking-wide text-text-muted">
            {compactSourceType(props.channel)}
          </div>
        </div>

        <button
          type="button"
          onClick={() => props.onOpenEffects()}
          class="shrink-0 text-text-muted transition-colors duration-150 hover:text-accent"
          title="Channel effects"
          aria-label={`Open effects for ${displayName()}`}
        >
          <SlidersVertical size={12} />
        </button>
      </div>

      <Show
        when={isStereo()}
        fallback={
          <div class="flex items-center gap-2">
            <div class="flex-1">
              <VuSlider
                value={cellVol()}
                peakLeft={peak.left()}
                peakRight={peak.right()}
                onInput={handleInput}
                muted={isMuted()}
                label={`${displayName()} level`}
                valueText={`${Math.round(cellVol() * 100)}% (effective ${effectivePct()}%)`}
                accentColor={props.mixColor}
              />
            </div>
            <span class="w-10 text-right font-mono text-[10px] text-text-secondary">
              {Math.round(cellVol() * 100)}
            </span>
          </div>
        }
      >
        <div class="flex flex-col gap-1.5">
          <div class="flex items-center gap-2">
            <span class="w-2 text-[8px] font-bold text-text-muted">L</span>
            <div class="flex-1">
              <VuSlider
                value={cellL()}
                peakLeft={peak.left()}
                peakRight={peak.left()}
                onInput={(value) => handleStereoInput("left", value)}
                muted={isMuted()}
                label={`${displayName()} left level`}
                valueText={`${Math.round(cellL() * 100)}% (effective ${effectivePctL()}%)`}
                accentColor={props.mixColor}
              />
            </div>
            <span class="w-10 text-right font-mono text-[10px] text-text-secondary">
              {Math.round(cellL() * 100)}
            </span>
          </div>
          <div class="flex items-center gap-2">
            <span class="w-2 text-[8px] font-bold text-text-muted">R</span>
            <div class="flex-1">
              <VuSlider
                value={cellR()}
                peakLeft={peak.right()}
                peakRight={peak.right()}
                onInput={(value) => handleStereoInput("right", value)}
                muted={isMuted()}
                label={`${displayName()} right level`}
                valueText={`${Math.round(cellR() * 100)}% (effective ${effectivePctR()}%)`}
                accentColor={props.mixColor}
              />
            </div>
            <span class="w-10 text-right font-mono text-[10px] text-text-secondary">
              {Math.round(cellR() * 100)}
            </span>
          </div>
        </div>
      </Show>

      <Show when={isLinked()}>
        <div class="mt-1.5 flex justify-end">
          <button
            type="button"
            onClick={() => props.onOpenEq()}
            class="text-[10px] text-text-muted transition-colors duration-150 hover:text-accent"
          >
            Route EQ & Effects
          </button>
        </div>
      </Show>
    </div>
  );
}

export default function CompactMode(props: CompactModeProps): JSX.Element {
  const [dropdownOpen, setDropdownOpen] = createSignal(false);
  let dropdownRef: HTMLDivElement | undefined;

  const selectedMix = createMemo(() => {
    if (props.mixes.length === 0) return null;
    return props.mixes.find((mix) => props.descKey(mix.desc) === props.selectedMixKey) ?? props.mixes[0];
  });

  const selectedMixLabel = () => {
    const mix = selectedMix();
    return mix ? (mix.ep.customName ?? mix.ep.displayName) : "Select mix";
  };

  function handleSelect(mixKey: string) {
    props.onSelectMixKey(mixKey);
    setDropdownOpen(false);
  }

  function handleKeyDown(event: KeyboardEvent) {
    if (event.key === "Escape") {
      event.preventDefault();
      setDropdownOpen(false);
    }
  }

  function handleClickOutside(event: MouseEvent) {
    if (dropdownRef && !dropdownRef.contains(event.target as Node)) {
      setDropdownOpen(false);
    }
  }

  createEffect(() => {
    if (dropdownOpen()) {
      document.addEventListener("mousedown", handleClickOutside);
      document.addEventListener("keydown", handleKeyDown);
    } else {
      document.removeEventListener("mousedown", handleClickOutside);
      document.removeEventListener("keydown", handleKeyDown);
    }
  });

  onCleanup(() => {
    document.removeEventListener("mousedown", handleClickOutside);
    document.removeEventListener("keydown", handleKeyDown);
  });

  return (
    <div class="mx-auto flex h-full w-full max-w-[400px] flex-col gap-3" data-testid="compact-mode">
      <div class="rounded-xl border border-border bg-bg-elevated/70 p-3 shadow-sm">
        <div class="mb-2 text-[10px] font-semibold uppercase tracking-[0.18em] text-text-muted">
          Compact Mixer
        </div>
        <div class="flex flex-col gap-1 text-[11px] text-text-secondary">
          Active mix
          <div ref={dropdownRef} class="relative">
            <button
              type="button"
              onClick={() => setDropdownOpen((prev) => !prev)}
              class="flex w-full items-center justify-between rounded-lg border border-border bg-bg-elevated px-3 py-2 text-sm text-text-primary transition-colors duration-150 hover:border-border-active focus:border-border-active focus:outline-none"
              aria-haspopup="listbox"
              aria-expanded={dropdownOpen()}
            >
              <span class="truncate">{selectedMixLabel()}</span>
              <ChevronDown size={14} class={`shrink-0 text-text-muted transition-transform duration-150 ${dropdownOpen() ? "rotate-180" : ""}`} />
            </button>
            <Show when={dropdownOpen()}>
              <div
                role="listbox"
                class="absolute z-50 mt-1 w-full rounded-lg border border-border bg-bg-elevated shadow-xl"
              >
                <For each={props.mixes}>
                  {(mix) => {
                    const mixKey = props.descKey(mix.desc);
                    const isSelected = () => mixKey === props.selectedMixKey;
                    return (
                      <button
                        type="button"
                        role="option"
                        aria-selected={isSelected()}
                        onClick={() => handleSelect(mixKey)}
                        class={`flex w-full items-center px-3 py-2 text-left text-sm transition-colors duration-150 first:rounded-t-lg last:rounded-b-lg hover:bg-bg-hover ${
                          isSelected() ? "text-accent" : "text-text-primary"
                        }`}
                      >
                        {mix.ep.customName ?? mix.ep.displayName}
                      </button>
                    );
                  }}
                </For>
              </div>
            </Show>
          </div>
        </div>
      </div>

      <Show
        when={selectedMix()}
        fallback={
          <div class="rounded-xl border border-dashed border-border bg-bg-elevated/40 px-4 py-8 text-center text-sm text-text-muted">
            Create a mix to use compact mode.
          </div>
        }
      >
        {(mix) => {
          const mixKey = () => props.descKey(mix().desc);
          const mixColor = () => getMixColor(mix().ep.displayName);

          return (
            <>
              <MixHeader
                descriptor={mix().desc}
                endpoint={mix().ep}
                color={mixColor()}
                outputDevice={props.mixOutputs[mixKey()] ?? null}
                usedDeviceIds={props.usedDeviceIds}
                onRemove={() => props.onRemoveMix(mix().desc)}
                onSelectOutput={(deviceId) => props.onSelectOutput(mixKey(), deviceId)}
                onOpenEq={() => props.onOpenMixEq(mix().ep, mix().desc)}
              />

              <div class="flex flex-col gap-2 overflow-y-auto pr-1">
                <For each={props.channels}>
                  {(channel) => (
                    <CompactChannelRow
                      source={channel}
                      sink={mix()}
                      channel={
                        "channel" in channel.desc ? props.channelsById[channel.desc.channel] : undefined
                      }
                      link={findLink(props.links, channel.desc, mix().desc)}
                      mixColor={mixColor()}
                      onOpenEq={() => props.onOpenCellEq(channel.desc, mix().desc)}
                      onOpenEffects={() => props.onOpenChannelEffects(channel.ep, channel.desc)}
                    />
                  )}
                </For>

                <Show when={props.channels.length === 0}>
                  <div class="rounded-lg border border-dashed border-border bg-bg-elevated/40 px-4 py-8 text-center text-sm text-text-muted">
                    Create a channel to control this mix in compact mode.
                  </div>
                </Show>
              </div>
            </>
          );
        }}
      </Show>
    </div>
  );
}
