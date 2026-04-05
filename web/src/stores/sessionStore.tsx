import { createContext, useContext, onCleanup } from "solid-js";
import type { ParentProps } from "solid-js";
import { createStore, reconcile } from "solid-js/store";
import type { MixerSession } from "../types/session";
import type { Command } from "../types/commands";

export const BACKOFF_INITIAL_MS = 1000;
export const BACKOFF_CAP_MS = 30_000;

/** Returns the delay in ms for the given attempt index (0-based). */
export function computeBackoffDelay(attempt: number): number {
  const delay = BACKOFF_INITIAL_MS * Math.pow(2, attempt);
  return Math.min(delay, BACKOFF_CAP_MS);
}

/** Returns the next backoff delay given the current delay. */
export function nextBackoffDelay(current: number): number {
  return Math.min(current * 2, BACKOFF_CAP_MS);
}

const EMPTY_SESSION: MixerSession = {
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

interface SessionState {
  session: MixerSession;
  connected: boolean;
  reconnecting: boolean;
  reconnectAttempt: number;
}

interface SessionApi {
  state: SessionState;
  send: (cmd: Command) => void;
}

const SessionContext = createContext<SessionApi>();

export function SessionProvider(props: ParentProps) {
  const [state, setState] = createStore<SessionState>({
    session: EMPTY_SESSION,
    connected: false,
    reconnecting: false,
    reconnectAttempt: 0,
  });

  let sessionWs: WebSocket | null = null;
  let commandWs: WebSocket | null = null;
  let reconnectTimer: ReturnType<typeof setTimeout> | null = null;
  const pendingCommands: string[] = [];

  function scheduleReconnect(attempt: number) {
    const delay = computeBackoffDelay(attempt);
    setState("reconnecting", true);
    setState("reconnectAttempt", attempt);
    reconnectTimer = setTimeout(() => connect(attempt + 1), delay);
  }

  function connect(attempt = 0) {
    const protocol = location.protocol === "https:" ? "wss:" : "ws:";
    const host = location.host;

    // Session state WebSocket (read)
    sessionWs = new WebSocket(`${protocol}//${host}/ws/session`);
    sessionWs.onopen = () => {
      setState("connected", true);
      setState("reconnecting", false);
      setState("reconnectAttempt", 0);
    };
    sessionWs.onmessage = (event) => {
      const session: MixerSession = JSON.parse(event.data);
      setState("session", reconcile(session));
    };
    sessionWs.onclose = () => {
      setState("connected", false);
      scheduleReconnect(attempt);
    };
    sessionWs.onerror = () => sessionWs?.close();

    // Command WebSocket (write)
    commandWs = new WebSocket(`${protocol}//${host}/ws/commands`);
    commandWs.onopen = () => {
      // Flush any commands queued while disconnected
      while (pendingCommands.length > 0) {
        commandWs!.send(pendingCommands.shift()!);
      }
    };
    commandWs.onerror = () => commandWs?.close();
  }

  function send(cmd: Command) {
    const json = JSON.stringify(cmd);
    if (commandWs?.readyState === WebSocket.OPEN) {
      commandWs.send(json);
    } else {
      pendingCommands.push(json);
    }
  }

  connect();

  onCleanup(() => {
    if (reconnectTimer) clearTimeout(reconnectTimer);
    sessionWs?.close();
    commandWs?.close();
  });

  const api: SessionApi = { state, send };

  return <SessionContext.Provider value={api}>{props.children}</SessionContext.Provider>;
}

export function useSession(): SessionApi {
  const ctx = useContext(SessionContext);
  if (!ctx) throw new Error("useSession must be used within SessionProvider");
  return ctx;
}
