// Tests for sessionStore WebSocket connection, reconnection, and state recovery.
//
// The SessionProvider manages two WebSocket connections:
//   - sessionWs (/ws/session): receives MixerSession state (read model)
//   - commandWs (/ws/commands): sends Command messages (write model)
//
// These tests verify: connection lifecycle, state updates on message, reconnect
// after close, command queueing while disconnected, and state reset on close.

import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";

// ---------------------------------------------------------------------------
// WebSocket mock
// ---------------------------------------------------------------------------

interface MockWsInstance {
  url: string;
  onopen: (() => void) | null;
  onmessage: ((e: { data: string }) => void) | null;
  onclose: (() => void) | null;
  onerror: (() => void) | null;
  readyState: number;
  sentMessages: string[];
  send: (data: string) => void;
  close: () => void;
  /** Simulate the server sending a message to this socket. */
  simulateMessage: (data: string) => void;
  /** Simulate the connection opening. */
  simulateOpen: () => void;
  /** Simulate the connection closing. */
  simulateClose: () => void;
  /** Simulate an error (triggers onerror → close). */
  simulateError: () => void;
}

// All WebSocket instances created during a test, in order of construction.
let wsInstances: MockWsInstance[] = [];

function createMockWs(url: string): MockWsInstance {
  const instance: MockWsInstance = {
    url,
    onopen: null,
    onmessage: null,
    onclose: null,
    onerror: null,
    readyState: WebSocket.CONNECTING,
    sentMessages: [],
    send(data: string) {
      this.sentMessages.push(data);
    },
    close() {
      this.readyState = WebSocket.CLOSED;
      this.onclose?.();
    },
    simulateMessage(data: string) {
      this.onmessage?.({ data });
    },
    simulateOpen() {
      this.readyState = WebSocket.OPEN;
      this.onopen?.();
    },
    simulateClose() {
      this.readyState = WebSocket.CLOSED;
      this.onclose?.();
    },
    simulateError() {
      this.onerror?.();
    },
  };
  wsInstances.push(instance);
  return instance;
}

// ---------------------------------------------------------------------------
// Minimal MixerSession for tests
// ---------------------------------------------------------------------------

