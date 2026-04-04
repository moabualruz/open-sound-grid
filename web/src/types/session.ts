/** MixerSession types — user's desired state (write model). */

import type { PortKind, GroupNodeKind, NodeIdentifier } from "./graph";
import type { EqConfig } from "./eq";
import type { EffectsConfig } from "./effects";

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
  volumeLeft: number;
  volumeRight: number;
  volumeMixed: boolean;
  volumeLockedMuted: VolumeLockMuteState;
  visible: boolean;
  eq?: EqConfig;
  effects?: EffectsConfig;
}

export interface MixerLink {
  start: EndpointDescriptor;
  end: EndpointDescriptor;
  state: LinkState;
  cellVolume: number;
  cellVolumeLeft: number;
  cellVolumeRight: number;
  /** PipeWire node ID of the cell's null-audio-sink (for VU metering). */
  cellNodeId?: number | null;
  cellEq?: EqConfig;
  cellEffects?: EffectsConfig;
}

export interface AppAssignment {
  applicationName: string;
  binaryName: string;
}

export type SourceType = "hardwareMic" | "hardwareLineIn" | "virtualSource" | "appStream";

export interface Channel {
  id: string;
  kind: GroupNodeKind;
  outputNodeId: number | null;
  assignedApps: AppAssignment[];
  autoApp: boolean;
  allowAppAssignment: boolean;
  sourceType?: SourceType;
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
  channelOrder: EndpointDescriptor[];
  mixOrder: EndpointDescriptor[];
  defaultOutputNodeId: number | null;
}
