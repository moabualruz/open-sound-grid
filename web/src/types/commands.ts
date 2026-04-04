/** Command union — frontend → backend via /ws/commands */

import type { GroupNodeKind } from "./graph";
import type { EndpointDescriptor } from "./session";
import type { EqConfig } from "./eq";
import type { EffectsConfig } from "./effects";

export type Command =
  | { type: "createChannel"; name: string; kind: GroupNodeKind }
  | { type: "removeEndpoint"; endpoint: EndpointDescriptor }
  | { type: "setVolume"; endpoint: EndpointDescriptor; volume: number }
  | { type: "setStereoVolume"; endpoint: EndpointDescriptor; left: number; right: number }
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
  | {
      type: "setLinkVolume";
      source: EndpointDescriptor;
      target: EndpointDescriptor;
      volume: number;
    }
  | {
      type: "setLinkStereoVolume";
      source: EndpointDescriptor;
      target: EndpointDescriptor;
      left: number;
      right: number;
    }
  | { type: "setMixOutput"; channel: string; outputNodeId: number | null }
  | { type: "setEndpointVisible"; endpoint: EndpointDescriptor; visible: boolean }
  | { type: "setChannelOrder"; order: EndpointDescriptor[] }
  | { type: "setMixOrder"; order: EndpointDescriptor[] }
  | { type: "assignApp"; channel: string; applicationName: string; binaryName: string }
  | { type: "unassignApp"; channel: string; applicationName: string; binaryName: string }
  | { type: "setEq"; endpoint: EndpointDescriptor; eq: EqConfig }
  | {
      type: "setCellEq";
      source: EndpointDescriptor;
      target: EndpointDescriptor;
      eq: EqConfig;
    }
  | { type: "setEffects"; endpoint: EndpointDescriptor; effects: EffectsConfig }
  | {
      type: "setCellEffects";
      source: EndpointDescriptor;
      target: EndpointDescriptor;
      effects: EffectsConfig;
    };
