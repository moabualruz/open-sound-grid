/**
 * Effects blocks — non-EQ processing that varies by source type.
 *
 * Source type determines which effects are available:
 * ┌───────────────┬────────────┬───────┬──────┬──────┐
 * │ Effect        │ App/Device │ Cell  │ Mix  │ Mic  │
 * ├───────────────┼────────────┼───────┼──────┼──────┤
 * │ Noise Gate    │ —          │ —     │ —    │ YES  │
 * │ De-Esser      │ —          │ —     │ —    │ YES  │
 * │ Compressor    │ YES        │ YES   │ YES  │ YES  │
 * │ Limiter       │ —          │ —     │ YES  │ —    │
 * │ Smart Volume  │ YES        │ —     │ YES  │ —    │
 * │ Volume Boost  │ YES        │ YES   │ YES  │ YES  │
 * │ Spatial Audio │ —          │ —     │ YES  │ —    │
 * └───────────────┴────────────┴───────┴──────┴──────┘
 */
import { createSignal, Show, For, untrack } from "solid-js";
import type { EffectsConfig } from "../types";
import {
  getEffectsForType,
  defaultEffectsConfig,
  buildEffectsConfig,
  getInitialFromConfig,
  MAPPED_EFFECTS,
} from "./effectDefinitions";
import type { SourceType, EffectDef } from "./effectDefinitions";

export type { SourceType };

interface EffectsBlockProps {
  sourceType: SourceType;
  color?: string;
  initialEffects?: EffectsConfig;
  onEffectsChange?: (effects: EffectsConfig) => void;
}

export default function EffectsBlock(props: EffectsBlockProps) {
  const effects = () => getEffectsForType(props.sourceType);
  const cardStates = new Map<string, { enabled: boolean; values: Record<string, number> }>();
  const baseConfig = () => props.initialEffects ?? defaultEffectsConfig();

  function handleCardChange(effectId: string, enabled: boolean, values: Record<string, number>) {
    cardStates.set(effectId, { enabled, values });
    if (MAPPED_EFFECTS.has(effectId)) {
      props.onEffectsChange?.(buildEffectsConfig(cardStates, baseConfig()));
    }
  }

  return (
    <Show when={effects().length > 0}>
      <div class="flex flex-col gap-2">
        <For each={effects()}>
          {(effect) => (
            <EffectCard
              effect={effect}
              color={props.color}
              initialEffects={props.initialEffects}
              onChange={(enabled, values) => handleCardChange(effect.id, enabled, values)}
            />
          )}
        </For>
      </div>
    </Show>
  );
}

interface EffectCardProps {
  effect: EffectDef;
  color?: string;
  initialEffects?: EffectsConfig;
  onChange?: (enabled: boolean, values: Record<string, number>) => void;
}

