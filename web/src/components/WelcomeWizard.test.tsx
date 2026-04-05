import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, fireEvent } from "@solidjs/testing-library";

// ---------------------------------------------------------------------------
// Mock useSession
// ---------------------------------------------------------------------------

import type { MixerSession } from "../types/session";
import type { Command } from "../types/commands";

type MockSendCall = [Command, ...unknown[]];


interface MockSessionState {
  session: MixerSession;
  connected: boolean;
  reconnecting: boolean;
  reconnectAttempt: number;
}

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
  canUndo: false,
  canRedo: false,
};

// Mutable test state shared by helpers
let mockState: MockSessionState;
let mockSend: ReturnType<typeof vi.fn>;

vi.mock("../stores/sessionStore", () => ({
  useSession: () => ({ state: mockState, send: mockSend }),
}));

function makeSession(overrides: Partial<MixerSession> = {}): MixerSession {
  return { ...EMPTY_SESSION, ...overrides };
}

function makeState(sessionOverrides: Partial<MixerSession> = {}): MockSessionState {
  return {
    session: makeSession(sessionOverrides),
    connected: true,
    reconnecting: false,
    reconnectAttempt: 0,
  };
}

// ---------------------------------------------------------------------------
// Import component AFTER mock is registered
// ---------------------------------------------------------------------------

import WelcomeWizard from "./WelcomeWizard";

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

function renderWizard(onDone = vi.fn()) {
  return render(() => <WelcomeWizard onDone={onDone} />);
}

// ---------------------------------------------------------------------------
// Visibility logic (controlled externally by Mixer — wizard renders when shown)
// ---------------------------------------------------------------------------

describe("WelcomeWizard visibility", () => {
  beforeEach(() => {
    mockSend = vi.fn();
  });

  it("renders the welcome heading when mounted", () => {
    mockState = makeState();
    const { getByText } = renderWizard();
    expect(getByText("Welcome to OpenSoundGrid")).toBeTruthy();
  });

  it("has role=dialog and aria-modal", () => {
    mockState = makeState();
    const { container } = renderWizard();
    const dialog = container.querySelector('[role="dialog"]');
    expect(dialog).toBeTruthy();
    expect(dialog?.getAttribute("aria-modal")).toBe("true");
  });
});

// ---------------------------------------------------------------------------
// App grouping
// ---------------------------------------------------------------------------

describe("WelcomeWizard app grouping", () => {
  beforeEach(() => {
    mockSend = vi.fn();
    mockState = makeState({
      apps: {
        ff: {
          id: "ff",
          kind: "source",
          name: "Firefox",
          binary: "firefox",
          iconName: "",
          exceptions: [],
        },
        sp: {
          id: "sp",
          kind: "source",
          name: "Spotify",
          binary: "spotify",
          iconName: "",
          exceptions: [],
        },
        dc: {
          id: "dc",
          kind: "source",
          name: "Discord",
          binary: "discord",
          iconName: "",
          exceptions: [],
        },
        st: {
          id: "st",
          kind: "source",
          name: "Steam",
          binary: "steam",
          iconName: "",
          exceptions: [],
        },
        un: {
          id: "un",
          kind: "source",
          name: "CustomApp",
          binary: "customapp",
          iconName: "",
          exceptions: [],
        },
      },
    });
  });

  it("shows Browsers section for firefox", () => {
    const { getByText } = renderWizard();
    expect(getByText("Browsers")).toBeTruthy();
  });

  it("shows Music section for spotify", () => {
    const { getByText } = renderWizard();
    expect(getByText("Music")).toBeTruthy();
  });

  it("shows Communication section for discord", () => {
    const { getByText } = renderWizard();
    expect(getByText("Communication")).toBeTruthy();
  });

  it("shows Games section for steam", () => {
    const { getByText } = renderWizard();
    expect(getByText("Games")).toBeTruthy();
  });

  it("shows Other section for unrecognized app", () => {
    const { getByText } = renderWizard();
    expect(getByText("Other")).toBeTruthy();
  });

  it("all app checkboxes are checked by default", () => {
    const { container } = renderWizard();
    const checkboxes = container.querySelectorAll<HTMLInputElement>('input[type="checkbox"]');
    expect(checkboxes.length).toBeGreaterThan(0);
    for (const cb of Array.from(checkboxes)) {
      expect(cb.checked).toBe(true);
    }
  });
});

// ---------------------------------------------------------------------------
// Create Mixer button
// ---------------------------------------------------------------------------

