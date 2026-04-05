import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import { render, fireEvent } from "@solidjs/testing-library";
import { createSignal } from "solid-js";
import MeterSlider from "./MeterSlider";

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

describe("MeterSlider", () => {
  beforeEach(() => mockRaf());
  afterEach(() => restoreRaf());

  it("renders with value and zero peak — VU fill is 0%", () => {
    const { getByTestId } = render(() => (
      <MeterSlider
        value={0.5}
        peak={() => ({ left: 0, right: 0 })}
        onInput={() => {}}
        label="Test volume"
      />
    ));
    flushRaf(16);
    const fill = getByTestId("vu-fill");
    expect(fill.style.width).toBe("0%");
  });

  it("renders with value and active peak — VU fill rises", () => {
    const { getByTestId } = render(() => (
      <MeterSlider
        value={0.8}
        peak={() => ({ left: 0.6, right: 0.5 })}
        onInput={() => {}}
        label="Test volume"
      />
    ));
    // Drive enough rAF ticks to converge (attack tau=30ms)
    for (let i = 1; i <= 15; i++) {
      flushRaf(i * 16);
    }
    const fill = getByTestId("vu-fill");
    const width = parseFloat(fill.style.width);
    // Peak is 0.6 (max of L/R), value is 0.8, so VU should approach 60%
    expect(width).toBeGreaterThanOrEqual(55);
    expect(width).toBeLessThanOrEqual(65);
  });

  it("peak is capped at volume position", () => {
    const { getByTestId } = render(() => (
      <MeterSlider
        value={0.4}
        peak={() => ({ left: 0.9, right: 0.9 })}
        onInput={() => {}}
        label="Test volume"
      />
    ));
    for (let i = 1; i <= 15; i++) {
      flushRaf(i * 16);
    }
    const fill = getByTestId("vu-fill");
    const width = parseFloat(fill.style.width);
    // Peak 0.9 but value 0.4, so VU capped at ~40%
    expect(width).toBeLessThanOrEqual(42);
    expect(width).toBeGreaterThanOrEqual(38);
  });

  it("calls onInput on slider change", () => {
    const handler = vi.fn();
    const { getByTestId } = render(() => (
      <MeterSlider
        value={0.5}
        peak={() => ({ left: 0, right: 0 })}
        onInput={handler}
        label="Test volume"
      />
    ));
    const input = getByTestId("meter-input") as HTMLInputElement;
    fireEvent.input(input, { target: { value: "0.75" } });
    expect(handler).toHaveBeenCalledOnce();
    expect(handler).toHaveBeenCalledWith(0.75);
  });

  it("disabled state: input is disabled", () => {
    const { getByTestId } = render(() => (
      <MeterSlider
        value={0.5}
        peak={() => ({ left: 0, right: 0 })}
        onInput={() => {}}
        label="Test volume"
        disabled
      />
    ));
    const input = getByTestId("meter-input") as HTMLInputElement;
    expect(input.disabled).toBe(true);
  });

  it("muted state reduces VU opacity", () => {
    const { getByTestId } = render(() => (
      <MeterSlider
        value={0.5}
        peak={() => ({ left: 0.5, right: 0.5 })}
        onInput={() => {}}
        label="Test volume"
        muted
      />
    ));
    flushRaf(16);
    const fill = getByTestId("vu-fill");
    expect(fill.style.opacity).toBe("0.1");
  });

  it("stereo mode shows L+R fills", () => {
    const { getByTestId } = render(() => (
      <MeterSlider
        value={0.8}
        peak={() => ({ left: 0.6, right: 0.4 })}
        onInput={() => {}}
        label="Test volume"
        stereo
      />
    ));
    flushRaf(16);
    // Stereo mode should render separate L and R fills
    expect(getByTestId("vu-fill-left")).toBeTruthy();
    expect(getByTestId("vu-fill-right")).toBeTruthy();
  });

  it("stereo mode: L and R fills have different widths for different peaks", () => {
    const { getByTestId } = render(() => (
      <MeterSlider
        value={1}
        peak={() => ({ left: 0.8, right: 0.3 })}
        onInput={() => {}}
        label="Test volume"
        stereo
      />
    ));
    for (let i = 1; i <= 15; i++) {
      flushRaf(i * 16);
    }
    const leftWidth = parseFloat(getByTestId("vu-fill-left").style.width);
    const rightWidth = parseFloat(getByTestId("vu-fill-right").style.width);
    expect(leftWidth).toBeGreaterThan(rightWidth);
  });
});
