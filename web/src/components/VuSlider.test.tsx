import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import { render, fireEvent } from "@solidjs/testing-library";
import { createSignal } from "solid-js";
import VuSlider from "./VuSlider";

// ---------------------------------------------------------------------------
// rAF mock helpers
// ---------------------------------------------------------------------------

let rafCallbacks: Map<number, FrameRequestCallback> = new Map();
let rafIdCounter = 0;

function mockRaf() {
  rafCallbacks = new Map();
  rafIdCounter = 0;
  vi.spyOn(globalThis, "requestAnimationFrame").mockImplementation((cb) => {
    const id = ++rafIdCounter;
    rafCallbacks.set(id, cb);
    return id;
  });
  vi.spyOn(globalThis, "cancelAnimationFrame").mockImplementation((id) => {
    rafCallbacks.delete(id);
  });
}

/** Flush all pending rAF callbacks once at the given timestamp. */
function flushRaf(timestamp = 16) {
  const entries = [...rafCallbacks.entries()];
  for (const [id, cb] of entries) {
    rafCallbacks.delete(id);
    cb(timestamp);
  }
}

function restoreRaf() {
  vi.restoreAllMocks();
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

describe("VuSlider", () => {
  beforeEach(() => mockRaf());
  afterEach(() => restoreRaf());

  it("renders with peak=0 — fill width is 0%", () => {
    const { getByTestId } = render(() => (
      <VuSlider value={0.5} peak={0} color="#da7756" onInput={() => {}} />
    ));
    // Flush one rAF tick so smoothedPeak settles
    flushRaf(16);
    const fill = getByTestId("vu-fill");
    // At peak=0 the fill should be at 0%
    expect(fill.style.width).toBe("0%");
  });

  it("renders with peak=1.0 — fill width reaches 100% after smoothing", () => {
    const { getByTestId } = render(() => (
      <VuSlider value={0.5} peak={1} color="#da7756" onInput={() => {}} />
    ));
    // Drive enough rAF ticks at small dt to fully converge (attack tau=50ms)
    // 10 ticks × 16ms = 160ms >> 50ms attack tau → smoothedPeak ≈ 1
    for (let i = 1; i <= 10; i++) {
      flushRaf(i * 16);
    }
    const fill = getByTestId("vu-fill");
    const width = parseFloat(fill.style.width);
    expect(width).toBeGreaterThanOrEqual(95);
  });

  it("slider value change fires onInput with the parsed float", () => {
    const handler = vi.fn();
    const { getByTestId } = render(() => (
      <VuSlider value={0.5} peak={0} color="#da7756" onInput={handler} />
    ));
    const input = getByTestId("vu-input") as HTMLInputElement;
    fireEvent.input(input, { target: { value: "0.75" } });
    expect(handler).toHaveBeenCalledOnce();
    expect(handler).toHaveBeenCalledWith(0.75);
  });

  it("disabled state: input is disabled and fill has low opacity", () => {
    const { getByTestId } = render(() => (
      <VuSlider value={0.5} peak={0.8} color="#da7756" onInput={() => {}} disabled />
    ));
    const input = getByTestId("vu-input") as HTMLInputElement;
    expect(input.disabled).toBe(true);

    flushRaf(16);
    const fill = getByTestId("vu-fill");
    // Disabled fill opacity is 0.08 per spec
    expect(fill.style.opacity).toBe("0.08");
  });

  it("peak smoothing: rapid peak rise converges with rAF ticks (attack tau=50ms)", () => {
    // Start at 0, then provide peak=1 — after several 16ms ticks it must be rising
    const [peak, setPeak] = createSignal(0);
    const { getByTestId } = render(() => (
      <VuSlider value={0.5} peak={peak()} color="#da7756" onInput={() => {}} />
    ));

    // Initial tick at 0 — fill should be 0
    flushRaf(16);
    expect(parseFloat((getByTestId("vu-fill") as HTMLElement).style.width)).toBe(0);

    // Raise peak to 1
    setPeak(1);

    // After 1 tick (16 ms), smoothed should have risen meaningfully
    flushRaf(32);
    const widthAfter1 = parseFloat((getByTestId("vu-fill") as HTMLElement).style.width);
    expect(widthAfter1).toBeGreaterThan(0);

    // After several more ticks it approaches 100
    for (let i = 3; i <= 12; i++) {
      flushRaf(i * 16);
    }
    const widthFinal = parseFloat((getByTestId("vu-fill") as HTMLElement).style.width);
    expect(widthFinal).toBeGreaterThanOrEqual(95);
  });
});