function EffectCard(props: EffectCardProps) {
  const isComingSoon = () => props.effect.comingSoon === true;
  const initial = untrack(() => getInitialFromConfig(props.effect.id, props.initialEffects));
  const [enabled, setEnabled] = createSignal(isComingSoon() ? false : (initial?.enabled ?? false));
  const [values, setValues] = createSignal<Record<string, number>>(
    untrack(() => {
      const defaults = Object.fromEntries(props.effect.controls.map((c) => [c.id, c.defaultValue]));
      return initial?.values ? { ...defaults, ...initial.values } : defaults;
    }),
  );
  const [options, setOptions] = createSignal<Record<string, string>>(
    untrack(() =>
      Object.fromEntries((props.effect.options ?? []).map((o) => [o.id, o.defaultValue])),
    ),
  );

  const updateValue = (controlId: string, value: number) => {
    setValues((prev) => {
      const next = { ...prev, [controlId]: value };
      props.onChange?.(enabled(), next);
      return next;
    });
  };

  const updateOption = (optionId: string, value: string) => {
    setOptions((prev) => ({ ...prev, [optionId]: value }));
  };

  return (
    <div
      class="overflow-hidden rounded-lg border"
      style={{
        "border-color": enabled() ? (props.color ?? "var(--color-accent)") : "var(--color-border)",
        "background-color": "var(--color-bg-primary)",
        opacity: enabled() ? 1 : 0.7,
      }}
    >
      {/* Header */}
      <div
        class="flex items-center justify-between px-3 py-1.5"
        style={{ "background-color": "var(--color-bg-elevated)" }}
      >
        <div class="flex items-center gap-2">
          <button
            class="relative h-3.5 w-7 rounded-full transition-colors"
            style={{
              "background-color": enabled()
                ? (props.color ?? "var(--color-accent)")
                : "var(--color-bg-hover)",
              opacity: isComingSoon() ? 0.4 : 1,
              cursor: isComingSoon() ? "not-allowed" : "pointer",
            }}
            disabled={isComingSoon()}
            onClick={() => {
              if (isComingSoon()) return;
              const next = !enabled();
              setEnabled(next);
              props.onChange?.(next, values());
            }}
          >
            <div
              class="absolute top-0.5 h-2.5 w-2.5 rounded-full transition-all duration-150"
              style={{
                left: enabled() ? "15px" : "2px",
                "background-color": "var(--color-text-primary)",
              }}
            />
          </button>
          <span
            class="text-[11px] font-medium uppercase tracking-wide"
            style={{
              color: enabled() ? "var(--color-text-secondary)" : "var(--color-text-muted)",
            }}
          >
            {props.effect.label}
            <Show when={isComingSoon()}>
              <span
                class="ml-1 text-[8px] normal-case"
                style={{ color: "var(--color-text-muted)" }}
              >
                (Coming Soon)
              </span>
            </Show>
          </span>
        </div>
        <span
          class="max-w-[50%] text-right text-[9px]"
          style={{ color: "var(--color-text-muted)" }}
        >
          {props.effect.description}
        </span>
      </div>

      {/* Controls */}
      <Show when={enabled()}>
        <div class="flex flex-col gap-2 px-3 py-2">
          {/* Option selectors (e.g. Spatial Audio mode) */}
          <Show when={props.effect.options && props.effect.options.length > 0}>
            <div class="flex flex-wrap gap-3">
              <For each={props.effect.options ?? []}>
                {(opt) => (
                  <div class="flex items-center gap-2">
                    <span class="text-[10px]" style={{ color: "var(--color-text-muted)" }}>
                      {opt.label}
                    </span>
                    <div
                      class="flex overflow-hidden rounded border"
                      style={{ "border-color": "var(--color-border)" }}
                    >
                      <For each={opt.options}>
                        {(val) => (
                          <button
                            class="px-2 py-0.5 text-[10px] font-medium transition-colors"
                            style={{
                              "background-color":
                                options()[opt.id] === val
                                  ? (props.color ?? "var(--color-accent)")
                                  : "var(--color-bg-primary)",
                              color:
                                options()[opt.id] === val
                                  ? "var(--color-text-primary)"
                                  : "var(--color-text-muted)",
                            }}
                            onClick={() => updateOption(opt.id, val)}
                          >
                            {val}
                          </button>
                        )}
                      </For>
                    </div>
                  </div>
                )}
              </For>
            </div>
          </Show>

          {/* Slider controls */}
          <div class="flex flex-wrap gap-x-4 gap-y-2">
            <For each={props.effect.controls}>
              {(ctrl) => (
                <div class="flex min-w-[180px] flex-1 items-center gap-2">
                  <span
                    class="w-16 shrink-0 text-right text-[10px]"
                    style={{ color: "var(--color-text-muted)" }}
                  >
                    {ctrl.label}
                  </span>
                  <input
                    type="range"
                    min={ctrl.min}
                    max={ctrl.max}
                    step={ctrl.step}
                    value={values()[ctrl.id]}
                    class="min-w-0 flex-1"
                    onInput={(e) => updateValue(ctrl.id, parseFloat(e.currentTarget.value))}
                  />
                  <span
                    class="w-16 shrink-0 font-mono tabular-nums text-[10px]"
                    style={{ color: "var(--color-text-secondary)" }}
                  >
                    {values()[ctrl.id]?.toFixed(ctrl.step < 1 ? 1 : 0)} {ctrl.unit}
                  </span>
                </div>
              )}
            </For>
          </div>
        </div>
      </Show>
    </div>
  );
}
