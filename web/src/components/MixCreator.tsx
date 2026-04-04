import { For, Show, createSignal } from "solid-js";
import type { JSX } from "solid-js";
import { useSession } from "../stores/sessionStore";
import { Plus, Headphones, Radio, Film, MessageCircle, Speaker, PenLine } from "lucide-solid";
import type { Component } from "solid-js";
import { findEndpoint } from "./mixerUtils";

type IconComp = Component<{ size: number; class?: string }>;

const MIX_TEMPLATES: { name: string; icon: IconComp }[] = [
  { name: "Monitor", icon: Headphones as IconComp },
  { name: "Stream", icon: Radio as IconComp },
  { name: "VOD", icon: Film as IconComp },
  { name: "Chat", icon: MessageCircle as IconComp },
  { name: "Aux", icon: Speaker as IconComp },
];

interface MixCreatorProps {
  maxMixes: number;
  currentCount: number;
}

export default function MixCreator(props: MixCreatorProps): JSX.Element {
  const { state, send } = useSession();
  const [open, setOpen] = createSignal(false);
  const [customName, setCustomName] = createSignal("");

  const existingMixNames = () => {
    const names = new Set<string>();
    for (const desc of state.session.activeSinks) {
      const ep = findEndpoint(state.session.endpoints, desc);
      if (ep) names.add(ep.displayName);
    }
    // Also check sink channels
    for (const [, ep] of state.session.endpoints) {
      const ch = Object.values(state.session.channels).find(
        (c) => c.kind === "sink" && ep.displayName,
      );
      if (ch) names.add(ep.displayName);
    }
    return names;
  };

  const availableTemplates = () => {
    const existing = existingMixNames();
    return MIX_TEMPLATES.filter((t) => !existing.has(t.name));
  };

  const atMax = () => props.currentCount >= props.maxMixes;

  function create(name: string) {
    if (atMax()) return;
    send({ type: "createChannel", name, kind: "sink" });
    setOpen(false);
    setCustomName("");
  }

  function close() {
    setOpen(false);
    setCustomName("");
  }

  const [dropdownPos, setDropdownPos] = createSignal({ top: 0, left: 0 });

  function toggleOpen(e: MouseEvent) {
    const btn = e.currentTarget as HTMLElement;
    const rect = btn.getBoundingClientRect();
    setDropdownPos({ top: rect.bottom + 4, left: rect.left });
    setOpen((v) => !v);
  }

  return (
    <div>
      <button
        onClick={toggleOpen}
        aria-expanded={open()}
        aria-haspopup="listbox"
        class="flex h-full w-16 flex-col items-center justify-center gap-1 rounded-lg border border-dashed border-border bg-bg-elevated/30 text-text-muted transition-colors duration-150 hover:border-accent hover:text-accent"
        title="Add mix"
      >
        <Plus size={20} />
        <span class="text-[10px]">Add mix</span>
      </button>

      <Show when={open()}>
        <div class="fixed inset-0 z-40" onClick={close} />

        <div
          class="fixed z-50 w-56 rounded-lg border border-border bg-bg-elevated shadow-xl"
          style={{ top: `${dropdownPos().top}px`, left: `${dropdownPos().left}px` }}
          onKeyDown={(e: KeyboardEvent) => e.key === "Escape" && close()}
        >
          <div class="p-2">
            <div class="px-2 pb-1 pt-1 text-[10px] font-semibold uppercase tracking-widest text-text-muted">
              Add Mix
            </div>

            <Show when={atMax()}>
              <p class="px-2 py-2 text-[10px] text-text-muted">
                Maximum {props.maxMixes} mixes reached
              </p>
            </Show>

            <Show when={!atMax() && availableTemplates().length > 0}>
              <For each={availableTemplates()}>
                {(tmpl) => (
                  <button
                    onClick={() => create(tmpl.name)}
                    class="flex w-full items-center gap-2.5 rounded-md px-2 py-1.5 text-left transition-colors duration-150 hover:bg-bg-hover hover:text-text-primary"
                  >
                    <tmpl.icon size={16} class="shrink-0 text-text-muted" />
                    <span class="text-xs text-text-secondary">{tmpl.name}</span>
                  </button>
                )}
              </For>
            </Show>

            <Show when={!atMax() && availableTemplates().length === 0}>
              <p class="px-2 py-2 text-[10px] text-text-muted">All presets in use</p>
            </Show>

            {/* Custom mix — always visible unless at max */}
            <Show when={!atMax()}>
              <div class="mt-1 border-t border-border pt-2">
                <div class="flex items-center gap-1.5 rounded-md px-2 py-1">
                  <PenLine size={14} class="shrink-0 text-text-muted" />
                  <input
                    type="text"
                    placeholder="Custom name..."
                    value={customName()}
                    onInput={(e) => setCustomName(e.currentTarget.value)}
                    onKeyDown={(e) => {
                      if (e.key === "Enter" && customName().trim()) {
                        create(customName().trim());
                      }
                    }}
                    class="flex-1 rounded border border-border bg-bg-primary px-2 py-1 text-xs text-text-primary placeholder:text-text-muted focus:border-border-active focus:outline-none"
                  />
                  <button
                    onClick={() => customName().trim() && create(customName().trim())}
                    disabled={!customName().trim()}
                    class="rounded bg-accent px-2 py-1 text-xs text-white disabled:opacity-30"
                  >
                    Add
                  </button>
                </div>
              </div>
            </Show>
          </div>
        </div>
      </Show>
    </div>
  );
}
