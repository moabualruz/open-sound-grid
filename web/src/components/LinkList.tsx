import { For, Show } from "solid-js";
import { useSession } from "../stores/sessionStore";
import type { EndpointDescriptor, MixerLink } from "../types";

function descLabel(d: EndpointDescriptor): string {
  if ("channel" in d) return d.channel.slice(0, 8);
  if ("app" in d) return `app:${d.app[0].slice(0, 8)}`;
  if ("ephemeralNode" in d) return `node:${d.ephemeralNode[0]}`;
  if ("persistentNode" in d) return `pnode:${d.persistentNode[0].slice(0, 8)}`;
  if ("device" in d) return `dev:${d.device[0].slice(0, 8)}`;
  return "?";
}

function endpointName(
  d: EndpointDescriptor,
  endpoints: [EndpointDescriptor, { displayName: string; customName: string | null }][],
): string {
  const match = endpoints.find(([desc]) => JSON.stringify(desc) === JSON.stringify(d));
  if (match) return match[1].customName ?? match[1].displayName;
  return descLabel(d);
}

export default function LinkList() {
  const { state, send } = useSession();

  const links = (): MixerLink[] => state.session.links;

  function removeLink(link: MixerLink) {
    send({ type: "removeLink", source: link.start, target: link.end });
  }

  return (
    <section>
      <h3 class="text-text-muted mb-2 text-sm font-medium uppercase tracking-wide">Routes</h3>
      <Show
        when={links().length > 0}
        fallback={<p class="text-text-muted text-sm">No routes configured.</p>}
      >
        <ul class="grid gap-1">
          <For each={links()}>
            {(link) => (
              <li class="flex items-center justify-between rounded border border-border bg-surface px-3 py-2 text-sm">
                <span>
                  <span class="text-source">
                    {endpointName(link.start, state.session.endpoints)}
                  </span>
                  <span class="text-text-muted mx-2">→</span>
                  <span class="text-sink">{endpointName(link.end, state.session.endpoints)}</span>
                </span>
                <div class="flex items-center gap-2">
                  <span class="text-text-muted text-xs">{link.state}</span>
                  <button
                    onClick={() => removeLink(link)}
                    class="text-text-muted text-xs hover:text-sink"
                  >
                    x
                  </button>
                </div>
              </li>
            )}
          </For>
        </ul>
      </Show>
    </section>
  );
}
