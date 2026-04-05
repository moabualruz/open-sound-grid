import { createContext, useContext, onCleanup, type ParentProps } from "solid-js";
import { createStore, reconcile } from "solid-js/store";
import type { AudioGraph } from "../types/graph";
import { computeBackoffDelay } from "./backoff";

const EMPTY_GRAPH: AudioGraph = {
  groupNodes: {},
  clients: {},
  devices: {},
  nodes: {},
  ports: {},
  links: {},
  defaultSinkName: null,
  defaultSourceName: null,
};

interface GraphState {
  graph: AudioGraph;
  connected: boolean;
}

const GraphContext = createContext<GraphState>();

export function GraphProvider(props: ParentProps) {
  const [state, setState] = createStore<GraphState>({
    graph: EMPTY_GRAPH,
    connected: false,
  });

  let ws: WebSocket | null = null;
  let reconnectTimer: ReturnType<typeof setTimeout> | null = null;
  let attempt = 0;

  function connect() {
    const protocol = location.protocol === "https:" ? "wss:" : "ws:";
    ws = new WebSocket(`${protocol}//${location.host}/ws/graph`);

    ws.onopen = () => {
      attempt = 0;
      setState("connected", true);
    };

    ws.onmessage = (event) => {
      const graph: AudioGraph = JSON.parse(event.data);
      setState("graph", reconcile(graph));
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

  return <GraphContext.Provider value={state}>{props.children}</GraphContext.Provider>;
}

export function useGraph(): GraphState {
  const ctx = useContext(GraphContext);
  if (!ctx) throw new Error("useGraph must be used within GraphProvider");
  return ctx;
}
