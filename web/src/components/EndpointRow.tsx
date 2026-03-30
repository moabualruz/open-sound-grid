import { Show } from "solid-js";
import type { Endpoint, EndpointDescriptor } from "../types";
import { useSession } from "../stores/sessionStore";
import VolumeSlider from "./VolumeSlider";

interface EndpointRowProps {
  endpoint: Endpoint;
}

function getMuteLabel(endpoint: Endpoint): string {
  const state = endpoint.volumeLockedMuted;
  if (state === "mutedLocked" || state === "mutedUnlocked") return "Unmute";
  return "Mute";
}

function isMuted(endpoint: Endpoint): boolean {
  return (
    endpoint.volumeLockedMuted === "mutedLocked" ||
    endpoint.volumeLockedMuted === "mutedUnlocked" ||
    endpoint.volumeLockedMuted === "muteMixed"
  );
}

export default function EndpointRow(props: EndpointRowProps) {
  const { send } = useSession();
  const ep = () => props.endpoint;
  const desc = (): EndpointDescriptor => ep().descriptor;

  return (
    <li class="flex items-center gap-3 rounded-lg border border-border bg-surface-alt px-4 py-3">
      <div class="w-40 shrink-0">
        <span class="font-medium">{ep().customName ?? ep().displayName}</span>
        <Show when={ep().isPlaceholder}>
          <span class="text-text-muted ml-1 text-xs">(pending)</span>
        </Show>
      </div>

      <div class="flex-1">
        <VolumeSlider
          value={ep().volume}
          muted={isMuted(ep())}
          onChange={(v) => send({ type: "setVolume", endpoint: desc(), volume: v })}
        />
      </div>

      <button
        onClick={() => send({ type: "setMute", endpoint: desc(), muted: !isMuted(ep()) })}
        class={`rounded px-2 py-1 text-xs ${isMuted(ep()) ? "bg-sink/20 text-sink" : "bg-surface-hover text-text-muted"}`}
      >
        {getMuteLabel(ep())}
      </button>

      <button
        onClick={() => send({ type: "removeEndpoint", endpoint: desc() })}
        class="text-text-muted text-xs hover:text-sink"
      >
        x
      </button>
    </li>
  );
}