describe("WelcomeWizard — Create Mixer", () => {
  beforeEach(() => {
    mockSend = vi.fn();
    mockState = makeState({
      apps: {
        ff: {
          id: "ff",
          kind: "source",
          name: "Firefox",
          binary: "firefox",
          iconName: "",
          exceptions: [],
        },
        sp: {
          id: "sp",
          kind: "source",
          name: "Spotify",
          binary: "spotify",
          iconName: "",
          exceptions: [],
        },
      },
    });
  });

  it("sends createChannel for each checked app", () => {
    const onDone = vi.fn();
    const { container } = render(() => <WelcomeWizard onDone={onDone} />);

    const createBtn = container.querySelector<HTMLButtonElement>(
      'button[aria-label*="Create mixer"]',
    );
    expect(createBtn).toBeTruthy();
    fireEvent.click(createBtn!);

    const createChannelCalls = (mockSend.mock.calls as MockSendCall[]).filter(
      ([cmd]) => cmd.type === "createChannel",
    );
    expect(createChannelCalls.length).toBe(2);
    const names = createChannelCalls.map(([cmd]) =>
      cmd.type === "createChannel" ? cmd.name : "",
    );
    expect(names).toContain("Firefox");
    expect(names).toContain("Spotify");
  });

  it("sends dismissWelcome command on Create Mixer", () => {
    const onDone = vi.fn();
    const { container } = render(() => <WelcomeWizard onDone={onDone} />);

    const createBtn = container.querySelector<HTMLButtonElement>(
      'button[aria-label*="Create mixer"]',
    );
    fireEvent.click(createBtn!);

    const dismissCalls = (mockSend.mock.calls as MockSendCall[]).filter(
      ([cmd]) => cmd.type === "dismissWelcome",
    );
    expect(dismissCalls.length).toBe(1);
  });

  it("calls onDone after Create Mixer", () => {
    const onDone = vi.fn();
    const { container } = render(() => <WelcomeWizard onDone={onDone} />);

    const createBtn = container.querySelector<HTMLButtonElement>(
      'button[aria-label*="Create mixer"]',
    );
    fireEvent.click(createBtn!);
    expect(onDone).toHaveBeenCalledOnce();
  });

  it("unchecked apps are not sent as channels", () => {
    const onDone = vi.fn();
    const { container } = render(() => <WelcomeWizard onDone={onDone} />);

    // Uncheck Spotify
    const checkboxes = container.querySelectorAll<HTMLInputElement>('input[type="checkbox"]');
    const spotifyCb = Array.from(checkboxes).find((cb) =>
      cb.getAttribute("aria-label")?.includes("Spotify"),
    );
    expect(spotifyCb).toBeTruthy();
    fireEvent.click(spotifyCb!);

    const createBtn = container.querySelector<HTMLButtonElement>(
      'button[aria-label*="Create mixer"]',
    );
    fireEvent.click(createBtn!);

    const createChannelCalls = (mockSend.mock.calls as MockSendCall[]).filter(
      ([cmd]) => cmd.type === "createChannel",
    );
    const names = createChannelCalls.map(([cmd]) =>
      cmd.type === "createChannel" ? cmd.name : "",
    );
    expect(names).toContain("Firefox");
    expect(names).not.toContain("Spotify");
  });
});

// ---------------------------------------------------------------------------
// Skip button
// ---------------------------------------------------------------------------

describe("WelcomeWizard — Skip", () => {
  beforeEach(() => {
    mockSend = vi.fn();
    mockState = makeState();
  });

  it("sends dismissWelcome on Skip", () => {
    const onDone = vi.fn();
    const { getByText } = render(() => <WelcomeWizard onDone={onDone} />);

    fireEvent.click(getByText("Skip"));

    const dismissCalls = (mockSend.mock.calls as MockSendCall[]).filter(
      ([cmd]) => cmd.type === "dismissWelcome",
    );
    expect(dismissCalls.length).toBe(1);
  });

  it("calls onDone after Skip", () => {
    const onDone = vi.fn();
    const { getByText } = render(() => <WelcomeWizard onDone={onDone} />);

    fireEvent.click(getByText("Skip"));
    expect(onDone).toHaveBeenCalledOnce();
  });

  it("sends dismissWelcome when X button is clicked", () => {
    const onDone = vi.fn();
    const { container } = render(() => <WelcomeWizard onDone={onDone} />);

    const closeBtn = container.querySelector<HTMLButtonElement>('button[aria-label="Skip setup"]');
    expect(closeBtn).toBeTruthy();
    fireEvent.click(closeBtn!);

    const dismissCalls = (mockSend.mock.calls as MockSendCall[]).filter(
      ([cmd]) => cmd.type === "dismissWelcome",
    );
    expect(dismissCalls.length).toBe(1);
  });
});

// ---------------------------------------------------------------------------
// Accessibility
// ---------------------------------------------------------------------------

describe("WelcomeWizard accessibility", () => {
  beforeEach(() => {
    mockSend = vi.fn();
    mockState = makeState();
  });

  it("has aria-labelledby pointing to the heading", () => {
    const { container } = renderWizard();
    const dialog = container.querySelector('[role="dialog"]');
    const labelId = dialog?.getAttribute("aria-labelledby");
    expect(labelId).toBeTruthy();
    const heading = document.getElementById(labelId!);
    expect(heading?.textContent).toContain("Welcome to OpenSoundGrid");
  });
});
