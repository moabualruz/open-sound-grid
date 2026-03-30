import { createContext, useContext, onCleanup } from "solid-js";
import type { ParentProps } from "solid-js";
import { createStore, reconcile } from "solid-js/store";
import type { MixerSession, Command } from "../types";

const EMPTY_SESSION: MixerSession = {
  activeSources: [],
  activeSinks: [],
  endpoints: [],
  links: [],
  persistentNodes: {},
  apps: {},
  devices: {},
  channels: {},
};

interface SessionState {
  session: MixerSession;
  connected: boolean;
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
  });

  let sessionWs: WebSocket | null = null;
  let commandWs: WebSocket | null = null;
  let reconnectTimer: ReturnType<typeof setTimeout> | null = null;

  function connect() {
    const protocol = location.protocol === "https:" ? "wss:" : "ws:";
    const host = location.host;

    // Session state WebSocket (read)
    sessionWs = new WebSocket(`${protocol}//${host}/ws/session`);
    sessionWs.onopen = () => setState("connected", true);
    sessionWs.onmessage = (event) => {
      const session: MixerSession = JSON.parse(event.data);
      setState("session", reconcile(session));
    };
    sessionWs.onclose = () => {
      setState("connected", false);
      reconnectTimer = setTimeout(connect, 2000);
    };
    sessionWs.onerror = () => sessionWs?.close();

    // Command WebSocket (write)
    commandWs = new WebSocket(`${protocol}//${host}/ws/commands`);
    commandWs.onerror = () => commandWs?.close();
  }

  function send(cmd: Command) {
    if (commandWs?.readyState === WebSocket.OPEN) {
      commandWs.send(JSON.stringify(cmd));
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
