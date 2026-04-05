/**
 * Tests for ChannelLabel — the left-column label card for each channel row.
 *
 * Mocks: sessionStore, mixerSettings, AppAssignment, useVolumeDebounce.
 */
import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, fireEvent } from "@solidjs/testing-library";
import type { Endpoint, EndpointDescriptor, Channel } from "../types/session";
import type { Command } from "../types/commands";

type MockSendCall = [Command, ...unknown[]];

// ---------------------------------------------------------------------------
// Mocks
// ---------------------------------------------------------------------------

let mockSend: ReturnType<typeof vi.fn>;
let mockStereoMode: "mono" | "stereo";

vi.mock("../stores/sessionStore", () => ({
  useSession: () => ({ state: { session: { links: [] } }, send: mockSend }),
}));

vi.mock("../stores/graphStore", () => ({
  useGraph: () => ({
    graph: { nodes: {} },
    connected: true,
  }),
}));

vi.mock("../stores/mixerSettings", () => ({
  useMixerSettings: () => ({
    settings: { stereoMode: mockStereoMode },
    setStereoMode: vi.fn(),
    setTheme: vi.fn(),
  }),
}));

// levelsStore — stub with empty peaks (component now uses useLevels directly)
vi.mock("../stores/levelsStore", () => ({
  useLevels: () => ({ peaks: {}, connected: true }),
}));

vi.mock("./AppAssignment", () => ({
  default: () => <div data-testid="app-assignment" />,
}));

vi.mock("../hooks/useVolumeDebounce", () => ({
  useVolumeDebounce: (fn: (v: number) => void) => fn,
}));

import ChannelLabel from "./ChannelLabel";

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

function makeDescriptor(id = "ch1"): EndpointDescriptor {
  return { channel: id };
}

function makeEndpoint(overrides: Partial<Endpoint> = {}): Endpoint {
  return {
    descriptor: { channel: "ch1" },
    isPlaceholder: false,
    displayName: "Music",
    customName: null,
    iconName: "",
    details: [],
    volume: 0.8,
    volumeLeft: 0.8,
    volumeRight: 0.8,
    volumeMixed: false,
    volumeLockedMuted: "unmutedUnlocked",
    visible: true,
    disabled: false,
    ...overrides,
  };
}

function makeChannel(overrides: Partial<Channel> = {}): Channel {
  return {
    id: "ch1",
    kind: "duplex",
    outputNodeId: null,
    assignedApps: [],
    autoApp: false,
    allowAppAssignment: true,
    ...overrides,
  };
}

