/**
 * Tests for ChannelCreator — the "+" button that opens a channel creation dropdown.
 *
 * Mocks: sessionStore (send), graphStore (graph for input devices).
 */
import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, fireEvent } from "@solidjs/testing-library";
import type { Command } from "../types/commands";
import type { MixerSession } from "../types/session";

type MockSendCall = [Command, ...unknown[]];

// ---------------------------------------------------------------------------
// Mocks
// ---------------------------------------------------------------------------

let mockSend: ReturnType<typeof vi.fn>;
let mockSession: MixerSession;
let mockGraphNodes: Record<string, unknown>;
let mockGraphDevices: Record<string, unknown>;

const EMPTY_SESSION: MixerSession = {
  welcomeDismissed: false,
  activeSources: [],
  activeSinks: [],
  endpoints: [],
  links: [],
  persistentNodes: {},
  apps: {},
  devices: {},
  channels: {},
  channelOrder: [],
  mixOrder: [],
  defaultOutputNodeId: null,
};

vi.mock("../stores/sessionStore", () => ({
  useSession: () => ({ state: { session: mockSession }, send: mockSend }),
}));

vi.mock("../stores/graphStore", () => ({
  useGraph: () => ({
    graph: { devices: mockGraphDevices, nodes: mockGraphNodes },
    connected: true,
  }),
}));

import ChannelCreator from "./ChannelCreator";

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

function renderCreator() {
  return render(() => <ChannelCreator />);
}

