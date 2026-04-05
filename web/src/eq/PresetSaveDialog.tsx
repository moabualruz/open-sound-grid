/**
 * Save current EQ config as a custom preset.
 * Small modal with name input, category selector, save/cancel.
 */
import { createSignal, For } from "solid-js";
import { X, Save } from "lucide-solid";
import type { PresetDef } from "./presets";
import { saveCustomPreset } from "./presets";
import type { EqConfig } from "../types/eq";

interface PresetSaveDialogProps {
  currentEq: EqConfig;
  defaultCategory: PresetDef["category"];
  onClose: () => void;
  onSaved: (preset: PresetDef) => void;
}

const CATEGORIES: { value: PresetDef["category"]; label: string }[] = [
  { value: "app", label: "Application" },
  { value: "mic", label: "Microphone" },
  { value: "mix", label: "Mix Bus" },
  { value: "cell", label: "Cell / Route" },
];

export default function PresetSaveDialog(props: PresetSaveDialogProps) {
  const [name, setName] = createSignal("");
  const [category, setCategory] = createSignal<PresetDef["category"]>(props.defaultCategory);

  function handleOverlayClick(e: MouseEvent) {
    if (e.target === e.currentTarget) props.onClose();
  }

  function handleSave() {
    const trimmed = name().trim();
    if (!trimmed) return;

    const preset: PresetDef = {
      id: `custom-${Date.now()}-${Math.random().toString(36).slice(2, 8)}`,
      name: trimmed,
      category: category(),
      description: `Custom preset: ${trimmed}`,
      eq: structuredClone(props.currentEq),
    };

    saveCustomPreset(preset);
    props.onSaved(preset);
  }

  return (
    <div
      class="fixed inset-0 z-50 flex items-center justify-center bg-[var(--color-bg-primary)]/60"
      onClick={handleOverlayClick}
    >
      <div
        class="relative w-full max-w-sm rounded-xl border shadow-2xl"
        style={{
          "background-color": "var(--color-bg-primary)",
          "border-color": "var(--color-border)",
        }}
      >
        {/* Header */}
        <div
          class="flex items-center justify-between px-4 py-2.5 border-b"
          style={{
            "background-color": "var(--color-bg-elevated)",
            "border-color": "var(--color-border)",
          }}
        >
          <div class="flex items-center gap-2">
            <Save size={14} style={{ color: "var(--color-text-secondary)" }} />
            <span class="text-sm font-semibold" style={{ color: "var(--color-text-primary)" }}>
              Save Preset
            </span>
          </div>
          <button
            class="rounded p-1 transition-colors"
            style={{ color: "var(--color-text-muted)" }}
            onClick={() => props.onClose()}
            title="Cancel"
          >
            <X size={16} />
          </button>
        </div>

        {/* Form */}
        <div class="p-4 flex flex-col gap-3">
          <div class="flex flex-col gap-1">
            <label
              class="text-[11px] font-medium uppercase tracking-wide"
              style={{ color: "var(--color-text-muted)" }}
            >
              Name
            </label>
            <input
              class="rounded border px-2.5 py-1.5 text-xs outline-none"
              style={{
                "background-color": "var(--color-bg-secondary)",
                "border-color": "var(--color-border)",
                color: "var(--color-text-primary)",
              }}
              placeholder="My custom preset"
              value={name()}
              onInput={(e) => setName(e.currentTarget.value)}
              onKeyDown={(e) => {
                if (e.key === "Enter") handleSave();
              }}
              autofocus
            />
          </div>

          <div class="flex flex-col gap-1">
            <label
              class="text-[11px] font-medium uppercase tracking-wide"
              style={{ color: "var(--color-text-muted)" }}
            >
              Category
            </label>
            <select
              class="rounded border px-2.5 py-1.5 text-xs"
              style={{
                "background-color": "var(--color-bg-secondary)",
                "border-color": "var(--color-border)",
                color: "var(--color-text-primary)",
              }}
              value={category()}
              onChange={(e) => setCategory(e.currentTarget.value as PresetDef["category"])}
            >
              <For each={CATEGORIES}>{(c) => <option value={c.value}>{c.label}</option>}</For>
            </select>
          </div>

          {/* Actions */}
          <div class="flex items-center justify-end gap-2 pt-1">
            <button
              class="rounded px-3 py-1.5 text-xs transition-colors"
              style={{
                color: "var(--color-text-muted)",
                "background-color": "var(--color-bg-secondary)",
              }}
              onClick={() => props.onClose()}
            >
              Cancel
            </button>
            <button
              class="rounded px-3 py-1.5 text-xs font-medium transition-colors"
              style={{
                "background-color": name().trim() ? "var(--color-accent)" : "var(--color-bg-hover)",
                color: name().trim() ? "var(--color-text-primary)" : "var(--color-text-muted)",
              }}
              onClick={handleSave}
              disabled={!name().trim()}
            >
              Save
            </button>
          </div>
        </div>
      </div>
    </div>
  );
}