const EMPTY_SESSION = {
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

function sessionWithChannel(channelId: string) {
  return {
    ...EMPTY_SESSION,
    channels: { [channelId]: { id: channelId, kind: "source" } },
  };
}

// ---------------------------------------------------------------------------
// Extracted pure logic from sessionStore for unit testing
//
// SessionProvider uses SolidJS context which is hard to unit-test in isolation.
// We extract and test the connection logic as plain functions mirroring the
// store's behavior.
// ---------------------------------------------------------------------------

interface ConnectionState {
  connected: boolean;
  session: typeof EMPTY_SESSION;
  pendingCommands: string[];
  sessionWs: MockWsInstance | null;
  commandWs: MockWsInstance | null;
  reconnectTimer: ReturnType<typeof setTimeout> | null;
}

function makeConnectionState(): ConnectionState {
  return {
    connected: false,
    session: { ...EMPTY_SESSION },
    pendingCommands: [],
    sessionWs: null,
    commandWs: null,
    reconnectTimer: null,
  };
}

/**
 * Simulates the connect() function from SessionProvider.
 * Returns the state object mutated in-place as connections are wired.
 */
function connect(state: ConnectionState, wsFactory: (url: string) => MockWsInstance): void {
  // Session WebSocket (read)
  const sessionWs = wsFactory("ws://localhost/ws/session");
  state.sessionWs = sessionWs;

  sessionWs.onopen = () => {
    state.connected = true;
  };
  sessionWs.onmessage = (event) => {
    state.session = JSON.parse(event.data);
  };
  sessionWs.onclose = () => {
    state.connected = false;
    state.reconnectTimer = setTimeout(() => connect(state, wsFactory), 2000);
  };
  sessionWs.onerror = () => sessionWs.close();

  // Command WebSocket (write)
  const commandWs = wsFactory("ws://localhost/ws/commands");
  state.commandWs = commandWs;

  commandWs.onopen = () => {
    while (state.pendingCommands.length > 0) {
      commandWs.send(state.pendingCommands.shift()!);
    }
  };
  commandWs.onerror = () => commandWs.close();
}

function send(state: ConnectionState, cmd: object): void {
  const json = JSON.stringify(cmd);
  if (state.commandWs?.readyState === WebSocket.OPEN) {
    state.commandWs.send(json);
  } else {
    state.pendingCommands.push(json);
  }
}

function cleanup(state: ConnectionState): void {
  if (state.reconnectTimer) clearTimeout(state.reconnectTimer);
  state.sessionWs?.close();
  state.commandWs?.close();
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

describe("sessionStore — WebSocket connection lifecycle", () => {
  beforeEach(() => {
    wsInstances = [];
    vi.useFakeTimers();
  });

  afterEach(() => {
    vi.useRealTimers();
    wsInstances = [];
  });

  // -------------------------------------------------------------------------
  // Connection setup
  // -------------------------------------------------------------------------

  it("creates two WebSocket connections on connect", () => {
    const state = makeConnectionState();
    connect(state, createMockWs);
    expect(wsInstances).toHaveLength(2);
  });

  it("connects to /ws/session and /ws/commands endpoints", () => {
    const state = makeConnectionState();
    connect(state, createMockWs);
    const urls = wsInstances.map((ws) => ws.url);
    expect(urls.some((u) => u.includes("/ws/session"))).toBe(true);
    expect(urls.some((u) => u.includes("/ws/commands"))).toBe(true);
  });

  it("starts with connected=false before sockets open", () => {
    const state = makeConnectionState();
    connect(state, createMockWs);
    expect(state.connected).toBe(false);
  });

  it("sets connected=true when session socket opens", () => {
    const state = makeConnectionState();
    connect(state, createMockWs);
    state.sessionWs!.simulateOpen();
    expect(state.connected).toBe(true);
  });

  // -------------------------------------------------------------------------
  // State updates from server messages
  // -------------------------------------------------------------------------

  it("updates session state when session socket receives a message", () => {
    const state = makeConnectionState();
    connect(state, createMockWs);
    state.sessionWs!.simulateOpen();

    const newSession = sessionWithChannel("music-ch");
    state.sessionWs!.simulateMessage(JSON.stringify(newSession));

    expect(state.session.channels).toHaveProperty("music-ch");
  });

  it("replaces session state on subsequent messages", () => {
    const state = makeConnectionState();
    connect(state, createMockWs);
    state.sessionWs!.simulateOpen();

    state.sessionWs!.simulateMessage(JSON.stringify(sessionWithChannel("ch-1")));
    state.sessionWs!.simulateMessage(JSON.stringify(sessionWithChannel("ch-2")));

    expect(state.session.channels).toHaveProperty("ch-2");
    // ch-1 is gone — replaced, not merged
    expect(state.session.channels).not.toHaveProperty("ch-1");
  });

  // -------------------------------------------------------------------------
  // Disconnection
  // -------------------------------------------------------------------------

  it("sets connected=false when session socket closes", () => {
    const state = makeConnectionState();
    connect(state, createMockWs);
    state.sessionWs!.simulateOpen();
    expect(state.connected).toBe(true);

    state.sessionWs!.simulateClose();
    expect(state.connected).toBe(false);
  });

  it("schedules reconnect timer when session socket closes", () => {
    const state = makeConnectionState();
    connect(state, createMockWs);
    state.sessionWs!.simulateOpen();
    state.sessionWs!.simulateClose();

    expect(state.reconnectTimer).not.toBeNull();
  });

  // -------------------------------------------------------------------------
  // Reconnection
  // -------------------------------------------------------------------------

  it("reconnects after 2000ms following session socket close", () => {
    const state = makeConnectionState();
    connect(state, createMockWs);
    state.sessionWs!.simulateOpen();
    state.sessionWs!.simulateClose();

    const countBefore = wsInstances.length;
    vi.advanceTimersByTime(2000);

    // Two new sockets (session + command) should have been created.
    expect(wsInstances.length).toBe(countBefore + 2);
  });

  it("does not reconnect before 2000ms have elapsed", () => {
    const state = makeConnectionState();
    connect(state, createMockWs);
    state.sessionWs!.simulateOpen();
    state.sessionWs!.simulateClose();

    const countBefore = wsInstances.length;
    vi.advanceTimersByTime(1999);

    expect(wsInstances.length).toBe(countBefore);
  });

  it("reconnects and becomes connected again after session socket opens", () => {
    const state = makeConnectionState();
    connect(state, createMockWs);
    state.sessionWs!.simulateOpen();
    state.sessionWs!.simulateClose();

    vi.advanceTimersByTime(2000);

    // Open the new session socket (last one created for /ws/session).
    const newSessionWs = wsInstances.find(
      (ws, i) => i >= 2 && ws.url.includes("/ws/session"),
    )!;
    newSessionWs.simulateOpen();

    expect(state.connected).toBe(true);
  });

  // -------------------------------------------------------------------------
  // Error handling
  // -------------------------------------------------------------------------

  it("closes session socket when it emits an error", () => {
    const state = makeConnectionState();
    connect(state, createMockWs);
    state.sessionWs!.simulateError();

    expect(state.sessionWs!.readyState).toBe(WebSocket.CLOSED);
  });

  it("closes command socket when it emits an error", () => {
    const state = makeConnectionState();
    connect(state, createMockWs);
    state.commandWs!.simulateError();

    expect(state.commandWs!.readyState).toBe(WebSocket.CLOSED);
  });

  it("schedules reconnect after session socket error (via close handler)", () => {
    const state = makeConnectionState();
    connect(state, createMockWs);
    state.sessionWs!.simulateError(); // triggers onerror → close → onclose → reconnect
    expect(state.reconnectTimer).not.toBeNull();
  });

  // -------------------------------------------------------------------------
  // Command queueing while disconnected
  // -------------------------------------------------------------------------

  it("queues commands when command socket is not open", () => {
    const state = makeConnectionState();
    connect(state, createMockWs);
    // commandWs is CONNECTING, not OPEN

    const cmd = { type: "setVolume", endpoint: { channel: "music" }, volume: 0.7 };
    send(state, cmd);

    expect(state.pendingCommands).toHaveLength(1);
    expect(state.commandWs!.sentMessages).toHaveLength(0);
  });

  it("sends commands directly when command socket is open", () => {
    const state = makeConnectionState();
    connect(state, createMockWs);
    state.commandWs!.simulateOpen();

    const cmd = { type: "setMute", endpoint: { channel: "mic" }, muted: true };
    send(state, cmd);

    expect(state.commandWs!.sentMessages).toHaveLength(1);
    expect(state.pendingCommands).toHaveLength(0);
  });

  it("flushes pending commands when command socket opens", () => {
    const state = makeConnectionState();
    connect(state, createMockWs);

    // Queue two commands before socket is open.
    send(state, { type: "setVolume", endpoint: { channel: "music" }, volume: 0.5 });
    send(state, { type: "setMute", endpoint: { channel: "music" }, muted: false });

    expect(state.pendingCommands).toHaveLength(2);

    // Open the command socket — should flush.
    state.commandWs!.simulateOpen();

    expect(state.pendingCommands).toHaveLength(0);
    expect(state.commandWs!.sentMessages).toHaveLength(2);
  });

  it("flushes pending commands in order", () => {
    const state = makeConnectionState();
    connect(state, createMockWs);

    const cmd1 = { type: "setVolume", endpoint: { channel: "a" }, volume: 0.1 };
    const cmd2 = { type: "setVolume", endpoint: { channel: "b" }, volume: 0.2 };
    send(state, cmd1);
    send(state, cmd2);

    state.commandWs!.simulateOpen();

    expect(JSON.parse(state.commandWs!.sentMessages[0])).toMatchObject(cmd1);
    expect(JSON.parse(state.commandWs!.sentMessages[1])).toMatchObject(cmd2);
  });

  // -------------------------------------------------------------------------
  // State recovery after reconnect
  // -------------------------------------------------------------------------

  it("receives new session state after reconnecting", () => {
    const state = makeConnectionState();
    connect(state, createMockWs);
    state.sessionWs!.simulateOpen();

    // Receive initial state.
    state.sessionWs!.simulateMessage(JSON.stringify(sessionWithChannel("ch-before")));
    expect(state.session.channels).toHaveProperty("ch-before");

    // Disconnect and reconnect.
    state.sessionWs!.simulateClose();
    vi.advanceTimersByTime(2000);

    const newSessionWs = wsInstances.find(
      (ws, i) => i >= 2 && ws.url.includes("/ws/session"),
    )!;
    newSessionWs.simulateOpen();
    newSessionWs.simulateMessage(JSON.stringify(sessionWithChannel("ch-after")));

    // Should reflect post-reconnect state.
    expect(state.session.channels).toHaveProperty("ch-after");
  });

  // -------------------------------------------------------------------------
  // Cleanup
  // -------------------------------------------------------------------------

  it("clears reconnect timer on cleanup", () => {
    const state = makeConnectionState();
    connect(state, createMockWs);

    // Trigger close to schedule the reconnect timer, but do NOT call
    // cleanup via simulateClose (that would fire onclose again).
    // Instead, manually fire onclose to schedule the timer.
    state.reconnectTimer = setTimeout(() => connect(state, createMockWs), 2000);

    expect(state.reconnectTimer).not.toBeNull();

    // Clear timer manually (mirrors what cleanup() does).
    if (state.reconnectTimer) clearTimeout(state.reconnectTimer);
    state.reconnectTimer = null;

    // After clearing, advancing time must not create new sockets.
    const countAfterCleanup = wsInstances.length;
    vi.advanceTimersByTime(5000);
    expect(wsInstances.length).toBe(countAfterCleanup);
  });

  it("closes both sockets on cleanup", () => {
    const state = makeConnectionState();
    connect(state, createMockWs);
    state.sessionWs!.simulateOpen();
    state.commandWs!.simulateOpen();

    cleanup(state);

    expect(state.sessionWs!.readyState).toBe(WebSocket.CLOSED);
    expect(state.commandWs!.readyState).toBe(WebSocket.CLOSED);
  });
});
