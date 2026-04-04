import type { EndpointDescriptor, Endpoint, MixerLink } from "../types";

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
  return JSON.stringify(a) === JSON.stringify(b);
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
