/**
 * Tests for MixHeader — the column header card for each mix destination.
 *
 * Mocks: sessionStore, graphStore, useVolumeDebounce.
 */
import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, fireEvent } from "@solidjs/testing-library";
import type { Endpoint, EndpointDescriptor } from "../types/session";
import type { Command } from "../types/commands";

type MockSendCall = [Command, ...unknown[]];

// ---------------------------------------------------------------------------
// Mocks
// ---------------------------------------------------------------------------

let mockSend: ReturnType<typeof vi.fn>;
let mockGraphDevices: Record<string, unknown>;
let mockGraphNodes: Record<string, unknown>;

vi.mock("../stores/sessionStore", () => ({
  useSession: () => ({ state: { session: { channels: {} } }, send: mockSend }),
}));

vi.mock("../stores/graphStore", () => ({
  useGraph: () => ({
    graph: { devices: mockGraphDevices, nodes: mockGraphNodes },
    connected: true,
  }),
}));

// levelsStore — stub with empty peaks (component now uses useLevels directly)
vi.mock("../stores/levelsStore", () => ({
  useLevels: () => ({ peaks: {}, connected: true }),
}));

vi.mock("../hooks/useVolumeDebounce", () => ({
  useVolumeDebounce: (fn: (v: number) => void) => fn,
}));

import MixHeader from "./MixHeader";

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

function makeDescriptor(id = "mix1"): EndpointDescriptor {
  return { channel: id };
}

