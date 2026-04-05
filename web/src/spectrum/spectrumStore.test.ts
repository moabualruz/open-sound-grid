/**
 * spectrumStore tests.
 *
 * Tests WebSocket subscribe/unsubscribe messaging, bin updates,
 * auto-disconnect when no subscribers, and reconnect on disconnect.
 */
import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";

// ---------------------------------------------------------------------------
// Mock WebSocket
// ---------------------------------------------------------------------------

class MockWebSocket {
  static OPEN = 1;
  static CLOSING = 2;
  static CLOSED = 3;

  readyState: number = 1; // OPEN by default
  url: string;

  onopen: ((e: Event) => void) | null = null;
  onmessage: ((e: MessageEvent) => void) | null = null;
  onclose: ((e: CloseEvent) => void) | null = null;
  onerror: ((e: Event) => void) | null = null;

  sentMessages: string[] = [];

  constructor(url: string) {
    this.url = url;
    MockWebSocket.instances.push(this);
  }

  send(data: string): void {
    this.sentMessages.push(data);
  }

  close(): void {
    this.readyState = MockWebSocket.CLOSED;
    this.onclose?.({} as CloseEvent);
  }

  /** Simulate server open event. */
  simulateOpen(): void {
    this.readyState = MockWebSocket.OPEN;
    this.onopen?.({} as Event);
  }

  /** Simulate server message event. */
  simulateMessage(data: unknown): void {
    this.onmessage?.({ data: JSON.stringify(data) } as MessageEvent);
  }

  /** Simulate server-side close (without triggering our close call). */
  simulateClose(): void {
    this.readyState = MockWebSocket.CLOSED;
    this.onclose?.({} as CloseEvent);
  }

  static instances: MockWebSocket[] = [];

  static reset(): void {
    MockWebSocket.instances = [];
  }

  static latest(): MockWebSocket {
    return MockWebSocket.instances[MockWebSocket.instances.length - 1]!;
  }
}

vi.stubGlobal("WebSocket", MockWebSocket);

// Use fake timers to control reconnect backoff
beforeEach(() => {
  MockWebSocket.reset();
  vi.useFakeTimers();
});

afterEach(() => {
  vi.useRealTimers();
  vi.resetModules();
});

// ---------------------------------------------------------------------------
// Re-import store after each test (module-level singleton resets require this)
// ---------------------------------------------------------------------------

async function freshStore() {
  // Reset modules so singleton state is cleared between tests
  vi.resetModules();
  const mod = await import("./spectrumStore");
  return mod.spectrumStore;
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

describe("spectrumStore", () => {
  it("subscribe sends correct WS message", async () => {
    const store = await freshStore();
    store.subscribe("node-a");

    const sock = MockWebSocket.latest();
    sock.simulateOpen();

    expect(sock.sentMessages).toHaveLength(1);
    expect(JSON.parse(sock.sentMessages[0]!)).toEqual({ subscribe: ["node-a"] });
  });

  it("unsubscribe sends correct WS message", async () => {
    const store = await freshStore();
    store.subscribe("node-b");

    const sock = MockWebSocket.latest();
    sock.simulateOpen();
    sock.sentMessages = []; // clear subscribe message

    store.unsubscribe("node-b");
    expect(sock.sentMessages).toHaveLength(1);
    expect(JSON.parse(sock.sentMessages[0]!)).toEqual({ unsubscribe: ["node-b"] });
  });

  it("store updates bins on message", async () => {
    const store = await freshStore();
    store.subscribe("node-c");

    const sock = MockWebSocket.latest();
    sock.simulateOpen();

    const left = new Array(256).fill(0.5) as number[];
    const right = new Array(256).fill(0.3) as number[];
    sock.simulateMessage({ spectra: { "node-c": { left, right } } });

    expect(store.state.bins["node-c"]?.left[0]).toBe(0.5);
    expect(store.state.bins["node-c"]?.right[0]).toBe(0.3);
  });

  it("auto-disconnect when last subscriber leaves", async () => {
    const store = await freshStore();
    store.subscribe("node-d");

    const sock = MockWebSocket.latest();
    sock.simulateOpen();

    // Track if close was called
    const closeSpy = vi.spyOn(sock, "close");
    store.unsubscribe("node-d");

    expect(closeSpy).toHaveBeenCalled();
  });

  it("does not disconnect if there are still subscribers", async () => {
    const store = await freshStore();
    store.subscribe("node-e");
    store.subscribe("node-f");

    const sock = MockWebSocket.latest();
    sock.simulateOpen();

    const closeSpy = vi.spyOn(sock, "close");
    store.unsubscribe("node-e");

    expect(closeSpy).not.toHaveBeenCalled();
  });

  it("reconnects after server close when subscribers exist", async () => {
    const store = await freshStore();
    store.subscribe("node-g");

    const sock = MockWebSocket.latest();
    sock.simulateOpen();

    const instancesBefore = MockWebSocket.instances.length;
    sock.simulateClose();

    // Advance past backoff delay (1000ms initial)
    vi.advanceTimersByTime(1500);

    expect(MockWebSocket.instances.length).toBeGreaterThan(instancesBefore);
  });

  it("does not reconnect after last unsubscribe + server close", async () => {
    const store = await freshStore();
    store.subscribe("node-h");

    const sock = MockWebSocket.latest();
    sock.simulateOpen();
    store.unsubscribe("node-h");

    const instancesBefore = MockWebSocket.instances.length;
    // simulate close from our own disconnect (already happened inside unsubscribe)
    vi.advanceTimersByTime(5000);

    // No new connections expected
    expect(MockWebSocket.instances.length).toBe(instancesBefore);
  });

  it("ref-counts multiple subscribers to same key", async () => {
    const store = await freshStore();
    store.subscribe("node-i");
    store.subscribe("node-i"); // second subscriber

    const sock = MockWebSocket.latest();
    sock.simulateOpen();

    const closeSpy = vi.spyOn(sock, "close");

    store.unsubscribe("node-i"); // first leaves — should NOT disconnect
    expect(closeSpy).not.toHaveBeenCalled();

    store.unsubscribe("node-i"); // second leaves — should disconnect
    expect(closeSpy).toHaveBeenCalled();
  });

  it("re-subscribes all keys after reconnect", async () => {
    const store = await freshStore();
    store.subscribe("node-j");

    const sock1 = MockWebSocket.latest();
    sock1.simulateOpen();
    sock1.simulateClose();

    vi.advanceTimersByTime(1500);

    const sock2 = MockWebSocket.latest();
    sock2.simulateOpen();

    expect(sock2.sentMessages.some((m) => JSON.parse(m)?.subscribe?.includes("node-j"))).toBe(
      true,
    );
  });
});
