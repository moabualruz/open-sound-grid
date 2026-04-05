import { describe, it, expect, vi } from "vitest";
import { render, fireEvent } from "@solidjs/testing-library";
import { createSignal } from "solid-js";
import VuSlider from "./VuSlider";

describe("VuSlider", () => {
  it("renders with zero peak — VU fill is 0%", () => {
    const { getByTestId } = render(() => (
      <VuSlider value={0.5} peakLeft={0} peakRight={0} onInput={() => {}} label="Test" />
    ));
    expect(getByTestId("vu-fill").style.width).toBe("0%");
  });

  it("renders with peak=1 — VU fill spans the full track", () => {
    const { getByTestId } = render(() => (
      <VuSlider value={0.25} peakLeft={1} peakRight={0.4} onInput={() => {}} label="Test" />
    ));
    const width = parseFloat(getByTestId("vu-fill").style.width);
    expect(width).toBe(100);
  });

  it("peak updates animate via width transition", () => {
    const [peakLeft, setPeakLeft] = createSignal(0.2);
    const { getByTestId } = render(() => (
      <VuSlider value={0.5} peakLeft={peakLeft()} peakRight={0.1} onInput={() => {}} label="Test" />
    ));
    const fill = getByTestId("vu-fill");
    expect(fill.style.width).toBe("20%");
    expect(fill.style.transition).toContain("width");

    setPeakLeft(0.85);
    expect(fill.style.width).toBe("85%");
  });

  it("calls onInput on slider change", () => {
    const handler = vi.fn();
    const { getByTestId } = render(() => (
      <VuSlider value={0.5} peakLeft={0} peakRight={0} onInput={handler} label="Test" />
    ));
    fireEvent.input(getByTestId("vu-input"), { target: { value: "0.75" } });
    expect(handler).toHaveBeenCalledOnce();
    expect(handler).toHaveBeenCalledWith(0.75);
  });

  it("disabled state: input is disabled", () => {
    const { getByTestId } = render(() => (
      <VuSlider value={0.5} peakLeft={0} peakRight={0} onInput={() => {}} label="Test" disabled />
    ));
    expect((getByTestId("vu-input") as HTMLInputElement).disabled).toBe(true);
  });

  it("muted state reduces VU opacity", () => {
    const { getByTestId } = render(() => (
      <VuSlider value={0.5} peakLeft={0.5} peakRight={0.5} onInput={() => {}} label="Test" muted />
    ));
    expect(getByTestId("vu-fill").style.opacity).toBe("0.12");
  });

  it("stereo mode shows L+R fills", () => {
    const { getByTestId } = render(() => (
      <VuSlider value={0.8} peakLeft={0.6} peakRight={0.4} onInput={() => {}} label="Test" stereo />
    ));
    expect(getByTestId("vu-fill-left")).toBeTruthy();
    expect(getByTestId("vu-fill-right")).toBeTruthy();
  });

  it("stereo mode: L and R fills differ for different peaks", () => {
    const { getByTestId } = render(() => (
      <VuSlider value={1} peakLeft={0.8} peakRight={0.3} onInput={() => {}} label="Test" stereo />
    ));
    const leftWidth = parseFloat(getByTestId("vu-fill-left").style.width);
    const rightWidth = parseFloat(getByTestId("vu-fill-right").style.width);
    expect(leftWidth).toBe(80);
    expect(rightWidth).toBe(30);
  });

  it("double-click swaps the slider for an exact-value input", () => {
    const { getByTestId } = render(() => (
      <VuSlider value={0.42} peakLeft={0.2} peakRight={0.1} onInput={() => {}} label="Test" />
    ));
    fireEvent.dblClick(getByTestId("vu-slider"));
    expect((getByTestId("vu-exact-input") as HTMLInputElement).value).toBe("42");
  });

  it("Enter on exact-value input clamps and sends onInput", () => {
    const handler = vi.fn();
    const { getByTestId, queryByTestId } = render(() => (
      <VuSlider value={0.5} peakLeft={0} peakRight={0} onInput={handler} label="Test" />
    ));
    fireEvent.dblClick(getByTestId("vu-slider"));
    const input = getByTestId("vu-exact-input") as HTMLInputElement;
    fireEvent.input(input, { target: { value: "150" } });
    fireEvent.keyDown(input, { key: "Enter" });
    expect(handler).toHaveBeenCalledWith(1);
    expect(queryByTestId("vu-exact-input")).toBeNull();
  });

  it("Escape on exact-value input cancels without sending onInput", () => {
    const handler = vi.fn();
    const { getByTestId, queryByTestId } = render(() => (
      <VuSlider value={0.5} peakLeft={0} peakRight={0} onInput={handler} label="Test" />
    ));
    fireEvent.dblClick(getByTestId("vu-slider"));
    const input = getByTestId("vu-exact-input") as HTMLInputElement;
    fireEvent.input(input, { target: { value: "12" } });
    fireEvent.keyDown(input, { key: "Escape" });
    expect(handler).not.toHaveBeenCalled();
    expect(queryByTestId("vu-exact-input")).toBeNull();
  });
});
