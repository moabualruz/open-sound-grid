/**
 * Tests for MixEffectsRow — inline effects overview rendered when a mix is expanded.
 * Covers: expand/collapse, accordion behavior, EQ thumbnail rendering,
 * effects badges, and EQ page navigation trigger.
 */
import { describe, it, expect, vi } from "vitest";
import { render, fireEvent } from "@solidjs/testing-library";
import { createSignal } from "solid-js";
import MixEffectsRow, { EffectsBadges, type MixEffectsCellData } from "./MixEffectsRow";
import type { EqConfig } from "../types/eq";
import type { EffectsConfig } from "../types/effects";
import type { EndpointDescriptor } from "../types/session";

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

function makeDescriptor(id: string): EndpointDescriptor {
  return { channel: id };
}

function makeLinkedCell(
  id: string,
  overrides: Partial<MixEffectsCellData> = {},
): MixEffectsCellData {
  return {
    sourceDescriptor: makeDescriptor(id),
    linked: true,
    ...overrides,
  };
}

function makeUnlinkedCell(id: string): MixEffectsCellData {
  return {
    sourceDescriptor: makeDescriptor(id),
    linked: false,
  };
}

const eqWithBands: EqConfig = {
  enabled: true,
  bands: [
    { enabled: true, filterType: "peaking", frequency: 1000, gain: 3, q: 0.707 },
    { enabled: true, filterType: "highShelf", frequency: 8000, gain: -2, q: 1 },
  ],
};

const eqDisabled: EqConfig = {
  enabled: false,
  bands: [{ enabled: true, filterType: "peaking", frequency: 1000, gain: 3, q: 0.707 }],
};

const effectsAllActive: EffectsConfig = {
  compressor: { enabled: true, threshold: -20, ratio: 4, attack: 10, release: 100, makeup: 0 },
  gate: { enabled: true, threshold: -60, hold: 100, attack: 0.5, release: 50 },
  deEsser: { enabled: true, frequency: 6000, threshold: -20, reduction: -6 },
  limiter: { enabled: true, ceiling: -0.3, release: 50 },
  boost: 6,
  smartVolume: { enabled: true, targetDb: -18, speed: 0.3, maxGainDb: 12 },
};

const effectsAllDisabled: EffectsConfig = {
  compressor: { enabled: false, threshold: -20, ratio: 4, attack: 10, release: 100, makeup: 0 },
  gate: { enabled: false, threshold: -60, hold: 100, attack: 0.5, release: 50 },
  deEsser: { enabled: false, frequency: 6000, threshold: -20, reduction: -6 },
  limiter: { enabled: false, ceiling: -0.3, release: 50 },
  boost: 0,
  smartVolume: { enabled: false, targetDb: -18, speed: 0.3, maxGainDb: 12 },
};

// ---------------------------------------------------------------------------
// MixEffectsRow — renders effects sub-rows when provided cells
// ---------------------------------------------------------------------------

describe("MixEffectsRow", () => {
  it("renders a row container with role=row", () => {
    const { container } = render(() => (
      <MixEffectsRow
        cells={[makeLinkedCell("ch1")]}
        mixColor="#5090e0"
        gridTemplateColumns="12rem 1fr"
        onOpenCellEq={() => {}}
      />
    ));
    const row = container.querySelector('[role="row"]');
    expect(row).toBeTruthy();
  });

  it("renders one gridcell per cell entry", () => {
    const { container } = render(() => (
      <MixEffectsRow
        cells={[makeLinkedCell("ch1"), makeLinkedCell("ch2"), makeLinkedCell("ch3")]}
        mixColor="#5090e0"
        gridTemplateColumns="12rem 1fr 1fr 1fr"
        onOpenCellEq={() => {}}
      />
    ));
    const cells = container.querySelectorAll('[role="gridcell"]');
    expect(cells.length).toBe(3);
  });

  it("renders canvas EQ thumbnail for linked cells", () => {
    const { container } = render(() => (
      <MixEffectsRow
        cells={[makeLinkedCell("ch1", { cellEq: eqWithBands })]}
        mixColor="#5090e0"
        gridTemplateColumns="12rem 1fr"
        onOpenCellEq={() => {}}
      />
    ));
    const canvas = container.querySelector("canvas");
    expect(canvas).toBeTruthy();
  });

  it("does not render canvas for unlinked cells", () => {
    const { container } = render(() => (
      <MixEffectsRow
        cells={[makeUnlinkedCell("ch1")]}
        mixColor="#5090e0"
        gridTemplateColumns="12rem 1fr"
        onOpenCellEq={() => {}}
      />
    ));
    const canvas = container.querySelector("canvas");
    expect(canvas).toBeNull();
  });

  it("calls onOpenCellEq with the correct source descriptor when thumbnail is clicked", async () => {
    const onOpenCellEq = vi.fn();
    const descriptor = makeDescriptor("mic-channel");
    const { container } = render(() => (
      <MixEffectsRow
        cells={[makeLinkedCell("mic-channel", { cellEq: eqWithBands })]}
        mixColor="#5090e0"
        gridTemplateColumns="12rem 1fr"
        onOpenCellEq={onOpenCellEq}
      />
    ));
    const canvas = container.querySelector("canvas");
    expect(canvas).toBeTruthy();
    canvas?.click();
    expect(onOpenCellEq).toHaveBeenCalledTimes(1);
    // Verify the descriptor passed matches
    expect(onOpenCellEq).toHaveBeenCalledWith(descriptor);
  });
});

