/**
 * Spectrum store — WebSocket client for /ws/spectrum.
 * Connects only when at least one subscriber exists.
 * Streams FFT magnitude data at ~15fps per subscribed node.
 *
 * Wire format (server → client):
 *   { spectra: { [nodeKey]: { left: number[256], right: number[256] } } }
 *   { nodeId: string, bins: number[256] }
 *
 * Wire format (client → server):
 *   { subscribe: [nodeKey, ...] }
 *   { unsubscribe: [nodeKey, ...] }
 */
import { createStore } from "solid-js/store";
import { computeBackoffDelay } from "../stores/backoff";

export const SPECTRUM_BINS = 256;

export interface SpectrumBins {
  left: number[];
  right: number[];
}

interface SpectrumState {
  /** Current bins per subscribed node key. */
  bins: Record<string, SpectrumBins>;
  connected: boolean;
}

interface SpectrumStoreApi {
  state: SpectrumState;
  subscribe: (nodeKey: string) => void;
  unsubscribe: (nodeKey: string) => void;
}

// ---------------------------------------------------------------------------
// Module-level singleton so multiple components share one connection
// ---------------------------------------------------------------------------

const [state, setState] = createStore<SpectrumState>({
  bins: {},
  connected: false,
});

/** Number of components subscribed to each node key. */
const refCounts: Map<string, number> = new Map();
/** All currently desired node keys (ref count > 0). */
const subscribedKeys = new Set<string>();

let ws: WebSocket | null = null;
let reconnectTimer: ReturnType<typeof setTimeout> | null = null;
let reconnectAttempt = 0;

// ---------------------------------------------------------------------------
// WebSocket lifecycle
// ---------------------------------------------------------------------------

function sendMessage(msg: object): void {
  if (ws?.readyState === WebSocket.OPEN) {
    ws.send(JSON.stringify(msg));
  }
}

function toStereoBins(magnitudes: number[]): SpectrumBins {
  return {
    left: magnitudes.slice(0, SPECTRUM_BINS),
    right: magnitudes.slice(0, SPECTRUM_BINS),
  };
}

function connect(): void {
  if (ws) return; // already connecting/connected

  const protocol = location.protocol === "https:" ? "wss:" : "ws:";
  const sock = new WebSocket(`${protocol}//${location.host}/ws/spectrum`);
  ws = sock;

  sock.onopen = () => {
    reconnectAttempt = 0;
    setState("connected", true);
    // Re-subscribe all pending keys after reconnect
    if (subscribedKeys.size > 0) {
      sendMessage({ subscribe: Array.from(subscribedKeys) });
    }
  };

  sock.onmessage = (event: MessageEvent) => {
    try {
      const data = JSON.parse(event.data as string) as {
        spectra?: Record<string, SpectrumBins>;
        nodeId?: string;
        bins?: number[];
      };
      if (data.spectra) {
        for (const [key, bins] of Object.entries(data.spectra)) {
          // Only update keys we're actively subscribed to
          if (subscribedKeys.has(key)) {
            setState("bins", key, bins);
          }
        }
      } else if (data.nodeId && Array.isArray(data.bins) && subscribedKeys.has(data.nodeId)) {
        setState("bins", data.nodeId, toStereoBins(data.bins));
      }
    } catch {
      // Malformed frame — ignore
    }
  };

  sock.onclose = () => {
    ws = null;
    setState("connected", false);
    // Reconnect only if there are still active subscribers
    if (subscribedKeys.size > 0) {
      scheduleReconnect();
    }
  };

  sock.onerror = () => sock.close();
}

function scheduleReconnect(): void {
  if (reconnectTimer) return;
  const delay = computeBackoffDelay(reconnectAttempt++);
  reconnectTimer = setTimeout(() => {
    reconnectTimer = null;
    if (subscribedKeys.size > 0) {
      connect();
    }
  }, delay);
}

function disconnectIfIdle(): void {
  if (subscribedKeys.size === 0) {
    if (reconnectTimer) {
      clearTimeout(reconnectTimer);
      reconnectTimer = null;
    }
    if (ws) {
      ws.onclose = null;
      ws.onerror = null;
      ws.onmessage = null;
      ws.close();
      ws = null;
    }
    setState("connected", false);
  }
}

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

function subscribe(nodeKey: string): void {
  const count = refCounts.get(nodeKey) ?? 0;
  refCounts.set(nodeKey, count + 1);

  if (count === 0) {
    // First subscriber for this key
    subscribedKeys.add(nodeKey);
    if (!ws) {
      connect();
    } else {
      sendMessage({ subscribe: [nodeKey] });
    }
  }
}

function unsubscribe(nodeKey: string): void {
  const count = refCounts.get(nodeKey) ?? 0;
  if (count <= 1) {
    refCounts.delete(nodeKey);
    subscribedKeys.delete(nodeKey);
    sendMessage({ unsubscribe: [nodeKey] });
    // Remove stale bins
    setState("bins", (prev: Record<string, SpectrumBins>) => {
      const next = { ...prev };
      delete next[nodeKey];
      return next;
    });
    disconnectIfIdle();
  } else {
    refCounts.set(nodeKey, count - 1);
  }
}

export const spectrumStore: SpectrumStoreApi = { state, subscribe, unsubscribe };
