/**
 * Tests for Mixer — the top-level mixer grid component.
 *
 * Strategy: mock all child components and all stores so we test only
 * the Mixer orchestration logic (what renders under which conditions,
 * keyboard handlers, undo/redo).
 *
 * Mocks: sessionStore, graphStore, levelsStore, mixerSettings, monitorStore,
 *        useMixerViewModel, useMixOutputs, useKeyboardNav, all heavy child components.
 */
import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import { render, fireEvent } from "@solidjs/testing-library";
import { For } from "solid-js";
import type { JSX } from "solid-js";
import type { MixerSession } from "../types/session";

// ---------------------------------------------------------------------------
// Shared mutable mock state
// ---------------------------------------------------------------------------

let mockSend: ReturnType<typeof vi.fn>;
let mockSession: MixerSession;
let mockConnected: boolean;
let mockGraphConnected: boolean;
let mockReconnecting: boolean;
let mockReconnectAttempt: number;
let mockNextRetryMs: number;

const EMPTY_SESSION: MixerSession = {
  welcomeDismissed: false,
  lastPresetName: null,
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
  canUndo: false,
  canRedo: false,
};

// ---------------------------------------------------------------------------
// Store mocks
// ---------------------------------------------------------------------------

vi.mock("../stores/sessionStore", () => ({
  useSession: () => ({
    state: {
      session: mockSession,
      connected: mockConnected,
      reconnecting: mockReconnecting,
      reconnectAttempt: mockReconnectAttempt,
      nextRetryMs: mockNextRetryMs,
    },
    send: mockSend,
  }),
}));

vi.mock("../stores/graphStore", () => ({
  useGraph: () => ({
    graph: { devices: {}, nodes: {}, groupNodes: {}, clients: {}, ports: {}, links: {}, defaultSinkName: null, defaultSourceName: null },
    connected: mockGraphConnected,
  }),
}));

vi.mock("../stores/levelsStore", () => ({
  useLevels: () => ({ peaks: {}, connected: false }),
}));

vi.mock("../stores/mixerSettings", () => ({
  useMixerSettings: () => ({
    settings: { stereoMode: "mono", theme: "dark" },
    setStereoMode: vi.fn(),
    setTheme: vi.fn(),
  }),
}));

vi.mock("../stores/monitorStore", () => ({
  useMonitor: () => ({
    state: { monitoredCell: null },
    startMonitoring: vi.fn(),
    stopMonitoring: vi.fn(),
    isCellMonitored: vi.fn(() => false),
  }),
}));

// ---------------------------------------------------------------------------
// Hook mocks
// ---------------------------------------------------------------------------

vi.mock("../hooks/useMixerViewModel", () => ({
  useMixerViewModel: () => ({
    channels: () => [],
    hiddenChannels: () => [],
    mixes: () => [],
    getPeaks: () => ({ left: 0, right: 0 }),
    descKey: (d: unknown) => JSON.stringify(d),
    persistChannelOrder: vi.fn(),
    persistMixOrder: vi.fn(),
  }),
  getMixColor: (_name: string) => "#5090e0",
  findEndpoint: () => undefined,
  findLink: () => null,
}));

vi.mock("./useMixOutputs", () => ({
  useMixOutputs: () => ({
    mixOutputs: {},
    setMixOutput: vi.fn(),
    usedDeviceIds: () => new Set(),
  }),
}));

vi.mock("./useKeyboardNav", () => ({
  useKeyboardNav: () => ({
    focusedCell: () => null,
    setFocusedCell: vi.fn(),
    registerCellActions: vi.fn(),
    handleGridKeyDown: vi.fn(),
  }),
}));

// ---------------------------------------------------------------------------
// Heavy child component mocks
// ---------------------------------------------------------------------------