function renderLabel(
  endpointOverrides: Partial<Endpoint> = {},
  channel?: Channel,
) {
  return render(() => (
    <ChannelLabel
      descriptor={makeDescriptor()}
      endpoint={makeEndpoint(endpointOverrides)}
      channel={channel}
      apps={[]}
    />
  ));
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

describe("ChannelLabel — rendering", () => {
  beforeEach(() => {
    mockSend = vi.fn();
    mockStereoMode = "mono";
  });

  it("renders the channel display name", () => {
    const { getByText } = renderLabel();
    expect(getByText("Music")).toBeTruthy();
  });

  it("renders customName when set", () => {
    const { getByText } = renderLabel({ customName: "My Custom" });
    expect(getByText("My Custom")).toBeTruthy();
  });

  it("renders VuSlider", () => {
    const { getByTestId } = renderLabel();
    expect(getByTestId("vu-slider")).toBeTruthy();
  });

  it("renders AppAssignment when channel prop is provided", () => {
    const { getByTestId } = renderLabel({}, makeChannel());
    expect(getByTestId("app-assignment")).toBeTruthy();
  });

  it("does not render AppAssignment when channel prop is absent", () => {
    const { queryByTestId } = renderLabel();
    expect(queryByTestId("app-assignment")).toBeNull();
  });
});

describe("ChannelLabel — mute button", () => {
  beforeEach(() => {
    mockSend = vi.fn();
    mockStereoMode = "mono";
  });

  it("mute button sends setMute(true) when unmuted", () => {
    const { getByRole } = renderLabel({ volumeLockedMuted: "unmutedUnlocked" });
    const muteBtn = getByRole("button", { name: /mute channel/i });
    fireEvent.click(muteBtn);
    const calls = (mockSend.mock.calls as MockSendCall[]).filter(([cmd]) => cmd.type === "setMute");
    expect(calls.length).toBe(1);
    const cmd = calls[0][0] as Extract<Command, { type: "setMute" }>;
    expect(cmd.muted).toBe(true);
    expect(cmd.endpoint).toEqual({ channel: "ch1" });
  });

  it("mute button sends setMute(false) when already muted", () => {
    const { getByRole } = renderLabel({ volumeLockedMuted: "mutedUnlocked" });
    const unmuteBtn = getByRole("button", { name: /unmute channel/i });
    fireEvent.click(unmuteBtn);
    const calls = (mockSend.mock.calls as MockSendCall[]).filter(([cmd]) => cmd.type === "setMute");
    expect(calls.length).toBe(1);
    const cmd = calls[0][0] as Extract<Command, { type: "setMute" }>;
    expect(cmd.muted).toBe(false);
  });
});

describe("ChannelLabel — hide button", () => {
  beforeEach(() => {
    mockSend = vi.fn();
    mockStereoMode = "mono";
  });

  it("hide button sends setEndpointVisible(false)", () => {
    // Hide button only renders when NOT autoApp and NOT app descriptor
    const { getByRole } = renderLabel({}, makeChannel({ autoApp: false }));
    const hideBtn = getByRole("button", { name: /hide channel/i });
    fireEvent.click(hideBtn);
    const calls = (mockSend.mock.calls as MockSendCall[]).filter(
      ([cmd]) => cmd.type === "setEndpointVisible",
    );
    expect(calls.length).toBe(1);
    const cmd = calls[0][0] as Extract<Command, { type: "setEndpointVisible" }>;
    expect(cmd.visible).toBe(false);
    expect(cmd.endpoint).toEqual({ channel: "ch1" });
  });
});

describe("ChannelLabel — disable button", () => {
  beforeEach(() => {
    mockSend = vi.fn();
    mockStereoMode = "mono";
  });

  it("disable button sends setEndpointDisabled(true) when enabled", () => {
    const { getByRole } = renderLabel({ disabled: false }, makeChannel({ autoApp: false }));
    const disableBtn = getByRole("button", { name: /disable channel/i });
    fireEvent.click(disableBtn);
    const calls = (mockSend.mock.calls as MockSendCall[]).filter(
      ([cmd]) => cmd.type === "setEndpointDisabled",
    );
    expect(calls.length).toBe(1);
    const cmd = calls[0][0] as Extract<Command, { type: "setEndpointDisabled" }>;
    expect(cmd.disabled).toBe(true);
    expect(cmd.endpoint).toEqual({ channel: "ch1" });
  });

  it("disable button sends setEndpointDisabled(false) when already disabled", () => {
    const { getByRole } = renderLabel({ disabled: true }, makeChannel({ autoApp: false }));
    const enableBtn = getByRole("button", { name: /enable channel/i });
    fireEvent.click(enableBtn);
    const calls = (mockSend.mock.calls as MockSendCall[]).filter(
      ([cmd]) => cmd.type === "setEndpointDisabled",
    );
    expect(calls.length).toBe(1);
    const cmd = calls[0][0] as Extract<Command, { type: "setEndpointDisabled" }>;
    expect(cmd.disabled).toBe(false);
  });

  it("disabled channel renders with dimmed opacity", () => {
    const { container } = renderLabel({ disabled: true });
    const outer = container.firstElementChild as HTMLElement;
    expect(outer.className).toContain("opacity-50");
  });

  it("enabled channel does not have dimmed opacity", () => {
    const { container } = renderLabel({ disabled: false });
    const outer = container.firstElementChild as HTMLElement;
    expect(outer.className).not.toContain("opacity-50");
  });
});

describe("ChannelLabel — master volume slider (mono)", () => {
  beforeEach(() => {
    mockSend = vi.fn();
    mockStereoMode = "mono";
  });

  it("renders master volume slider with correct initial value", () => {
    const { container } = renderLabel({ volume: 0.6 });
    const slider = container.querySelector('input[aria-label="Master volume"]') as HTMLInputElement;
    expect(slider).toBeTruthy();
    expect(parseFloat(slider.value)).toBeCloseTo(0.6, 2);
  });

  it("input event on master volume slider sends setVolume command", () => {
    const { container } = renderLabel({ volume: 0.8 });
    const slider = container.querySelector('input[aria-label="Master volume"]') as HTMLInputElement;
    fireEvent.input(slider, { target: { value: "0.5" } });
    const calls = (mockSend.mock.calls as MockSendCall[]).filter(([cmd]) => cmd.type === "setVolume");
    expect(calls.length).toBeGreaterThanOrEqual(1);
    const cmd = calls[calls.length - 1][0] as Extract<Command, { type: "setVolume" }>;
    expect(cmd.volume).toBeCloseTo(0.5, 5);
    expect(cmd.endpoint).toEqual({ channel: "ch1" });
  });
});

describe("ChannelLabel — master volume slider (stereo)", () => {
  beforeEach(() => {
    mockSend = vi.fn();
    mockStereoMode = "stereo";
  });

  it("renders L and R volume sliders in stereo mode", () => {
    const { container } = renderLabel();
    expect(container.querySelector('input[aria-label="Left volume"]')).toBeTruthy();
    expect(container.querySelector('input[aria-label="Right volume"]')).toBeTruthy();
  });

  it("mono master volume slider is not present in stereo mode", () => {
    const { container } = renderLabel();
    expect(container.querySelector('input[aria-label="Master volume"]')).toBeNull();
  });

  it("L slider input sends setStereoVolume command", () => {
    const { container } = renderLabel();
    const leftSlider = container.querySelector('input[aria-label="Left volume"]') as HTMLInputElement;
    fireEvent.input(leftSlider, { target: { value: "0.3" } });
    const calls = (mockSend.mock.calls as MockSendCall[]).filter(
      ([cmd]) => cmd.type === "setStereoVolume",
    );
    expect(calls.length).toBeGreaterThanOrEqual(1);
  });
});

describe("ChannelLabel — inline rename (custom name channels only)", () => {
  beforeEach(() => {
    mockSend = vi.fn();
    mockStereoMode = "mono";
  });

  it("double-click on a custom-named channel shows rename input", () => {
    // Custom channels have names NOT in the PRESET_CHANNEL_NAMES list
    const { container } = renderLabel({ displayName: "My Channel" });
    const nameSpan = container.querySelector("span.flex-1") as HTMLElement;
    expect(nameSpan).toBeTruthy();
    fireEvent.dblClick(nameSpan);
    const input = container.querySelector('input[type="text"]') as HTMLInputElement;
    expect(input).toBeTruthy();
  });

  it("Enter key in rename input sends renameEndpoint command", () => {
    const { container } = renderLabel({ displayName: "My Channel" });
    const nameSpan = container.querySelector("span.flex-1") as HTMLElement;
    fireEvent.dblClick(nameSpan);
    const input = container.querySelector('input[type="text"]') as HTMLInputElement;
    // Change value then press Enter
    fireEvent.input(input, { target: { value: "Renamed" } });
    fireEvent.keyDown(input, { key: "Enter" });
    const calls = (mockSend.mock.calls as MockSendCall[]).filter(
      ([cmd]) => cmd.type === "renameEndpoint",
    );
    expect(calls.length).toBe(1);
    const cmd = calls[0][0] as Extract<Command, { type: "renameEndpoint" }>;
    expect(cmd.name).toBe("Renamed");
    expect(cmd.endpoint).toEqual({ channel: "ch1" });
  });

  it("Escape in rename input cancels without sending command", () => {
    const { container } = renderLabel({ displayName: "My Channel" });
    const nameSpan = container.querySelector("span.flex-1") as HTMLElement;
    fireEvent.dblClick(nameSpan);
    const input = container.querySelector('input[type="text"]') as HTMLInputElement;
    fireEvent.input(input, { target: { value: "Changed" } });
    fireEvent.keyDown(input, { key: "Escape" });
    expect(mockSend).not.toHaveBeenCalled();
    // Input should be gone
    expect(container.querySelector('input[type="text"]')).toBeNull();
  });

  it("double-click on a preset-named channel does NOT open rename input", () => {
    // "Music" is in PRESET_CHANNEL_NAMES — rename is not available
    const { container } = renderLabel({ displayName: "Music" });
    const nameSpan = container.querySelector("span.flex-1") as HTMLElement;
    fireEvent.dblClick(nameSpan);
    expect(container.querySelector('input[type="text"]')).toBeNull();
  });
});
