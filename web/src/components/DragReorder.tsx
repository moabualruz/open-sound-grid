import { For, createSignal } from "solid-js";
import type { JSX } from "solid-js";

interface DragReorderProps<T> {
  items: T[];
  keyFn: (item: T) => string;
  onReorder: (reordered: T[]) => void;
  children: (item: T, index: () => number, dragHandle: () => JSX.Element) => JSX.Element;
  direction?: "horizontal" | "vertical";
}

export default function DragReorder<T>(props: DragReorderProps<T>): JSX.Element {
  const [dragIdx, setDragIdx] = createSignal<number | null>(null);
  const [overIdx, setOverIdx] = createSignal<number | null>(null);

  function handleDragStart(idx: number) {
    setDragIdx(idx);
  }

  function handleDragOver(e: DragEvent, idx: number) {
    e.preventDefault();
    if (e.dataTransfer) e.dataTransfer.dropEffect = "move";
    setOverIdx(idx);
  }

  function handleDrop(idx: number) {
    const from = dragIdx();
    if (from === null || from === idx) {
      reset();
      return;
    }
    const arr = [...props.items];
    const [moved] = arr.splice(from, 1);
    arr.splice(idx, 0, moved);
    props.onReorder(arr);
    reset();
  }

  function reset() {
    setDragIdx(null);
    setOverIdx(null);
  }

  const isHorizontal = () => props.direction === "horizontal";

  return (
    <For each={props.items}>
      {(item, index) => {
        const isDragging = () => dragIdx() === index();
        const isOver = () => overIdx() === index() && dragIdx() !== null && dragIdx() !== index();

        const dragHandle = () => (
          <div
            draggable={true}
            onDragStart={(e: DragEvent) => {
              e.dataTransfer?.setData("text/plain", String(index()));
              e.dataTransfer!.effectAllowed = "move";
              handleDragStart(index());
            }}
            onDragEnd={reset}
            class="cursor-grab active:cursor-grabbing px-1 py-2"
            aria-label="Drag to reorder"
          >
            <svg
              width="10"
              height="16"
              viewBox="0 0 10 16"
              class="text-text-muted/40 hover:text-text-muted"
            >
              <circle cx="3" cy="3" r="1.5" fill="currentColor" />
              <circle cx="7" cy="3" r="1.5" fill="currentColor" />
              <circle cx="3" cy="8" r="1.5" fill="currentColor" />
              <circle cx="7" cy="8" r="1.5" fill="currentColor" />
              <circle cx="3" cy="13" r="1.5" fill="currentColor" />
              <circle cx="7" cy="13" r="1.5" fill="currentColor" />
            </svg>
          </div>
        );

        return (
          <div
            onDragOver={(e) => handleDragOver(e, index())}
            onDrop={() => handleDrop(index())}
            class={`transition-opacity duration-100 ${isDragging() ? "opacity-30" : ""} ${isHorizontal() ? "min-w-[10rem] flex-1" : ""}`}
            style={{
              [isHorizontal() ? "border-left" : "border-top"]: isOver()
                ? "2px solid var(--color-accent)"
                : "2px solid transparent",
            }}
          >
            {props.children(item, index, dragHandle)}
          </div>
        );
      }}
    </For>
  ) as unknown as JSX.Element;
}
