import type { EndpointDescriptor, Endpoint, MixerLink } from "../types/session";

export const MIX_COLORS: Record<string, string> = {
  Monitor: "var(--color-mix-monitor)",
  Stream: "var(--color-mix-stream)",
  VOD: "var(--color-mix-vod)",
  Chat: "var(--color-mix-chat)",
  Aux: "var(--color-mix-aux)",
};

export function getMixColor(name: string): string {
  for (const [key, color] of Object.entries(MIX_COLORS)) {
    if (name.includes(key)) return color;
  }
  return "var(--color-mix-monitor)";
}

export function descriptorsEqual(a: EndpointDescriptor, b: EndpointDescriptor): boolean {
  if ("ephemeralNode" in a && "ephemeralNode" in b) {
    return a.ephemeralNode[0] === b.ephemeralNode[0] && a.ephemeralNode[1] === b.ephemeralNode[1];
  }
  if ("persistentNode" in a && "persistentNode" in b) {
    return (
      a.persistentNode[0] === b.persistentNode[0] && a.persistentNode[1] === b.persistentNode[1]
    );
  }
  if ("channel" in a && "channel" in b) {
    return a.channel === b.channel;
  }
  if ("app" in a && "app" in b) {
    return a.app[0] === b.app[0] && a.app[1] === b.app[1];
  }
  if ("device" in a && "device" in b) {
    return a.device[0] === b.device[0] && a.device[1] === b.device[1];
  }
  return false;
}

export function descriptorKey(d: EndpointDescriptor): string {
  if ("ephemeralNode" in d) return `ephemeralNode:${d.ephemeralNode[0]}:${d.ephemeralNode[1]}`;
  if ("persistentNode" in d) return `persistentNode:${d.persistentNode[0]}:${d.persistentNode[1]}`;
  if ("channel" in d) return `channel:${d.channel}`;
  if ("app" in d) return `app:${d.app[0]}:${d.app[1]}`;
  if ("device" in d) return `device:${d.device[0]}:${d.device[1]}`;
  return JSON.stringify(d);
}

export function findEndpoint(
  endpoints: [EndpointDescriptor, Endpoint][],
  desc: EndpointDescriptor,
): Endpoint | undefined {
  return endpoints.find(([d]) => descriptorsEqual(d, desc))?.[1];
}

export function findLink(
  links: MixerLink[],
  source: EndpointDescriptor,
  target: EndpointDescriptor,
): MixerLink | null {
  return (
    links.find((l) => descriptorsEqual(l.start, source) && descriptorsEqual(l.end, target)) ?? null
  );
}
