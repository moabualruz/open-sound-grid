import type { JSX } from "solid-js";
import { useLevels } from "../stores/levelsStore";

interface VuMeterProps {
  nodeId: number | undefined;
}

/** Color class based on peak level: green 0-70%, yellow 70-90%, red 90-100%. */
function peakColorClass(value: number): string {
  if (value >= 0.9) return "bg-vu-hot";
  if (value >= 0.7) return "bg-vu-warm";
  return "bg-vu-safe";
}

/**
 * Compact stereo VU meter bar. Two thin horizontal bars (L/R) stacked vertically.
 * Reads peak levels from the levels store by PipeWire node ID.
 * Total height ~8px (4px per channel).
 */
export default function VuMeter(props: VuMeterProps): JSX.Element {
  const levels = useLevels();

  const peaks = () => {
    if (props.nodeId == null) return { left: 0, right: 0 };
    return levels.peaks[String(props.nodeId)] ?? { left: 0, right: 0 };
  };

  return (
    <div class="flex w-full flex-col gap-px">
      {/* Left channel */}
      <div class="h-1 w-full overflow-hidden rounded-full bg-bg-primary/60">
        <div
          class={`h-full rounded-full transition-[width] duration-[50ms] ease-out ${peakColorClass(peaks().left)}`}
          style={{ width: `${Math.round(peaks().left * 100)}%` }}
        />
      </div>
      {/* Right channel */}
      <div class="h-1 w-full overflow-hidden rounded-full bg-bg-primary/60">
        <div
          class={`h-full rounded-full transition-[width] duration-[50ms] ease-out ${peakColorClass(peaks().right)}`}
          style={{ width: `${Math.round(peaks().right * 100)}%` }}
        />
      </div>
    </div>
  );
}