// ---------------------------------------------------------------------------
// Accordion behavior — tested via signal-driven rendering in parent
// ---------------------------------------------------------------------------

describe("MixEffectsRow accordion behavior", () => {
  it("shows effects row when expanded signal is set", () => {
    const cells = [makeLinkedCell("ch1")];

    // Drive expand state via a button click inside the reactive render scope
    const { container, getByTestId } = render(() => {
      const [expanded, setExpanded] = createSignal(false);
      return (
        <>
          <button data-testid="toggle" onClick={() => setExpanded((v) => !v)} />
          {expanded() ? (
            <MixEffectsRow
              cells={cells}
              mixColor="#5090e0"
              gridTemplateColumns="12rem 1fr"
              onOpenCellEq={() => {}}
            />
          ) : null}
        </>
      );
    });

    // Initially collapsed — no row
    expect(container.querySelector('[role="row"]')).toBeNull();

    // Expand via click (SolidJS reactive update within the render scope)
    fireEvent.click(getByTestId("toggle"));
    expect(container.querySelector('[role="row"]')).toBeTruthy();
  });

  it("hides effects row when expanded signal is cleared", () => {
    const cells = [makeLinkedCell("ch1")];

    const { container, getByTestId } = render(() => {
      const [expanded, setExpanded] = createSignal(true);
      return (
        <>
          <button data-testid="toggle" onClick={() => setExpanded((v) => !v)} />
          {expanded() ? (
            <MixEffectsRow
              cells={cells}
              mixColor="#5090e0"
              gridTemplateColumns="12rem 1fr"
              onOpenCellEq={() => {}}
            />
          ) : null}
        </>
      );
    });

    expect(container.querySelector('[role="row"]')).toBeTruthy();

    fireEvent.click(getByTestId("toggle"));
    expect(container.querySelector('[role="row"]')).toBeNull();
  });

  it("expanding one mix collapses another (accordion)", () => {
    const [expandedKey, setExpandedKey] = createSignal<string | null>(null);

    function toggle(key: string) {
      setExpandedKey((curr) => (curr === key ? null : key));
    }

    const { queryByTestId } = render(() => (
      <>
        <div data-testid="mix-a">
          {expandedKey() === "a" ? <span data-testid="effects-a">effects-a</span> : null}
        </div>
        <div data-testid="mix-b">
          {expandedKey() === "b" ? <span data-testid="effects-b">effects-b</span> : null}
        </div>
        <button data-testid="toggle-a" onClick={() => toggle("a")}>
          Toggle A
        </button>
        <button data-testid="toggle-b" onClick={() => toggle("b")}>
          Toggle B
        </button>
      </>
    ));

    // Nothing expanded initially
    expect(queryByTestId("effects-a")).toBeNull();
    expect(queryByTestId("effects-b")).toBeNull();

    // Expand A
    queryByTestId("toggle-a")!.click();
    expect(queryByTestId("effects-a")).toBeTruthy();
    expect(queryByTestId("effects-b")).toBeNull();

    // Expand B — A should collapse
    queryByTestId("toggle-b")!.click();
    expect(queryByTestId("effects-a")).toBeNull();
    expect(queryByTestId("effects-b")).toBeTruthy();

    // Click B again — should collapse
    queryByTestId("toggle-b")!.click();
    expect(queryByTestId("effects-a")).toBeNull();
    expect(queryByTestId("effects-b")).toBeNull();
  });
});

