import { describe, it, expect, vi } from "vitest";
import { render, fireEvent } from "@solidjs/testing-library";
import MeterSlider from "./MeterSlider";

describe("MeterSlider", () => {
  it("renders with zero peak — VU fill is 0%", () => {
    const { getByTestId } = render(() => (
      <MeterSlider value={0.5} peakLeft={0} peakRight={0} onInput={() => {}} label="Test" />
    ));
    expect(getByTestId("vu-fill").style.width).toBe("0%");
  });

  it("renders with active peak — VU fill matches peak", () => {
    const { getByTestId } = render(() => (
      <MeterSlider value={0.8} peakLeft={0.6} peakRight={0.5} onInput={() => {}} label="Test" />
    ));
    const width = parseFloat(getByTestId("vu-fill").style.width);
    // mono = max(peakLeft, peakRight) = 0.6, capped at value 0.8 → 60%
    expect(width).toBe(60);
  });

  it("peak is capped at volume position", () => {
    const { getByTestId } = render(() => (
      <MeterSlider value={0.4} peakLeft={0.9} peakRight={0.9} onInput={() => {}} label="Test" />
    ));
    const width = parseFloat(getByTestId("vu-fill").style.width);
    // Peak 0.9 but value 0.4 → capped at 40%
    expect(width).toBe(40);
  });

  it("calls onInput on slider change", () => {
    const handler = vi.fn();
    const { getByTestId } = render(() => (
      <MeterSlider value={0.5} peakLeft={0} peakRight={0} onInput={handler} label="Test" />
    ));
    fireEvent.input(getByTestId("meter-input"), { target: { value: "0.75" } });
    expect(handler).toHaveBeenCalledOnce();
    expect(handler).toHaveBeenCalledWith(0.75);
  });

  it("disabled state: input is disabled", () => {
    const { getByTestId } = render(() => (
      <MeterSlider value={0.5} peakLeft={0} peakRight={0} onInput={() => {}} label="Test" disabled />
    ));
    expect((getByTestId("meter-input") as HTMLInputElement).disabled).toBe(true);
  });

  it("muted state reduces VU opacity", () => {
    const { getByTestId } = render(() => (
      <MeterSlider value={0.5} peakLeft={0.5} peakRight={0.5} onInput={() => {}} label="Test" muted />
    ));
    expect(getByTestId("vu-fill").style.opacity).toBe("0.1");
  });

  it("stereo mode shows L+R fills", () => {
    const { getByTestId } = render(() => (
      <MeterSlider value={0.8} peakLeft={0.6} peakRight={0.4} onInput={() => {}} label="Test" stereo />
    ));
    expect(getByTestId("vu-fill-left")).toBeTruthy();
    expect(getByTestId("vu-fill-right")).toBeTruthy();
  });

  it("stereo mode: L and R fills differ for different peaks", () => {
    const { getByTestId } = render(() => (
      <MeterSlider value={1} peakLeft={0.8} peakRight={0.3} onInput={() => {}} label="Test" stereo />
    ));
    const leftWidth = parseFloat(getByTestId("vu-fill-left").style.width);
    const rightWidth = parseFloat(getByTestId("vu-fill-right").style.width);
    expect(leftWidth).toBe(80);
    expect(rightWidth).toBe(30);
  });
});
