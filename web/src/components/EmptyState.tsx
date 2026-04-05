import { Show } from "solid-js";
import type { JSX } from "solid-js";
import { SlidersVertical, WifiOff, Columns3 } from "lucide-solid";

interface EmptyStateProps {
  kind: "no-channels" | "disconnected" | "no-mixes";
}

export default function EmptyState(props: EmptyStateProps): JSX.Element {
  return (
    <div class="flex flex-1 items-center justify-center py-12">
      <div class="text-center max-w-xs">
        <Show when={props.kind === "no-channels"}>
          <SlidersVertical class="w-8 h-8 text-text-muted/40 mx-auto mb-3" />
          <p class="text-sm text-text-secondary mb-1">No channels yet</p>
          <p class="text-xs text-text-muted leading-relaxed">
            Create a channel to start routing audio. Use the + button above to create a channel.
          </p>
        </Show>
        <Show when={props.kind === "disconnected"}>
          <WifiOff class="w-8 h-8 text-vu-hot/40 mx-auto mb-3" />
          <p class="text-sm text-text-secondary mb-1">Disconnected from PipeWire</p>
          <p class="text-xs text-text-muted leading-relaxed">
            Waiting for connection to the audio server. Make sure osg-server is running.
          </p>
        </Show>
        <Show when={props.kind === "no-mixes"}>
          <Columns3 class="w-8 h-8 text-text-muted/40 mx-auto mb-3" />
          <p class="text-sm text-text-secondary mb-1">No output mixes</p>
          <p class="text-xs text-text-muted leading-relaxed">
            Add a mix to create output destinations for your audio channels.
          </p>
        </Show>
      </div>
    </div>
  );
}
