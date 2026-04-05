import { createContext, useContext, onCleanup, type ParentProps } from "solid-js";
import { createStore } from "solid-js/store";
import { computeBackoffDelay } from "./backoff";

interface PeakLevel {
  nodeId: number;
  left: number;
  right: number;
}

interface LevelsState {
  /** Peak levels keyed by PW node ID. */
  peaks: Record<string, { left: number; right: number }>;
  connected: boolean;
}

const LevelsContext = createContext<LevelsState>();

export function LevelsProvider(props: ParentProps) {
  const [state, setState] = createStore<LevelsState>({
    peaks: {},
    connected: false,
  });

  let ws: WebSocket | null = null;
  let reconnectTimer: ReturnType<typeof setTimeout> | null = null;
  let attempt = 0;

  function connect() {
    const protocol = location.protocol === "https:" ? "wss:" : "ws:";
    ws = new WebSocket(`${protocol}//${location.host}/ws/levels`);

    ws.onopen = () => {
      attempt = 0;
      setState("connected", true);
    };

    ws.onmessage = (event) => {
      const levels: PeakLevel[] = JSON.parse(event.data);
      const peaks: Record<string, { left: number; right: number }> = {};
      for (const l of levels) {
        peaks[String(l.nodeId)] = { left: l.left, right: l.right };
      }
      for (const [key, val] of Object.entries(peaks)) {
        setState("peaks", key, val);
      }
    };

    ws.onclose = () => {
      setState("connected", false);
      const delay = computeBackoffDelay(attempt);
      attempt += 1;
      reconnectTimer = setTimeout(connect, delay);
    };

    ws.onerror = () => ws?.close();
  }

  connect();

  onCleanup(() => {
    if (reconnectTimer) clearTimeout(reconnectTimer);
    ws?.close();
  });

  return <LevelsContext.Provider value={state}>{props.children}</LevelsContext.Provider>;
}

export function useLevels(): LevelsState {
  const ctx = useContext(LevelsContext);
  if (!ctx) throw new Error("useLevels must be used within LevelsProvider");
  return ctx;
}