vi.mock("./MixHeader", () => ({ default: () => <div data-testid="mix-header" /> }));
vi.mock("./ChannelLabel", () => ({ default: () => <div data-testid="channel-label" /> }));
vi.mock("./CompactMode", () => ({ default: () => <div data-testid="compact-mode" /> }));
vi.mock("./MatrixCell", () => ({ default: () => <div data-testid="matrix-cell" /> }));
vi.mock("./ChannelCreator", () => ({ default: () => <div data-testid="channel-creator" /> }));
vi.mock("./MixCreator", () => ({ default: () => <div data-testid="mix-creator" /> }));
vi.mock("./MixEffectsRow", () => ({ default: () => <div data-testid="mix-effects-row" /> }));
vi.mock("./WelcomeWizard", () => ({ default: (props: { onDone: () => void }) => <div data-testid="welcome-wizard"><button onClick={() => props.onDone()}>Done</button></div> }));
vi.mock("./EmptyState", () => ({ default: (props: { kind: string }) => <div data-testid={`empty-state-${props.kind}`} /> }));
vi.mock("./DragReorder", () => ({
  default: (props: { items: unknown[]; children: (item: unknown, idx: () => number, handle: () => unknown) => unknown }) => (
    <For each={props.items}>
      {(item, i) => props.children(item, i, () => <span />) as JSX.Element}
    </For>
  ),
}));
vi.mock("./SettingsPanel", () => ({ default: () => <div data-testid="settings-panel" /> }));
vi.mock("../eq/EqPage", () => ({ default: () => <div data-testid="eq-page" /> }));
vi.mock("./mixerUtils", () => ({
  getMixColor: () => "#5090e0",
  findEndpoint: () => undefined,
  findLink: () => null,
  descriptorKey: (d: unknown) => JSON.stringify(d),
}));

vi.mock("../hooks/useVolumeDebounce", () => ({
  useVolumeDebounce: (fn: (v: number) => void) => fn,
}));

import Mixer from "./Mixer";

