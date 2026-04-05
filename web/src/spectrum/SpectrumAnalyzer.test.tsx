/**
 * SpectrumAnalyzer component tests.
 *
 * Verifies:
 * - Canvas renders at specified dimensions
 * - Overlay mode sets transparent background
 * - Subscribes to spectrumStore on mount, unsubscribes on cleanup
 */
import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, cleanup } from "@solidjs/testing-library";

// ---------------------------------------------------------------------------
// Mock spectrumStore — use vi.hoisted so refs are available before hoisting
// ---------------------------------------------------------------------------

const { mockSubscribe, mockUnsubscribe } = vi.hoisted(() => ({
  mockSubscribe: vi.fn(),
  mockUnsubscribe: vi.fn(),
}));

vi.mock("./spectrumStore", () => ({
  spectrumStore: {
    state: { bins: {}, connected: false },
    subscribe: mockSubscribe,
    unsubscribe: mockUnsubscribe,
  },
  SPECTRUM_BINS: 256,
}));

// ---------------------------------------------------------------------------
// Mock requestAnimationFrame / cancelAnimationFrame
// ---------------------------------------------------------------------------

beforeEach(() => {
  mockSubscribe.mockClear();
  mockUnsubscribe.mockClear();

  // Stub rAF: do NOT invoke the callback synchronously — the render loop is
  // recursive and would overflow the stack. Just return a stable ID.
  let rafId = 0;
  vi.stubGlobal("requestAnimationFrame", vi.fn(() => ++rafId));
  vi.stubGlobal("cancelAnimationFrame", vi.fn());

  // Stub canvas getContext so jsdom doesn't crash
  HTMLCanvasElement.prototype.getContext = vi.fn(() => ({
    save: vi.fn(),
    restore: vi.fn(),
    scale: vi.fn(),
    clearRect: vi.fn(),
    fillRect: vi.fn(),
    beginPath: vi.fn(),
    moveTo: vi.fn(),
    lineTo: vi.fn(),
    closePath: vi.fn(),
    fill: vi.fn(),
    stroke: vi.fn(),
    fillText: vi.fn(),
    createLinearGradient: vi.fn(() => ({
      addColorStop: vi.fn(),
    })),
    translate: vi.fn(),
    strokeStyle: "",
    fillStyle: "",
    lineWidth: 1,
    globalAlpha: 1,
    font: "",
    textAlign: "left",
    textBaseline: "alphabetic",
  })) as unknown as HTMLCanvasElement["getContext"];
});

// ---------------------------------------------------------------------------
// Import component AFTER mocks
// ---------------------------------------------------------------------------

import SpectrumAnalyzer from "./SpectrumAnalyzer";

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

describe("SpectrumAnalyzer", () => {
  it("renders a canvas element", () => {
    const { getByTestId } = render(() => (
      <SpectrumAnalyzer nodeKey="test-node" />
    ));
    const canvas = getByTestId("spectrum-canvas");
    expect(canvas.tagName).toBe("CANVAS");
    cleanup();
  });

  it("renders canvas at specified width and height", () => {
    const { getByTestId } = render(() => (
      <SpectrumAnalyzer nodeKey="test-node" width={400} height={150} />
    ));
    const canvas = getByTestId("spectrum-canvas") as HTMLCanvasElement;
    // CSS size is set via style
    expect(canvas.style.width).toBe("400px");
    expect(canvas.style.height).toBe("150px");
    cleanup();
  });

  it("overlay mode sets transparent background", () => {
    const { getByTestId } = render(() => (
      <SpectrumAnalyzer nodeKey="test-node" overlay={true} />
    ));
    const canvas = getByTestId("spectrum-canvas") as HTMLCanvasElement;
    expect(canvas.style.background).toBe("transparent");
    cleanup();
  });

  it("non-overlay mode does not force transparent background", () => {
    const { getByTestId } = render(() => (
      <SpectrumAnalyzer nodeKey="test-node" overlay={false} />
    ));
    const canvas = getByTestId("spectrum-canvas") as HTMLCanvasElement;
    // background should not be explicitly set to transparent
    expect(canvas.style.background).not.toBe("transparent");
    cleanup();
  });

  it("subscribes to spectrumStore on mount", () => {
    render(() => <SpectrumAnalyzer nodeKey="node-xyz" />);
    expect(mockSubscribe).toHaveBeenCalledWith("node-xyz");
    cleanup();
  });

  it("unsubscribes from spectrumStore on cleanup", () => {
    const { unmount } = render(() => <SpectrumAnalyzer nodeKey="node-xyz" />);
    unmount();
    expect(mockUnsubscribe).toHaveBeenCalledWith("node-xyz");
  });
});
