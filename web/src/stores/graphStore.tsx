import { createContext, useContext, onCleanup, type ParentProps } from "solid-js";
import { createStore, reconcile } from "solid-js/store";
import type { AudioGraph } from "../types";

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

  function connect() {
    const protocol = location.protocol === "https:" ? "wss:" : "ws:";
    ws = new WebSocket(`${protocol}//${location.host}/ws/graph`);

    ws.onopen = () => setState("connected", true);

    ws.onmessage = (event) => {
      const graph: AudioGraph = JSON.parse(event.data);
      setState("graph", reconcile(graph));
    };

    ws.onclose = () => {
      setState("connected", false);
      reconnectTimer = setTimeout(connect, 2000);
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