function commandType(args: unknown[]): string | undefined {
  const firstArg = args[0];
  if (!firstArg || typeof firstArg !== "object" || !("type" in firstArg)) return undefined;
  return (firstArg as { type: string }).type;
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

describe("Mixer — basic rendering", () => {
  beforeEach(() => {
    mockSend = vi.fn();
    mockSession = { ...EMPTY_SESSION };
    mockConnected = true;
    mockGraphConnected = true;
    mockReconnecting = false;
    mockReconnectAttempt = 0;
    mockNextRetryMs = 0;
  });

  it("renders the Open Sound Grid heading", () => {
    const { getByText } = render(() => <Mixer />);
    expect(getByText("Open Sound Grid")).toBeTruthy();
  });

  it("renders the ChannelCreator button area", () => {
    const { getByTestId } = render(() => <Mixer />);
    expect(getByTestId("channel-creator")).toBeTruthy();
  });

  it("renders the MixCreator", () => {
    const { getByTestId } = render(() => <Mixer />);
    expect(getByTestId("mix-creator")).toBeTruthy();
  });

  it("renders the settings button in the header", () => {
    const { getByRole } = render(() => <Mixer />);
    expect(getByRole("button", { name: /settings/i })).toBeTruthy();
  });

  it("does not render the dead grid preset dropdown", () => {
    const { queryByRole } = render(() => <Mixer />);
    expect(queryByRole("combobox")).toBeNull();
  });

  it("renders the compact mode toggle in the header", () => {
    const { getByRole } = render(() => <Mixer />);
    expect(getByRole("button", { name: /enable compact mode/i })).toBeTruthy();
  });
});

describe("Mixer — welcome wizard", () => {
  beforeEach(() => {
    mockSend = vi.fn();
    mockConnected = true;
    mockGraphConnected = true;
    mockReconnecting = false;
    mockReconnectAttempt = 0;
    mockNextRetryMs = 0;
  });

  it("shows WelcomeWizard when no channels and welcomeDismissed=false", () => {
    mockSession = { ...EMPTY_SESSION, welcomeDismissed: false, channels: {} };
    const { getByTestId } = render(() => <Mixer />);
    expect(getByTestId("welcome-wizard")).toBeTruthy();
  });

  it("hides WelcomeWizard when welcomeDismissed=true", () => {
    mockSession = { ...EMPTY_SESSION, welcomeDismissed: true, channels: {} };
    const { queryByTestId } = render(() => <Mixer />);
    expect(queryByTestId("welcome-wizard")).toBeNull();
  });

  it("hides WelcomeWizard when channels exist", () => {
    mockSession = {
      ...EMPTY_SESSION,
      welcomeDismissed: false,
      channels: { ch1: { id: "ch1", kind: "duplex", outputNodeId: null, assignedApps: [], autoApp: false, allowAppAssignment: true } },
    };
    const { queryByTestId } = render(() => <Mixer />);
    expect(queryByTestId("welcome-wizard")).toBeNull();
  });

  it("clicking Done in WelcomeWizard dismisses it", () => {
    mockSession = { ...EMPTY_SESSION, welcomeDismissed: false, channels: {} };
    const { getByText, queryByTestId } = render(() => <Mixer />);
    fireEvent.click(getByText("Done"));
    expect(queryByTestId("welcome-wizard")).toBeNull();
  });
});

describe("Mixer — disconnected state", () => {
  beforeEach(() => {
    mockSend = vi.fn();
    mockSession = { ...EMPTY_SESSION };
    mockConnected = false;
    mockGraphConnected = false;
  });

  it("shows disconnected empty state when graph is not connected", () => {
    const { getByTestId } = render(() => <Mixer />);
    expect(getByTestId("empty-state-disconnected")).toBeTruthy();
  });

  it("status bar shows Disconnected text when not connected", () => {
    mockConnected = false;
    const { getByText } = render(() => <Mixer />);
    expect(getByText("Disconnected")).toBeTruthy();
  });
});

describe("Mixer — reconnecting banner", () => {
  beforeEach(() => {
    mockSend = vi.fn();
    mockSession = { ...EMPTY_SESSION };
    mockConnected = false;
    mockGraphConnected = false;
    mockReconnecting = true;
    mockReconnectAttempt = 2;
    mockNextRetryMs = 4000;
  });

  it("shows reconnecting banner with attempt and retry delay", () => {
    const { getByRole } = render(() => <Mixer />);
    const banner = getByRole("status");
    expect(banner.textContent).toContain("attempt 3");
    expect(banner.textContent).toContain("retry in 4s");
  });

  it("hides reconnecting banner when not reconnecting", () => {
    mockReconnecting = false;
    render(() => <Mixer />);
    // The status bar footer also has role="status" implicitly via aria-live,
    // so we check there is no element with the reconnecting text.
    const statusElements = document.querySelectorAll('[role="status"]');
    const reconnectBanner = Array.from(statusElements).find((el) =>
      el.textContent?.includes("Reconnecting"),
    );
    expect(reconnectBanner).toBeUndefined();
  });
});

describe("Mixer — connected status bar", () => {
  beforeEach(() => {
    mockSend = vi.fn();
    mockSession = { ...EMPTY_SESSION };
    mockConnected = true;
    mockGraphConnected = true;
    mockReconnecting = false;
    mockReconnectAttempt = 0;
    mockNextRetryMs = 0;
  });

  it("shows Connected to PipeWire in status bar when connected", () => {
    const { getByText } = render(() => <Mixer />);
    expect(getByText("Connected to PipeWire")).toBeTruthy();
  });

  it("shows channel count in status bar", () => {
    const { getByText } = render(() => <Mixer />);
    expect(getByText("0 channels")).toBeTruthy();
  });

  it("shows the loaded preset name when the session provides one", () => {
    mockSession = { ...EMPTY_SESSION, lastPresetName: "Gaming" };
    const { getByText } = render(() => <Mixer />);
    expect(getByText("Preset: Gaming")).toBeTruthy();
  });
});

describe("Mixer — keyboard shortcuts (undo/redo)", () => {
  beforeEach(() => {
    mockSend = vi.fn();
    mockSession = { ...EMPTY_SESSION };
    mockConnected = true;
    mockGraphConnected = true;
    mockReconnecting = false;
    mockReconnectAttempt = 0;
    mockNextRetryMs = 0;
  });

  afterEach(() => {
    // Clean up event listeners by unmounting
  });

  it("Ctrl+Z sends undo command", () => {
    render(() => <Mixer />);
    fireEvent.keyDown(document, { key: "z", ctrlKey: true });
    const calls = mockSend.mock.calls.filter((args: unknown[]) => commandType(args) === "undo");
    expect(calls.length).toBe(1);
  });

  it("Ctrl+Shift+Z sends redo command", () => {
    render(() => <Mixer />);
    fireEvent.keyDown(document, { key: "z", ctrlKey: true, shiftKey: true });
    const calls = mockSend.mock.calls.filter((args: unknown[]) => commandType(args) === "redo");
    expect(calls.length).toBe(1);
  });

  it("Ctrl+Z inside an input field does NOT send undo command", () => {
    const { container } = render(() => (
      <>
        <Mixer />
        <input data-testid="text-input" type="text" />
      </>
    ));
    const input = container.querySelector('input[data-testid="text-input"]') as HTMLInputElement;
    fireEvent.keyDown(input, { key: "z", ctrlKey: true });
    const calls = mockSend.mock.calls.filter((args: unknown[]) => commandType(args) === "undo");
    expect(calls.length).toBe(0);
  });
});

describe("Mixer — undo/redo toolbar buttons", () => {
  beforeEach(() => {
    mockSend = vi.fn();
    mockSession = { ...EMPTY_SESSION };
    mockConnected = true;
    mockGraphConnected = true;
    mockReconnecting = false;
    mockReconnectAttempt = 0;
    mockNextRetryMs = 0;
  });

  it("renders undo and redo buttons in the toolbar", () => {
    const { getByRole } = render(() => <Mixer />);
    expect(getByRole("button", { name: /undo/i })).toBeTruthy();
    expect(getByRole("button", { name: /redo/i })).toBeTruthy();
  });

  it("undo button is disabled when canUndo is false", () => {
    mockSession = { ...EMPTY_SESSION, canUndo: false };
    const { getByRole } = render(() => <Mixer />);
    const btn = getByRole("button", { name: /^undo$/i }) as HTMLButtonElement;
    expect(btn.disabled).toBe(true);
  });

  it("undo button is enabled when canUndo is true", () => {
    mockSession = { ...EMPTY_SESSION, canUndo: true };
    const { getByRole } = render(() => <Mixer />);
    const btn = getByRole("button", { name: /^undo$/i }) as HTMLButtonElement;
    expect(btn.disabled).toBe(false);
  });

  it("redo button is disabled when canRedo is false", () => {
    mockSession = { ...EMPTY_SESSION, canRedo: false };
    const { getByRole } = render(() => <Mixer />);
    const btn = getByRole("button", { name: /^redo$/i }) as HTMLButtonElement;
    expect(btn.disabled).toBe(true);
  });

  it("redo button is enabled when canRedo is true", () => {
    mockSession = { ...EMPTY_SESSION, canRedo: true };
    const { getByRole } = render(() => <Mixer />);
    const btn = getByRole("button", { name: /^redo$/i }) as HTMLButtonElement;
    expect(btn.disabled).toBe(false);
  });

  it("clicking undo button sends undo command", () => {
    mockSession = { ...EMPTY_SESSION, canUndo: true };
    const { getByRole } = render(() => <Mixer />);
    fireEvent.click(getByRole("button", { name: /^undo$/i }));
    const calls = mockSend.mock.calls.filter((args: unknown[]) => commandType(args) === "undo");
    expect(calls.length).toBe(1);
  });

  it("clicking redo button sends redo command", () => {
    mockSession = { ...EMPTY_SESSION, canRedo: true };
    const { getByRole } = render(() => <Mixer />);
    fireEvent.click(getByRole("button", { name: /^redo$/i }));
    const calls = mockSend.mock.calls.filter((args: unknown[]) => commandType(args) === "redo");
    expect(calls.length).toBe(1);
  });
});

describe("Mixer — grid structure", () => {
  beforeEach(() => {
    mockSend = vi.fn();
    mockSession = { ...EMPTY_SESSION };
    mockConnected = true;
    mockGraphConnected = true;
    mockReconnecting = false;
    mockReconnectAttempt = 0;
    mockNextRetryMs = 0;
  });

  it("renders a grid with role=grid and aria-label", () => {
    const { container } = render(() => <Mixer />);
    const grid = container.querySelector('[role="grid"][aria-label="Mixer matrix"]');
    expect(grid).toBeTruthy();
  });

  it("renders header row with role=row", () => {
    const { container } = render(() => <Mixer />);
    const rows = container.querySelectorAll('[role="row"]');
    expect(rows.length).toBeGreaterThanOrEqual(1);
  });

  it("switches to compact mode when the toolbar toggle is clicked", () => {
    const { getByRole, getByTestId, queryByRole } = render(() => <Mixer />);
    fireEvent.click(getByRole("button", { name: /enable compact mode/i }));
    expect(getByTestId("compact-mode")).toBeTruthy();
    expect(queryByRole("grid", { name: /mixer matrix/i })).toBeNull();
  });
});
