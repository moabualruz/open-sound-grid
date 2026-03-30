import { For, Show } from "solid-js";
import { useSession } from "../stores/sessionStore";
import EndpointRow from "./EndpointRow";

export default function MixerPanel() {
  const { state } = useSession();

  const endpoints = () => state.session.endpoints.map(([, ep]) => ep);
  const sources = () =>
    endpoints().filter((ep) => {
      const d = ep.descriptor;
      return (
        "app" in d || "channel" in d || ("persistentNode" in d && d.persistentNode[1] === "source")
      );
    });
  const sinks = () =>
    endpoints().filter((ep) => {
      const d = ep.descriptor;
      return (
        ("persistentNode" in d && d.persistentNode[1] === "sink") ||
        ("ephemeralNode" in d && d.ephemeralNode[1] === "sink")
      );
    });

  return (
    <section class="col-span-full">
      <h2 class="mb-4 text-lg font-semibold">Mixer</h2>

      <Show
        when={endpoints().length > 0}
        fallback={
          <p class="text-text-muted text-sm">
            No endpoints in session. Create a channel to get started.
          </p>
        }
      >
        <div class="grid gap-6 lg:grid-cols-2">
          <div>
            <h3 class="text-text-muted mb-2 text-sm font-medium uppercase tracking-wide">
              Sources
            </h3>
            <ul class="grid gap-2">
              <For each={sources()}>{(ep) => <EndpointRow endpoint={ep} />}</For>
            </ul>
            <Show when={sources().length === 0}>
              <p class="text-text-muted text-sm">No sources</p>
            </Show>
          </div>

          <div>
            <h3 class="text-text-muted mb-2 text-sm font-medium uppercase tracking-wide">
              Outputs
            </h3>
            <ul class="grid gap-2">
              <For each={sinks()}>{(ep) => <EndpointRow endpoint={ep} />}</For>
            </ul>
            <Show when={sinks().length === 0}>
              <p class="text-text-muted text-sm">No outputs</p>
            </Show>
          </div>
        </div>
      </Show>
    </section>
  );
}
