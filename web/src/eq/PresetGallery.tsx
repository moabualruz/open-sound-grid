/**
 * Preset Gallery — full-screen overlay showing all presets grouped by category.
 * Supports favorites toggle, click-to-apply, and click-outside-to-close.
 */
import { For, createMemo } from "solid-js";
import { X, Heart } from "lucide-solid";
import type { PresetDef } from "./presets";
import { BUILT_IN_PRESETS, getCustomPresets } from "./presets";

const CATEGORY_LABELS: Record<string, string> = {
  app: "Application",
  mic: "Microphone",
  mix: "Mix Bus",
  cell: "Cell / Route",
};

const CATEGORY_ORDER: PresetDef["category"][] = ["app", "mic", "mix", "cell"];

interface PresetGalleryProps {
  category: string;
  onApply: (preset: PresetDef) => void;
  onClose: () => void;
  favorites: Set<string>;
  onToggleFavorite: (id: string) => void;
}

export default function PresetGallery(props: PresetGalleryProps) {
  const grouped = createMemo(() => {
    const allPresets = [...BUILT_IN_PRESETS, ...getCustomPresets()];
    const groups: Record<string, PresetDef[]> = {};
    for (const p of allPresets) {
      (groups[p.category] ??= []).push(p);
    }
    return groups;
  });

  /** Categories to display: current category first, then the rest. */
  const sortedCategories = createMemo(() => {
    const current = props.category as PresetDef["category"];
    const rest = CATEGORY_ORDER.filter((c) => c !== current && grouped()[c]?.length);
    return grouped()[current]?.length ? [current, ...rest] : rest;
  });

  function handleOverlayClick(e: MouseEvent) {
    if (e.target === e.currentTarget) props.onClose();
  }

  return (
    <div
      class="fixed inset-0 z-50 flex items-center justify-center bg-[var(--color-bg-primary)]/60"
      onClick={handleOverlayClick}
    >
      <div
        class="relative w-full max-w-2xl max-h-[80vh] overflow-y-auto rounded-xl border shadow-2xl"
        style={{
          "background-color": "var(--color-bg-primary)",
          "border-color": "var(--color-border)",
        }}
      >
        {/* Header */}
        <div
          class="sticky top-0 z-10 flex items-center justify-between px-5 py-3 border-b"
          style={{
            "background-color": "var(--color-bg-elevated)",
            "border-color": "var(--color-border)",
          }}
        >
          <span class="text-sm font-semibold" style={{ color: "var(--color-text-primary)" }}>
            Preset Gallery
          </span>
          <button
            class="rounded p-1 transition-colors"
            style={{ color: "var(--color-text-muted)" }}
            onClick={() => props.onClose()}
            title="Close gallery"
          >
            <X size={16} />
          </button>
        </div>

        {/* Category sections */}
        <div class="p-4 flex flex-col gap-5">
          <For each={sortedCategories()}>
            {(cat) => (
              <div>
                <h3
                  class="text-[11px] font-semibold uppercase tracking-wider mb-2"
                  style={{ color: "var(--color-text-muted)" }}
                >
                  {CATEGORY_LABELS[cat] ?? cat}
                </h3>
                <div class="grid grid-cols-2 sm:grid-cols-3 gap-2">
                  <For each={grouped()[cat]}>
                    {(preset) => (
                      <button
                        class="group relative flex flex-col items-start rounded-lg border px-3 py-2.5 text-left transition-colors"
                        style={{
                          "background-color": "var(--color-bg-secondary)",
                          "border-color": "var(--color-border)",
                        }}
                        onClick={() => props.onApply(preset)}
                        title={preset.description}
                      >
                        <div class="flex w-full items-center justify-between">
                          <span
                            class="text-xs font-medium truncate"
                            style={{ color: "var(--color-text-primary)" }}
                          >
                            {preset.name}
                          </span>
                          <button
                            class="ml-1 shrink-0 rounded p-0.5 transition-colors"
                            style={{
                              color: props.favorites.has(preset.id)
                                ? "var(--color-accent)"
                                : "var(--color-text-muted)",
                            }}
                            onClick={(e) => {
                              e.stopPropagation();
                              props.onToggleFavorite(preset.id);
                            }}
                            title={
                              props.favorites.has(preset.id)
                                ? "Remove from favorites"
                                : "Add to favorites"
                            }
                          >
                            <Heart
                              size={12}
                              fill={props.favorites.has(preset.id) ? "currentColor" : "none"}
                            />
                          </button>
                        </div>
                        <span
                          class="mt-0.5 text-[10px] line-clamp-2"
                          style={{ color: "var(--color-text-muted)" }}
                        >
                          {preset.description}
                        </span>
                      </button>
                    )}
                  </For>
                </div>
              </div>
            )}
          </For>
        </div>
      </div>
    </div>
  );
}