function makeEndpoint(overrides: Partial<Endpoint> = {}): Endpoint {
  return {
    descriptor: { channel: "mix1" },
    isPlaceholder: false,
    displayName: "Monitor",
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

function renderHeader(
  endpointOverrides: Partial<Endpoint> = {},
  extraProps: {
    outputDevice?: string | null;
    onToggleExpand?: () => void;
    expanded?: boolean;
    onOpenEq?: () => void;
    onRemove?: () => void;
    onSelectOutput?: (id: string | null) => void;
  } = {},
) {
  return render(() => (
    <MixHeader
      descriptor={makeDescriptor()}
      endpoint={makeEndpoint(endpointOverrides)}
      color="#5090e0"
      outputDevice={extraProps.outputDevice ?? null}
      usedDeviceIds={new Set()}
      onRemove={extraProps.onRemove ?? vi.fn()}
      onSelectOutput={extraProps.onSelectOutput ?? vi.fn()}
      onOpenEq={extraProps.onOpenEq}
      onToggleExpand={extraProps.onToggleExpand}
      expanded={extraProps.expanded}
    />
  ));
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

describe("MixHeader — rendering", () => {
  beforeEach(() => {
    mockSend = vi.fn();
    mockGraphDevices = {};
    mockGraphNodes = {};
  });

  it("renders the mix display name", () => {
    const { getByText } = renderHeader();
    expect(getByText("Monitor")).toBeTruthy();
  });

  it("renders customName when set", () => {
    const { getByText } = renderHeader({ customName: "My Mix" });
    expect(getByText("My Mix")).toBeTruthy();
  });

  it("renders the color bar at the top", () => {
    const { container } = renderHeader();
    // The color bar is a div with the mix color as background-color inline style
    const bar = container.querySelector('div[style*="background-color"]');
    expect(bar).toBeTruthy();
  });

  it("renders the VuSlider (master volume)", () => {
    const { getByTestId } = renderHeader();
    expect(getByTestId("vu-slider")).toBeTruthy();
  });

  it("renders Remove mix button", () => {
    const { getByRole } = renderHeader();
    expect(getByRole("button", { name: /remove mix/i })).toBeTruthy();
  });

  it("shows 'No output' label when outputDevice is null", () => {
    const { getByText } = renderHeader();
    expect(getByText("No output")).toBeTruthy();
  });
});

describe("MixHeader — volume slider", () => {
  beforeEach(() => {
    mockSend = vi.fn();
    mockGraphDevices = {};
    mockGraphNodes = {};
  });

  it("VuSlider input sends setVolume command", () => {
    const { getByTestId } = renderHeader({ volume: 0.8 });
    const slider = getByTestId("vu-input") as HTMLInputElement;
    fireEvent.input(slider, { target: { value: "0.5" } });
    const calls = (mockSend.mock.calls as MockSendCall[]).filter(([cmd]) => cmd.type === "setVolume");
    expect(calls.length).toBeGreaterThanOrEqual(1);
    const cmd = calls[calls.length - 1][0] as Extract<Command, { type: "setVolume" }>;
    expect(cmd.volume).toBeCloseTo(0.5, 5);
    expect(cmd.endpoint).toEqual({ channel: "mix1" });
  });
});

describe("MixHeader — output device picker", () => {
  beforeEach(() => {
    mockSend = vi.fn();
    mockGraphDevices = {};
    mockGraphNodes = {};
  });

  it("clicking output label opens the picker dropdown", () => {
    const { getByText } = renderHeader();
    const outputBtn = getByText("No output").closest("button") as HTMLButtonElement;
    fireEvent.click(outputBtn);
    // Dropdown heading should appear
    expect(getByText("Output Device")).toBeTruthy();
  });

  it("clicking 'None' in the picker calls onSelectOutput(null)", () => {
    const onSelectOutput = vi.fn();
    const { getByText } = renderHeader({}, { onSelectOutput });
    const outputBtn = getByText("No output").closest("button") as HTMLButtonElement;
    fireEvent.click(outputBtn);
    fireEvent.click(getByText("None"));
    expect(onSelectOutput).toHaveBeenCalledWith(null);
  });

  it("available devices from graph are shown in picker", () => {
    mockGraphNodes = {
      "1": {
        id: 1,
        identifier: {
          isMonitor: false,
          nodeName: "alsa_output.usb-Device",
          nodeNick: null,
          nodeDescription: "USB Audio",
          objectPath: null,
        },
        ports: [["p1", "sink", false]],
        clientId: null,
      },
    };
    mockGraphDevices = {
      "dev1": { name: "USB Audio Device", nodes: [1] },
    };
    const { getByText } = renderHeader();
    const outputBtn = getByText("No output").closest("button") as HTMLButtonElement;
    fireEvent.click(outputBtn);
    expect(getByText("USB Audio")).toBeTruthy();
  });
});

describe("MixHeader — effects expand toggle", () => {
  beforeEach(() => {
    mockSend = vi.fn();
    mockGraphDevices = {};
    mockGraphNodes = {};
  });

  it("shows expand button when onToggleExpand is provided", () => {
    const { getByRole } = renderHeader({}, { onToggleExpand: vi.fn(), expanded: false });
    expect(getByRole("button", { name: /expand effects/i })).toBeTruthy();
  });

  it("clicking expand button calls onToggleExpand", () => {
    const onToggleExpand = vi.fn();
    const { getByRole } = renderHeader({}, { onToggleExpand, expanded: false });
    fireEvent.click(getByRole("button", { name: /expand effects/i }));
    expect(onToggleExpand).toHaveBeenCalledOnce();
  });

  it("shows collapse button aria-label when expanded=true", () => {
    const { getByRole } = renderHeader({}, { onToggleExpand: vi.fn(), expanded: true });
    expect(getByRole("button", { name: /collapse effects/i })).toBeTruthy();
  });

  it("expand toggle is absent when onToggleExpand is not provided", () => {
    const { queryByRole } = renderHeader({}, { expanded: false });
    expect(queryByRole("button", { name: /expand effects/i })).toBeNull();
  });
});

describe("MixHeader — inline rename (custom-named mixes)", () => {
  beforeEach(() => {
    mockSend = vi.fn();
    mockGraphDevices = {};
    mockGraphNodes = {};
  });

  it("double-click on a custom-named mix shows rename input", () => {
    // Custom names are those NOT in PRESET_NAMES = ["Monitor","Stream","VOD","Chat","Aux"]
    const { container } = renderHeader({ displayName: "My Mix" });
    const nameSpan = container.querySelector("span.truncate") as HTMLElement;
    expect(nameSpan).toBeTruthy();
    fireEvent.dblClick(nameSpan);
    expect(container.querySelector('input[type="text"]')).toBeTruthy();
  });

  it("Enter in rename input sends renameEndpoint command", () => {
    const { container } = renderHeader({ displayName: "My Mix" });
    const nameSpan = container.querySelector("span.truncate") as HTMLElement;
    fireEvent.dblClick(nameSpan);
    const input = container.querySelector('input[type="text"]') as HTMLInputElement;
    fireEvent.input(input, { target: { value: "Renamed Mix" } });
    fireEvent.keyDown(input, { key: "Enter" });
    const calls = (mockSend.mock.calls as MockSendCall[]).filter(
      ([cmd]) => cmd.type === "renameEndpoint",
    );
    expect(calls.length).toBe(1);
    const cmd = calls[0][0] as Extract<Command, { type: "renameEndpoint" }>;
    expect(cmd.name).toBe("Renamed Mix");
    expect(cmd.endpoint).toEqual({ channel: "mix1" });
  });

  it("double-click on a preset-named mix does NOT open rename input", () => {
    // "Monitor" is in PRESET_NAMES
    const { container } = renderHeader({ displayName: "Monitor" });
    const nameSpan = container.querySelector("span.truncate") as HTMLElement;
    fireEvent.dblClick(nameSpan);
    expect(container.querySelector('input[type="text"]')).toBeNull();
  });
});

describe("MixHeader — context menu", () => {
  beforeEach(() => {
    mockSend = vi.fn();
    mockGraphDevices = {};
    mockGraphNodes = {};
  });

  it("right-click shows mix context menu actions", () => {
    const { container, getByRole } = renderHeader({ displayName: "My Mix" });
    fireEvent.contextMenu(container.firstElementChild as HTMLElement);
    expect(getByRole("menuitem", { name: "Rename" })).toBeTruthy();
    expect(getByRole("menuitem", { name: "Change Output" })).toBeTruthy();
    expect(getByRole("menuitem", { name: "Remove" })).toBeTruthy();
  });

  it("context menu Rename triggers the existing rename flow", () => {
    const { container, getByRole } = renderHeader({ displayName: "My Mix" });
    fireEvent.contextMenu(container.firstElementChild as HTMLElement);
    fireEvent.click(getByRole("menuitem", { name: "Rename" }));
    expect(container.querySelector('input[type="text"]')).toBeTruthy();
  });

  it("context menu Change Output opens the output picker", () => {
    const { container, getByRole, getByText } = renderHeader({ displayName: "My Mix" });
    fireEvent.contextMenu(container.firstElementChild as HTMLElement);
    fireEvent.click(getByRole("menuitem", { name: "Change Output" }));
    expect(getByText("Output Device")).toBeTruthy();
  });

  it("context menu Remove calls onRemove", () => {
    const onRemove = vi.fn();
    const { container, getByRole } = renderHeader({ displayName: "My Mix" }, { onRemove });
    fireEvent.contextMenu(container.firstElementChild as HTMLElement);
    fireEvent.click(getByRole("menuitem", { name: "Remove" }));
    expect(onRemove).toHaveBeenCalledOnce();
  });
});
