import { For, Show, createSignal } from "solid-js";
import { useSession } from "../stores/sessionStore";

export default function ChannelList() {
  const { state, send } = useSession();
  const [newName, setNewName] = createSignal("");

  function createChannel() {
    const name = newName().trim();
    if (name) {
      send({ type: "createChannel", name, kind: "duplex" });
      setNewName("");
    }
  }

  const channels = () => Object.values(state.session.channels);

  return (
    <section>
      <div class="mb-4 flex items-center gap-3">
        <h2 class="text-lg font-semibold">Channels</h2>
        <span
          class={`inline-block h-2 w-2 rounded-full ${state.connected ? "bg-source" : "bg-sink"}`}
          title={state.connected ? "Connected" : "Disconnected"}
        />
        <span class="text-text-muted text-xs">{channels().length} channels</span>
      </div>

      <div class="mb-4 flex gap-2">
        <input
          type="text"
          placeholder="Channel name..."
          value={newName()}
          onInput={(e) => setNewName(e.currentTarget.value)}
          onKeyDown={(e) => e.key === "Enter" && createChannel()}
          class="flex-1 rounded-lg border border-border bg-surface-alt px-3 py-2 text-sm text-text placeholder:text-text-muted focus:border-accent focus:outline-none"
        />
        <button
          onClick={createChannel}
          disabled={!newName().trim()}
          class="rounded-lg bg-accent px-4 py-2 text-sm font-medium text-surface disabled:opacity-40 hover:bg-accent-hover"
        >
          Add
        </button>
      </div>

      <Show
        when={channels().length > 0}
        fallback={<p class="text-text-muted text-sm">No channels yet. Create one above.</p>}
      >
        <ul class="grid gap-2">
          <For each={channels()}>
            {(channel) => (
              <li class="flex items-center justify-between rounded-lg border border-border bg-surface-alt px-4 py-3">
                <div class="flex items-center gap-2">
                  <span class="font-medium">{channel.id}</span>
                  <span class="text-text-muted text-xs">{channel.kind}</span>
                </div>
                <button
                  onClick={() =>
                    send({ type: "removeEndpoint", endpoint: { channel: channel.id } })
                  }
                  class="text-sink text-xs hover:text-text"
                >
                  Remove
                </button>
              </li>
            )}
          </For>
        </ul>
      </Show>
    </section>
  );
}
