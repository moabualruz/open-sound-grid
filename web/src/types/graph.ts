/** AudioGraph types — projection of PipeWire reality (read model). */

export type PortKind = "source" | "sink";
export type GroupNodeKind = "source" | "duplex" | "sink";

export interface NodeIdentifier {
  isMonitor: boolean;
  nodeName: string | null;
  nodeNick: string | null;
  nodeDescription: string | null;
  objectPath: string | null;
}

export type EndpointId =
  | { device: { id: number; deviceIndex: number | null } }
  | { client: number };

export interface PwNode {
  id: number;
  identifier: NodeIdentifier;
  endpoint: EndpointId;
  ports: [number, PortKind, boolean][];
  channelVolumes: number[];
  mute: boolean;
}

export interface PwClient {
  id: number;
  name: string;
  isOsg: boolean;
  nodes: number[];
}

export interface PwDevice {
  id: number;
  name: string;
  client: number;
  nodes: number[];
  activeRoutes: { routeIndex: number; deviceIndex: number; iconName: string | null }[];
}

export interface PwPort {
  id: number;
  name: string;
  channel: string;
  node: number;
  kind: PortKind;
  isMonitor: boolean;
  links: number[];
}

export interface PwLink {
  id: number;
  startNode: number;
  startPort: number;
  endNode: number;
  endPort: number;
}

export interface PwGroupNode {
  id: number | null;
  name: string;
  kind: GroupNodeKind;
}

export interface AudioGraph {
  groupNodes: Record<string, PwGroupNode>;
  clients: Record<string, PwClient>;
  devices: Record<string, PwDevice>;
  nodes: Record<string, PwNode>;
  ports: Record<string, PwPort>;
  links: Record<string, PwLink>;
  defaultSinkName: string | null;
  defaultSourceName: string | null;
}
