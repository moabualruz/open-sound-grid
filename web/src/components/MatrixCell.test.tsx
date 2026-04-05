/**
 * Tests for MatrixCell — the volume cell at each channel × mix intersection.
 *
 * Mocks: sessionStore, mixerSettings, monitorStore (all context-based hooks).
 * Does NOT mock VuMeter — it renders fine with no nodeId.
 */
import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, fireEvent } from "@solidjs/testing-library";
import type { MixerSession, MixerLink, Endpoint, EndpointDescriptor } from "../types/session";
import type { Command } from "../types/commands";

// ---------------------------------------------------------------------------
// Store mocks — must be declared before component import
// ---------------------------------------------------------------------------

type SendFn = (cmd: Command) => void;

let mockSend: ReturnType<typeof vi.fn>;
let mockStereoMode: "mono" | "stereo";
let mockMonitoredCell: { source: EndpointDescriptor; target: EndpointDescriptor } | null;

vi.mock("../stores/sessionStore", () => ({
  useSession: () => ({ state: {}, send: mockSend }),
}));

vi.mock("../stores/mixerSettings", () => ({
  useMixerSettings: () => ({ settings: { stereoMode: mockStereoMode }, setStereoMode: vi.fn(), setTheme: vi.fn() }),
}));

vi.mock("../stores/monitorStore", () => ({
  useMonitor: () => ({
    state: { monitoredCell: mockMonitoredCell },
    startMonitoring: vi.fn(),
    stopMonitoring: vi.fn(),
    isCellMonitored: vi.fn(() => false),
  }),
}));

// VuMeter uses levelsStore via context — stub it out
vi.mock("./VuMeter", () => ({
  default: () => <div data-testid="vu-meter" />,
}));

// useVolumeDebounce — call the callback synchronously so tests see send() immediately
vi.mock("../hooks/useVolumeDebounce", () => ({
  useVolumeDebounce: (fn: (v: number) => void) => fn,
}));

import MatrixCell from "./MatrixCell";

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

function makeDescriptor(id: string): EndpointDescriptor {
  return { channel: id };
}

function makeEndpoint(overrides: Partial<Endpoint> = {}): Endpoint {
  return {
    descriptor: { channel: "src" },
    isPlaceholder: false,
    displayName: "Music",
    customName: null,
    iconName: "",
    details: [],
    volume: 1,
    volumeLeft: 1,
    volumeRight: 1,
    volumeMixed: false,
    volumeLockedMuted: "unmutedUnlocked",
    visible: true,
    disabled: false,
    ...overrides,
  };
}

function makeLink(overrides: Partial<MixerLink> = {}): MixerLink {
  return {
    start: { channel: "src" },
    end: { channel: "mix1" },
    state: "connectedUnlocked",
    cellVolume: 1,
    cellVolumeLeft: 1,
    cellVolumeRight: 1,
    ...overrides,
  };
}

