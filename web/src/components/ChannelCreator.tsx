import { For, Show, createSignal } from "solid-js";
import { useSession } from "../stores/sessionStore";
import type { PwClient } from "../types";

const PRESETS = [
  { name: "Music", icon: "🎵", kind: "duplex" as const },
  { name: "Browser", icon: "🌐", kind: "duplex" as const },
  { name: "System", icon: "🔔", kind: "duplex" as const },
  { name: "Game", icon: "🎮", kind: "duplex" as const },
  { name: "Voice Chat", icon: "💬", kind: "duplex" as const },
  { name: "SFX", icon: "🔊", kind: "duplex" as const },
];

interface ChannelCreatorProps {
  apps: PwClient[];
}

export default function ChannelCreator(props: ChannelCreatorProps) {
  const { send } = useSession();
  const [open, setOpen] = createSignal(false);
  const [search, setSearch] = createSignal("");

  const filteredApps = () => {
    const q = search().toLowerCase();
    return props.apps.filter((a) => a.name.toLowerCase().includes(q));
  };

  const filteredPresets = () => {
    const q = search().toLowerCase();
    return PRESETS.filter((p) => p.name.toLowerCase().includes(q));
  };

  function create(name: string, kind: "source" | "duplex" | "sink") {
    send({ type: "createChannel", name, kind });
    setOpen(false);
    setSearch("");
  }

  return (
    <div class="relative">
      <button
        onClick={() => setOpen(!open())}
        class="flex w-full items-center justify-center gap-1.5 rounded-lg border border-dashed border-border bg-bg-secondary px-3 py-2.5 text-[13px] text-text-muted transition-colors hover:border-accent hover:text-accent"
      >
        <span class="text-base">+</span> Create channel
      </button>

      <Show when={open()}>
        {/* Backdrop */}
        <div class="fixed inset-0 z-20" onClick={() => setOpen(false)} />

        {/* Dropdown */}
        <div class="absolute bottom-full left-0 z-30 mb-1 w-64 rounded-lg border border-border bg-bg-elevated shadow-xl">
          {/* Search */}
          <div class="border-b border-border p-2">
            <input
              type="text"
              placeholder="Search apps..."
              value={search()}
              onInput={(e) => setSearch(e.currentTarget.value)}
              autofocus
              class="w-full rounded-md border border-border bg-bg-primary px-2.5 py-1.5 text-xs text-text-primary placeholder:text-text-muted focus:border-border-active focus:outline-none"
            />
          </div>

          <div class="max-h-64 overflow-y-auto">
            {/* Detected apps */}
            <Show when={filteredApps().length > 0}>
              <div class="px-2 pt-2">
                <div class="px-1 pb-1 text-[10px] font-semibold uppercase tracking-widest text-text-muted">
                  Running Apps
                </div>
                <For each={filteredApps()}>
                  {(app) => (
                    <button
                      onClick={() => create(app.name, "duplex")}
                      class="flex w-full items-center gap-2 rounded-md px-2 py-1.5 text-left text-xs text-text-secondary hover:bg-bg-hover hover:text-text-primary"
                    >
                      <span class="text-sm">📱</span>
                      <span class="flex-1 truncate">{app.name}</span>
                      <span class="rounded-full bg-vu-safe/20 px-1.5 py-0.5 text-[10px] text-vu-safe">
                        {app.nodes.length}
                      </span>
                    </button>
                  )}
                </For>
              </div>
            </Show>

            {/* Presets */}
            <div class="px-2 py-2">
              <div class="px-1 pb-1 text-[10px] font-semibold uppercase tracking-widest text-text-muted">
                Add Empty Channel
              </div>
              <For each={filteredPresets()}>
                {(preset) => (
                  <button
                    onClick={() => create(preset.name, preset.kind)}
                    class="flex w-full items-center gap-2 rounded-md px-2 py-1.5 text-left text-xs text-text-secondary hover:bg-bg-hover hover:text-text-primary"
                  >
                    <span class="text-sm">{preset.icon}</span>
                    <span>{preset.name}</span>
                  </button>
                )}
              </For>
            </div>
          </div>
        </div>
      </Show>
    </div>
  );
}
