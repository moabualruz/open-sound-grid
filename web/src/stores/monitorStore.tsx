/**
 * Shared monitoring store — allows EqPage and MatrixCell to coordinate
 * monitor-solo state across the entire UI.
 */
import { createContext, useContext, type ParentProps, type JSX } from "solid-js";
import { createStore } from "solid-js/store";
import type { EndpointDescriptor } from "../types/session";
import { descriptorsEqual } from "../components/mixerUtils";

interface MonitorState {
  monitoredCell: { source: EndpointDescriptor; target: EndpointDescriptor } | null;
}

interface MonitorApi {
  state: MonitorState;
  startMonitoring: (source: EndpointDescriptor, target: EndpointDescriptor) => void;
  stopMonitoring: () => void;
  isCellMonitored: (source: EndpointDescriptor, target: EndpointDescriptor) => boolean;
}

const MonitorContext = createContext<MonitorApi>();

export function MonitorProvider(props: ParentProps): JSX.Element {
  const [state, setState] = createStore<MonitorState>({
    monitoredCell: null,
  });

  const api: MonitorApi = {
    state,
    startMonitoring(source, target) {
      setState("monitoredCell", { source, target });
    },
    stopMonitoring() {
      setState("monitoredCell", null);
    },
    isCellMonitored(source, target) {
      return (
        state.monitoredCell !== null &&
        descriptorsEqual(state.monitoredCell.source, source) &&
        descriptorsEqual(state.monitoredCell.target, target)
      );
    },
  };

  return <MonitorContext.Provider value={api}>{props.children}</MonitorContext.Provider>;
}

export function useMonitor(): MonitorApi {
  const ctx = useContext(MonitorContext);
  if (!ctx) throw new Error("useMonitor must be used within MonitorProvider");
  return ctx;
}