// ---------------------------------------------------------------------------
// EffectsBadges — active/inactive/off state
// ---------------------------------------------------------------------------

describe("EffectsBadges", () => {
  it("renders active badges for all enabled effects", () => {
    const { container } = render(() => <EffectsBadges effects={effectsAllActive} />);
    const dots = container.querySelectorAll("span[title]");
    // comp, gate, deEsser, limiter, smartVolume = 5 + boost dot = 6
    const activeDots = Array.from(dots).filter((d) => d.getAttribute("title")?.includes("active"));
    expect(activeDots.length).toBeGreaterThanOrEqual(5);
  });

  it("renders inactive (gray) dots for disabled effects that exist", () => {
    const { container } = render(() => <EffectsBadges effects={effectsAllDisabled} />);
    const dots = container.querySelectorAll("span[title]");
    const inactiveDots = Array.from(dots).filter((d) =>
      d.getAttribute("title")?.includes("disabled"),
    );
    // All 5 structural effects are disabled
    expect(inactiveDots.length).toBe(5);
  });

  it("renders no dots when effects is undefined", () => {
    const { container } = render(() => <EffectsBadges effects={undefined} />);
    const dots = container.querySelectorAll("span[title]");
    expect(dots.length).toBe(0);
  });

  it("renders boost dot when boost > 0", () => {
    const effects: EffectsConfig = { ...effectsAllDisabled, boost: 3 };
    const { container } = render(() => <EffectsBadges effects={effects} />);
    const boostDot = container.querySelector('span[aria-label="Volume Boost active"]');
    expect(boostDot).toBeTruthy();
  });

  it("does not render boost dot when boost = 0", () => {
    const { container } = render(() => <EffectsBadges effects={effectsAllDisabled} />);
    const boostDot = container.querySelector('span[aria-label="Volume Boost active"]');
    expect(boostDot).toBeNull();
  });
});

// ---------------------------------------------------------------------------
// EQ thumbnail shape — canvas renders without error for various EqConfigs
// ---------------------------------------------------------------------------

describe("EqThumbnail via MixEffectsRow", () => {
  it("renders canvas for active EQ with bands", () => {
    const { container } = render(() => (
      <MixEffectsRow
        cells={[makeLinkedCell("ch1", { cellEq: eqWithBands })]}
        mixColor="#da7756"
        gridTemplateColumns="12rem 1fr"
        onOpenCellEq={() => {}}
      />
    ));
    const canvas = container.querySelector("canvas");
    expect(canvas).toBeTruthy();
    expect(canvas?.getAttribute("aria-label")).toBeTruthy();
  });

  it("renders canvas for disabled EQ (flat line mode)", () => {
    const { container } = render(() => (
      <MixEffectsRow
        cells={[makeLinkedCell("ch1", { cellEq: eqDisabled })]}
        mixColor="#da7756"
        gridTemplateColumns="12rem 1fr"
        onOpenCellEq={() => {}}
      />
    ));
    const canvas = container.querySelector("canvas");
    expect(canvas).toBeTruthy();
  });

  it("renders canvas when no EQ config (undefined)", () => {
    const { container } = render(() => (
      <MixEffectsRow
        cells={[makeLinkedCell("ch1", { cellEq: undefined })]}
        mixColor="#da7756"
        gridTemplateColumns="12rem 1fr"
        onOpenCellEq={() => {}}
      />
    ));
    const canvas = container.querySelector("canvas");
    expect(canvas).toBeTruthy();
  });

  it("canvas has button role for keyboard navigation", () => {
    const { container } = render(() => (
      <MixEffectsRow
        cells={[makeLinkedCell("ch1", { cellEq: eqWithBands })]}
        mixColor="#da7756"
        gridTemplateColumns="12rem 1fr"
        onOpenCellEq={() => {}}
      />
    ));
    const canvas = container.querySelector("canvas");
    expect(canvas?.getAttribute("role")).toBe("button");
  });
});