function makeHardwareNode(id: number, nodeName: string, desc: string) {
  return {
    id,
    identifier: {
      isMonitor: false,
      nodeName,
      nodeNick: null,
      nodeDescription: desc,
      objectPath: null,
    },
    // source port, not a monitor port
    ports: [["p1", "source", false]],
    clientId: null,
  };
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

describe("ChannelCreator — initial state", () => {
  beforeEach(() => {
    mockSend = vi.fn();
    mockSession = { ...EMPTY_SESSION };
    mockGraphNodes = {};
    mockGraphDevices = {};
  });

  it("renders the Create channel button", () => {
    const { getByText } = renderCreator();
    expect(getByText("Create channel")).toBeTruthy();
  });

  it("dropdown is not visible initially", () => {
    const { queryByText } = renderCreator();
    expect(queryByText("Add Empty Channel")).toBeNull();
  });

  it("clicking the button opens the dropdown", () => {
    const { getByText } = renderCreator();
    fireEvent.click(getByText("Create channel"));
    expect(getByText("Add Empty Channel")).toBeTruthy();
  });

  it("button has aria-expanded=false initially", () => {
    const { container } = renderCreator();
    const btn = container.querySelector("button[aria-expanded]") as HTMLButtonElement;
    expect(btn.getAttribute("aria-expanded")).toBe("false");
  });

  it("button has aria-expanded=true after click", () => {
    const { container } = renderCreator();
    const btn = container.querySelector("button[aria-expanded]") as HTMLButtonElement;
    fireEvent.click(btn);
    expect(btn.getAttribute("aria-expanded")).toBe("true");
  });
});

describe("ChannelCreator — channel templates (presets)", () => {
  beforeEach(() => {
    mockSend = vi.fn();
    mockSession = { ...EMPTY_SESSION };
    mockGraphNodes = {};
    mockGraphDevices = {};
  });

  it("shows preset channel templates when dropdown is open", () => {
    const { getByText } = renderCreator();
    fireEvent.click(getByText("Create channel"));
    // All CHANNEL_TEMPLATES names should appear
    expect(getByText("Music")).toBeTruthy();
    expect(getByText("Browser")).toBeTruthy();
    expect(getByText("System")).toBeTruthy();
  });

  it("clicking a preset sends createChannel command", () => {
    const { getByText } = renderCreator();
    fireEvent.click(getByText("Create channel"));
    fireEvent.click(getByText("Music"));
    const calls = (mockSend.mock.calls as MockSendCall[]).filter(([cmd]) => cmd.type === "createChannel");
    expect(calls.length).toBe(1);
    const cmd = calls[0][0] as Extract<Command, { type: "createChannel" }>;
    expect(cmd.name).toBe("Music");
    expect(cmd.kind).toBe("duplex");
  });

  it("dropdown closes after selecting a preset", () => {
    const { getByText, queryByText } = renderCreator();
    fireEvent.click(getByText("Create channel"));
    fireEvent.click(getByText("Music"));
    expect(queryByText("Add Empty Channel")).toBeNull();
  });

  it("already-used preset names are filtered out", () => {
    // If "Music" already exists as a visible endpoint, it should not appear
    mockSession = {
      ...EMPTY_SESSION,
      endpoints: [
        [
          { channel: "ch1" },
          {
            descriptor: { channel: "ch1" },
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
          },
        ],
      ],
    };
    const { getByText, queryByText } = renderCreator();
    fireEvent.click(getByText("Create channel"));
    expect(queryByText("Music")).toBeNull();
    // Other presets still appear
    expect(getByText("Browser")).toBeTruthy();
  });
});

describe("ChannelCreator — custom name input", () => {
  beforeEach(() => {
    mockSend = vi.fn();
    mockSession = { ...EMPTY_SESSION };
    mockGraphNodes = {};
    mockGraphDevices = {};
  });

  it("renders the custom name input when dropdown is open", () => {
    const { container, getByText } = renderCreator();
    fireEvent.click(getByText("Create channel"));
    const input = container.querySelector('input[placeholder="Custom name..."]');
    expect(input).toBeTruthy();
  });

  it("typing and pressing Enter creates channel with custom name", () => {
    const { container, getByText } = renderCreator();
    fireEvent.click(getByText("Create channel"));
    const input = container.querySelector('input[placeholder="Custom name..."]') as HTMLInputElement;
    fireEvent.input(input, { target: { value: "My Custom Channel" } });
    fireEvent.keyDown(input, { key: "Enter" });
    const calls = (mockSend.mock.calls as MockSendCall[]).filter(([cmd]) => cmd.type === "createChannel");
    expect(calls.length).toBe(1);
    const cmd = calls[0][0] as Extract<Command, { type: "createChannel" }>;
    expect(cmd.name).toBe("My Custom Channel");
    expect(cmd.kind).toBe("duplex");
  });

  it("clicking Add button creates channel with custom name", () => {
    const { container, getByText } = renderCreator();
    fireEvent.click(getByText("Create channel"));
    const input = container.querySelector('input[placeholder="Custom name..."]') as HTMLInputElement;
    fireEvent.input(input, { target: { value: "Studio Bus" } });
    fireEvent.click(getByText("Add"));
    const calls = (mockSend.mock.calls as MockSendCall[]).filter(([cmd]) => cmd.type === "createChannel");
    expect(calls.length).toBe(1);
    expect((calls[0][0] as Extract<Command, { type: "createChannel" }>).name).toBe("Studio Bus");
  });

  it("Add button is disabled when custom name is empty", () => {
    const { container, getByText } = renderCreator();
    fireEvent.click(getByText("Create channel"));
    const addBtn = getByText("Add") as HTMLButtonElement;
    expect(addBtn.disabled).toBe(true);
  });
});

describe("ChannelCreator — hardware input devices", () => {
  beforeEach(() => {
    mockSend = vi.fn();
    mockSession = { ...EMPTY_SESSION };
    mockGraphNodes = {
      "10": makeHardwareNode(10, "alsa_input.usb-Mic", "USB Microphone"),
    };
    mockGraphDevices = {
      "dev1": { name: "USB Microphone", nodes: [10] },
    };
  });

  it("shows Input Devices section when hardware inputs are present", () => {
    const { getByText } = renderCreator();
    fireEvent.click(getByText("Create channel"));
    expect(getByText("Input Devices")).toBeTruthy();
  });

  it("shows the hardware input device name", () => {
    const { getByText } = renderCreator();
    fireEvent.click(getByText("Create channel"));
    expect(getByText("USB Microphone")).toBeTruthy();
  });

  it("clicking a hardware device sends createChannel with kind=source", () => {
    const { getByText } = renderCreator();
    fireEvent.click(getByText("Create channel"));
    fireEvent.click(getByText("USB Microphone"));
    const calls = (mockSend.mock.calls as MockSendCall[]).filter(([cmd]) => cmd.type === "createChannel");
    expect(calls.length).toBe(1);
    const cmd = calls[0][0] as Extract<Command, { type: "createChannel" }>;
    expect(cmd.kind).toBe("source");
  });
});

describe("ChannelCreator — search filter", () => {
  beforeEach(() => {
    mockSend = vi.fn();
    mockSession = { ...EMPTY_SESSION };
    mockGraphNodes = {};
    mockGraphDevices = {};
  });

  it("typing in search filters preset list", () => {
    const { container, getByText, queryByText } = renderCreator();
    fireEvent.click(getByText("Create channel"));
    const searchInput = container.querySelector(
      'input[placeholder="Search devices, apps, presets..."]',
    ) as HTMLInputElement;
    fireEvent.input(searchInput, { target: { value: "music" } });
    expect(getByText("Music")).toBeTruthy();
    expect(queryByText("Browser")).toBeNull();
  });

  it("shows empty state when search has no results", () => {
    const { container, getByText } = renderCreator();
    fireEvent.click(getByText("Create channel"));
    const searchInput = container.querySelector(
      'input[placeholder="Search devices, apps, presets..."]',
    ) as HTMLInputElement;
    fireEvent.input(searchInput, { target: { value: "zzznoresults" } });
    expect(getByText(/No results for/i)).toBeTruthy();
  });
});
