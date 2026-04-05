import { createContext, useContext, onCleanup } from "solid-js";
import type { ParentProps } from "solid-js";
import { createStore, reconcile } from "solid-js/store";
import type { MixerSession } from "../types/session";
import type { Command } from "../types/commands";

export const BACKOFF_INITIAL_MS = 1000;
export const BACKOFF_CAP_MS = 30_000;

/** Maximum number of pending commands queued while disconnected. */
const PENDING_COMMANDS_CAP = 100;

/** Returns the delay in ms for the given attempt index (0-based). */
export function computeBackoffDelay(attempt: number): number {
  const delay = BACKOFF_INITIAL_MS * Math.pow(2, attempt);
  return Math.min(delay, BACKOFF_CAP_MS);
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
    // F1-P0-3: Close existing sockets before reconnect
    if (sessionWs) {
      sessionWs.onclose = null;
      sessionWs.onerror = null;
      sessionWs.onmessage = null;
      sessionWs.close();
      sessionWs = null;
    }
    if (commandWs) {
      commandWs.onclose = null;
      commandWs.onerror = null;
      commandWs.onopen = null;
      commandWs.close();
      commandWs = null;
    }

    const protocol = location.protocol === "https:" ? "wss:" : "ws:";
    const host = location.host;

    // Track whether this connection was successfully opened so onclose
    // can decide the correct reconnect attempt (F1-P0-1).
    let opened = false;

    // Session state WebSocket (read)
    const session = new WebSocket(`${protocol}//${host}/ws/session`);
    sessionWs = session;
    session.onopen = () => {
      opened = true;
      setState("connected", true);
      setState("reconnecting", false);
      setState("reconnectAttempt", 0);
    };
    session.onmessage = (event) => {
      const data: MixerSession = JSON.parse(event.data);
      setState("session", reconcile(data));
    };
    session.onclose = () => {
      setState("connected", false);
      // F1-P0-1: If connection had been open, restart backoff from 0.
      // If it never opened, continue the existing backoff sequence.
      scheduleReconnect(opened ? 0 : attempt);
    };
    session.onerror = () => session.close();

    // Command WebSocket (write)
    const command = new WebSocket(`${protocol}//${host}/ws/commands`);
    commandWs = command;
    command.onopen = () => {
      // Flush any commands queued while disconnected
      while (pendingCommands.length > 0) {
        command.send(pendingCommands.shift()!);
      }
    };
    // F1-P0-2: commandWs reconnect piggybacks on sessionWs reconnect
    command.onclose = () => {
      // If sessionWs is still open, close it to trigger unified reconnect
      if (sessionWs && sessionWs.readyState === WebSocket.OPEN) {
        sessionWs.close();
      }
    };
    command.onerror = () => command.close();
  }

  function send(cmd: Command) {
    const json = JSON.stringify(cmd);
    if (commandWs?.readyState === WebSocket.OPEN) {
      commandWs.send(json);
    } else {
      pendingCommands.push(json);
      // F1-P0-2: Cap pending commands — drop oldest on overflow
      while (pendingCommands.length > PENDING_COMMANDS_CAP) {
        pendingCommands.shift();
      }
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
