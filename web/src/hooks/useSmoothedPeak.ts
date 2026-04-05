import { createSignal, onCleanup } from "solid-js";
import { useLevels } from "../stores/levelsStore";

const ATTACK = 0.4;
const RELEASE = 0.06;
const POLL_MS = 33; // ~30fps, matches /ws/levels push rate

function smooth(cur: number, target: number): number {
  const alpha = target > cur ? ATTACK : RELEASE;
  return cur + (target - cur) * alpha;
}

/**
 * Poll-based smoothed peak for a single PipeWire node.
 * Bypasses SolidJS reactive tracking entirely — reads the store
 * directly in a setInterval callback and writes to plain signals.
 */
export function useSmoothedPeak(getNodeId: () => number | null | undefined) {
  const levels = useLevels();
  const [left, setLeft] = createSignal(0);
  const [right, setRight] = createSignal(0);

  const id = setInterval(() => {
    const nodeId = getNodeId();
    if (!nodeId) {
      setLeft((c) => smooth(c, 0));
      setRight((c) => smooth(c, 0));
      return;
    }
    const p = levels.peaks[String(nodeId)];
    const tL = p?.left ?? 0;
    const tR = p?.right ?? 0;
    setLeft((c) => smooth(c, tL));
    setRight((c) => smooth(c, tR));
  }, POLL_MS);

  onCleanup(() => clearInterval(id));
  return { left, right };
}

/**
 * Poll-based smoothed peak aggregated across multiple nodes.
 * Takes the max peak from all provided nodeIds each tick.
 * Used for channel labels (max of all cell sinks) and mix headers.
 */
export function useSmoothedAggregatePeak(
  getNodeIds: () => Array<number | null | undefined>,
) {
  const levels = useLevels();
  const [left, setLeft] = createSignal(0);
  const [right, setRight] = createSignal(0);

  const id = setInterval(() => {
    let maxL = 0;
    let maxR = 0;
    for (const nodeId of getNodeIds()) {
      if (!nodeId) continue;
      const p = levels.peaks[String(nodeId)];
      if (p) {
        if (p.left > maxL) maxL = p.left;
        if (p.right > maxR) maxR = p.right;
      }
    }
    setLeft((c) => smooth(c, maxL));
    setRight((c) => smooth(c, maxR));
  }, POLL_MS);

  onCleanup(() => clearInterval(id));
  return { left, right };
}
