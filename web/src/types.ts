/** TypeScript types matching osg-core's Serialize output (camelCase wire format). */

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
}

// ---------------------------------------------------------------------------
// MixerSession (write model) — matches osg-core graph::types::MixerSession
// ---------------------------------------------------------------------------

export type VolumeLockMuteState =
  | "muteMixed"
  | "mutedLocked"
  | "mutedUnlocked"
  | "unmutedLocked"
  | "unmutedUnlocked";

export type LinkState =
  | "partiallyConnected"
  | "connectedUnlocked"
  | "connectedLocked"
  | "disconnectedLocked";

export type EndpointDescriptor =
  | { ephemeralNode: [number, PortKind] }
  | { persistentNode: [string, PortKind] }
  | { channel: string }
  | { app: [string, PortKind] }
  | { device: [string, PortKind] };

export interface Endpoint {
  descriptor: EndpointDescriptor;
  isPlaceholder: boolean;
  displayName: string;
  customName: string | null;
  iconName: string;
  details: string[];
  volume: number;
  volumeMixed: boolean;
  volumeLockedMuted: VolumeLockMuteState;
  visible: boolean;
}

export interface MixerLink {
  start: EndpointDescriptor;
  end: EndpointDescriptor;
  state: LinkState;
}

export interface Channel {
  id: string;
  kind: GroupNodeKind;
  outputNodeId: number | null;
}

export interface App {
  id: string;
  kind: PortKind;
  name: string;
  binary: string;
  iconName: string;
  exceptions: EndpointDescriptor[];
}

export interface MixerSession {
  activeSources: EndpointDescriptor[];
  activeSinks: EndpointDescriptor[];
  endpoints: [EndpointDescriptor, Endpoint][];
  links: MixerLink[];
  persistentNodes: Record<string, [NodeIdentifier, PortKind]>;
  apps: Record<string, App>;
  devices: Record<string, unknown>;
  channels: Record<string, Channel>;
  displayOrder: EndpointDescriptor[];
  defaultOutputNodeId: number | null;
}

// ---------------------------------------------------------------------------
// Commands (frontend → backend via /ws/commands)
// ---------------------------------------------------------------------------

export type Command =
  | { type: "createChannel"; name: string; kind: GroupNodeKind }
  | { type: "removeEndpoint"; endpoint: EndpointDescriptor }
  | { type: "setVolume"; endpoint: EndpointDescriptor; volume: number }
  | { type: "setMute"; endpoint: EndpointDescriptor; muted: boolean }
  | { type: "setVolumeLocked"; endpoint: EndpointDescriptor; locked: boolean }
  | { type: "renameEndpoint"; endpoint: EndpointDescriptor; name: string | null }
  | { type: "link"; source: EndpointDescriptor; target: EndpointDescriptor }
  | { type: "removeLink"; source: EndpointDescriptor; target: EndpointDescriptor }
  | {
      type: "setLinkLocked";
      source: EndpointDescriptor;
      target: EndpointDescriptor;
      locked: boolean;
    }
  | { type: "setMixOutput"; channel: string; outputNodeId: number | null }
  | { type: "setEndpointVisible"; endpoint: EndpointDescriptor; visible: boolean }
  | { type: "setDisplayOrder"; order: EndpointDescriptor[] };