function renderCell(
  link: MixerLink | null,
  overrides: {
    onOpenEq?: () => void;
    focused?: boolean;
    onActionsReady?: (a: import("./MatrixCell").MatrixCellActions) => void;
    sourceEndpoint?: Endpoint;
  } = {},
) {
  const src = makeDescriptor("src");
  const sink = makeDescriptor("mix1");
  return render(() => (
    <MatrixCell
      link={link}
      sourceEndpoint={overrides.sourceEndpoint ?? makeEndpoint()}
      sourceDescriptor={src}
      sinkDescriptor={sink}
      mixColor="#5090e0"
      onOpenEq={overrides.onOpenEq}
      focused={overrides.focused}
      onActionsReady={overrides.onActionsReady}
    />
  ));
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

describe("MatrixCell — mono mode (default)", () => {
  beforeEach(() => {
    mockSend = vi.fn();
    mockStereoMode = "mono";
    mockMonitoredCell = null;
  });

  it("renders a volume slider when a link exists", () => {
    const { container } = renderCell(makeLink());
    const slider = container.querySelector('input[type="range"][aria-label="Cell volume"]');
    expect(slider).toBeTruthy();
  });

  it("slider initial value reflects link cellVolume", () => {
    const { container } = renderCell(makeLink({ cellVolume: 0.75 }));
    const slider = container.querySelector('input[type="range"]') as HTMLInputElement;
    expect(parseFloat(slider.value)).toBeCloseTo(0.75, 2);
  });

  it("input event sends setLinkVolume command", () => {
    const { container } = renderCell(makeLink());
    const slider = container.querySelector('input[type="range"]') as HTMLInputElement;
    fireEvent.input(slider, { target: { value: "0.60" } });
    const calls = (mockSend as ReturnType<typeof vi.fn>).mock.calls.filter(
      ([cmd]: [Command]) => cmd.type === "setLinkVolume",
    );
    expect(calls.length).toBeGreaterThanOrEqual(1);
    const last = calls[calls.length - 1][0] as Extract<Command, { type: "setLinkVolume" }>;
    expect(last.volume).toBeCloseTo(0.6, 5);
    expect(last.source).toEqual({ channel: "src" });
    expect(last.target).toEqual({ channel: "mix1" });
  });

  it("mute button click sends setLinkVolume with 0 when link exists", () => {
    const { container } = renderCell(makeLink({ cellVolume: 0.8 }));
    const muteBtn = container.querySelector('button[aria-label="Mute cell"]') as HTMLButtonElement;
    expect(muteBtn).toBeTruthy();
    fireEvent.click(muteBtn);
    const calls = (mockSend as ReturnType<typeof vi.fn>).mock.calls.filter(
      ([cmd]: [Command]) => cmd.type === "setLinkVolume",
    );
    expect(calls.length).toBeGreaterThanOrEqual(1);
    const last = calls[calls.length - 1][0] as Extract<Command, { type: "setLinkVolume" }>;
    expect(last.volume).toBe(0);
  });

  it("mute button aria-label changes after muting", () => {
    const { container } = renderCell(makeLink());
    const muteBtn = container.querySelector('button[aria-label="Mute cell"]') as HTMLButtonElement;
    fireEvent.click(muteBtn);
    // After mute, label changes to unmute
    expect(container.querySelector('button[aria-label="Unmute cell"]')).toBeTruthy();
  });

  it("EQ button fires onOpenEq when link exists", () => {
    const onOpenEq = vi.fn();
    const { getByRole } = renderCell(makeLink(), { onOpenEq });
    // aria-label is HTML-encoded in jsdom; use getByRole with name matcher
    const eqBtn = getByRole("button", { name: /EQ/i });
    expect(eqBtn).toBeTruthy();
    fireEvent.click(eqBtn);
    expect(onOpenEq).toHaveBeenCalledOnce();
  });

  it("EQ button is not rendered when no link exists", () => {
    const { queryByRole } = renderCell(null);
    expect(queryByRole("button", { name: /EQ/i })).toBeNull();
  });

  it("empty cell (no link) shows dashed border style class", () => {
    const { container } = renderCell(null);
    const inner = container.querySelector(".border-dashed");
    expect(inner).toBeTruthy();
  });

  it("scroll wheel up increases volume and sends command", () => {
    const { container } = renderCell(makeLink({ cellVolume: 0.5 }));
    const wrapper = container.querySelector(".relative.flex-1") as HTMLElement;
    fireEvent.wheel(wrapper, { deltaY: -1 });
    const calls = (mockSend as ReturnType<typeof vi.fn>).mock.calls.filter(
      ([cmd]: [Command]) => cmd.type === "setLinkVolume",
    );
    expect(calls.length).toBeGreaterThanOrEqual(1);
    const v = (calls[calls.length - 1][0] as Extract<Command, { type: "setLinkVolume" }>).volume;
    expect(v).toBeCloseTo(0.51, 5);
  });

  it("scroll wheel down decreases volume", () => {
    const { container } = renderCell(makeLink({ cellVolume: 0.5 }));
    const wrapper = container.querySelector(".relative.flex-1") as HTMLElement;
    fireEvent.wheel(wrapper, { deltaY: 1 });
    const calls = (mockSend as ReturnType<typeof vi.fn>).mock.calls.filter(
      ([cmd]: [Command]) => cmd.type === "setLinkVolume",
    );
    const v = (calls[calls.length - 1][0] as Extract<Command, { type: "setLinkVolume" }>).volume;
    expect(v).toBeCloseTo(0.49, 5);
  });

  it("focused prop adds ring-2 class to the inner cell container", () => {
    const { container } = renderCell(makeLink(), { focused: true });
    // Source applies "ring-2" class when focused (color via CSS variable, not a separate class)
    const inner = container.querySelector(".ring-2");
    expect(inner).toBeTruthy();
  });

  it("onActionsReady receives toggleMute and adjustVolume", () => {
    const onActionsReady = vi.fn();
    renderCell(makeLink(), { onActionsReady });
    expect(onActionsReady).toHaveBeenCalledOnce();
    const actions = onActionsReady.mock.calls[0][0];
    expect(typeof actions.toggleMute).toBe("function");
    expect(typeof actions.adjustVolume).toBe("function");
  });
});

describe("MatrixCell — stereo mode", () => {
  beforeEach(() => {
    mockSend = vi.fn();
    mockStereoMode = "stereo";
    mockMonitoredCell = null;
  });

  it("renders L and R sliders when stereoMode=stereo", () => {
    const { container } = renderCell(makeLink());
    const leftSlider = container.querySelector('input[aria-label="Cell volume left"]');
    const rightSlider = container.querySelector('input[aria-label="Cell volume right"]');
    expect(leftSlider).toBeTruthy();
    expect(rightSlider).toBeTruthy();
  });

  it("mono slider is not rendered in stereo mode", () => {
    const { container } = renderCell(makeLink());
    expect(container.querySelector('input[aria-label="Cell volume"]')).toBeNull();
  });

  it("L slider input sends setLinkStereoVolume", () => {
    const { container } = renderCell(makeLink());
    const leftSlider = container.querySelector('input[aria-label="Cell volume left"]') as HTMLInputElement;
    fireEvent.input(leftSlider, { target: { value: "0.4" } });
    const calls = (mockSend as ReturnType<typeof vi.fn>).mock.calls.filter(
      ([cmd]: [Command]) => cmd.type === "setLinkStereoVolume",
    );
    expect(calls.length).toBeGreaterThanOrEqual(1);
  });
});
