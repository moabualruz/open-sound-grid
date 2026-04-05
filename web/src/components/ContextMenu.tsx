import { For, Show, createEffect, createSignal, onCleanup } from "solid-js";
import type { JSX } from "solid-js";

export interface ContextMenuItem {
  label: string;
  onSelect: () => void;
  disabled?: boolean;
  danger?: boolean;
}

interface ContextMenuProps {
  open: boolean;
  position: { x: number; y: number } | null;
  items: ContextMenuItem[];
  onClose: () => void;
}

export default function ContextMenu(props: ContextMenuProps): JSX.Element {
  const [resolvedPosition, setResolvedPosition] = createSignal({ x: 0, y: 0 });
  let menuRef: HTMLDivElement | undefined;

  createEffect(() => {
    if (!props.open || !props.position) return;
    const position = props.position;

    setResolvedPosition(position);

    const rafId = window.requestAnimationFrame(() => {
      if (!menuRef) return;
      const rect = menuRef.getBoundingClientRect();
      const maxLeft = Math.max(8, window.innerWidth - rect.width - 8);
      const maxTop = Math.max(8, window.innerHeight - rect.height - 8);
      setResolvedPosition({
        x: Math.min(position.x, maxLeft),
        y: Math.min(position.y, maxTop),
      });
    });

    onCleanup(() => window.cancelAnimationFrame(rafId));
  });

  createEffect(() => {
    if (!props.open) return;

    const handleKeyDown = (event: KeyboardEvent) => {
      if (event.key === "Escape") props.onClose();
    };

    document.addEventListener("keydown", handleKeyDown);
    onCleanup(() => document.removeEventListener("keydown", handleKeyDown));
  });

  return (
    <Show when={props.open && props.position}>
      <div
        class="fixed inset-0 z-40"
        onMouseDown={() => props.onClose()}
        onContextMenu={(event) => event.preventDefault()}
      />
      <div
        ref={menuRef}
        role="menu"
        class="fixed z-50 min-w-40 rounded-lg border border-border bg-bg-elevated p-1 shadow-xl"
        style={{
          top: `${resolvedPosition().y}px`,
          left: `${resolvedPosition().x}px`,
        }}
        onMouseDown={(event) => event.stopPropagation()}
        onContextMenu={(event) => event.preventDefault()}
      >
        <For each={props.items}>
          {(item) => (
            <button
              type="button"
              role="menuitem"
              disabled={item.disabled}
              onClick={() => {
                if (item.disabled) return;
                item.onSelect();
                props.onClose();
              }}
              class={`flex w-full items-center rounded-md px-3 py-1.5 text-left text-xs transition-colors duration-150 ${
                item.disabled
                  ? "cursor-not-allowed text-text-muted/50"
                  : item.danger
                    ? "text-vu-hot hover:bg-vu-hot/10"
                    : "text-text-secondary hover:bg-bg-hover hover:text-text-primary"
              }`}
            >
              {item.label}
            </button>
          )}
        </For>
      </div>
    </Show>
  );
}
