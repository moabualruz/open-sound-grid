import { describe, it, expect } from "vitest";
import { computeMutedLinks, computeRestoreVolumes } from "./monitorLogic";
import type { MixerLink, EndpointDescriptor } from "../types";

// Helper to create a link
function link(
  source: string,
  target: string,
  volume: number,
): MixerLink {
  return {
    start: { channel: source },
    end: { channel: target },
    state: "connectedUnlocked",
    cellVolume: volume,
    cellVolumeLeft: volume,
    cellVolumeRight: volume,
  };
}

describe("computeMutedLinks", () => {
  const chA = "ch-a";
  const chB = "ch-b";
  const chC = "ch-c";
  const mainMix = "main-mix";
  const streamMix = "stream-mix";

  it("mutes other cells going to the same mix", () => {
    const links = [
      link(chA, mainMix, 0.8),
      link(chB, mainMix, 0.5),
    ];

    const result = computeMutedLinks(links, { channel: chA }, { channel: mainMix });

    expect(result.monitoredLink).not.toBeNull();
    expect(result.monitoredLink!.prevVolume).toBe(0.8);
    expect(result.linksToMute).toHaveLength(1);
    expect(result.linksToMute[0].prevVolume).toBe(0.5);
  });

  it("mutes cells going to OTHER mixes as well", () => {
    const links = [
      link(chA, mainMix, 0.8),
      link(chB, mainMix, 0.5),
      link(chC, streamMix, 0.7),
    ];

    const result = computeMutedLinks(links, { channel: chA }, { channel: mainMix });

    expect(result.monitoredLink).not.toBeNull();
    // Both chB→MainMix AND chC→StreamMix should be muted
    expect(result.linksToMute).toHaveLength(2);
    expect(result.linksToMute.map((l) => ({
      source: (l.source as { channel: string }).channel,
      target: (l.target as { channel: string }).channel,
    }))).toContainEqual({ source: chB, target: mainMix });
    expect(result.linksToMute.map((l) => ({
      source: (l.source as { channel: string }).channel,
      target: (l.target as { channel: string }).channel,
    }))).toContainEqual({ source: chC, target: streamMix });
  });

  it("returns null monitoredLink when target link does not exist", () => {
    const links = [
      link(chB, mainMix, 0.5),
    ];

    const result = computeMutedLinks(links, { channel: chA }, { channel: mainMix });

    expect(result.monitoredLink).toBeNull();
    expect(result.linksToMute).toHaveLength(1);
  });

  it("handles empty links array", () => {
    const result = computeMutedLinks([], { channel: chA }, { channel: mainMix });

    expect(result.monitoredLink).toBeNull();
    expect(result.linksToMute).toHaveLength(0);
  });

  it("captures correct previous volumes for all muted links", () => {
    const links = [
      link(chA, mainMix, 0.9),
      link(chB, mainMix, 0.3),
      link(chC, streamMix, 0.6),
      link(chA, streamMix, 0.4),
    ];

    const result = computeMutedLinks(links, { channel: chA }, { channel: mainMix });

    // chA→mainMix is monitored, all others muted
    expect(result.linksToMute).toHaveLength(3);
    const mutedMap = new Map(
      result.linksToMute.map((l) => [
        `${(l.source as { channel: string }).channel}->${(l.target as { channel: string }).channel}`,
        l.prevVolume,
      ]),
    );
    expect(mutedMap.get(`${chB}->${mainMix}`)).toBe(0.3);
    expect(mutedMap.get(`${chC}->${streamMix}`)).toBe(0.6);
    expect(mutedMap.get(`${chA}->${streamMix}`)).toBe(0.4);
  });
});

describe("computeRestoreVolumes", () => {
  const chB = "ch-b";
  const chC = "ch-c";
  const mainMix = "main-mix";
  const streamMix = "stream-mix";

  it("restores volumes from current state, not stale state", () => {
    // Current state: user changed chB volume while monitoring was active
    const currentLinks = [
      link(chB, mainMix, 0.7), // was 0.5, user changed to 0.7
      link(chC, streamMix, 0.3),
    ];

    const previouslyMuted = [
      { source: { channel: chB } as EndpointDescriptor, target: { channel: mainMix } as EndpointDescriptor },
      { source: { channel: chC } as EndpointDescriptor, target: { channel: streamMix } as EndpointDescriptor },
    ];

    const result = computeRestoreVolumes(currentLinks, previouslyMuted);

    expect(result.commands).toHaveLength(2);
    // Should restore to CURRENT volumes (0.7, 0.3), NOT stale volumes
    const cmdMap = new Map(
      result.commands.map((c) => [
        `${(c.source as { channel: string }).channel}->${(c.target as { channel: string }).channel}`,
        c.volume,
      ]),
    );
    expect(cmdMap.get(`${chB}->${mainMix}`)).toBe(0.7);
    expect(cmdMap.get(`${chC}->${streamMix}`)).toBe(0.3);
  });

  it("skips links that no longer exist", () => {
    const currentLinks = [
      link(chB, mainMix, 0.5),
    ];

    const previouslyMuted = [
      { source: { channel: chB } as EndpointDescriptor, target: { channel: mainMix } as EndpointDescriptor },
      { source: { channel: chC } as EndpointDescriptor, target: { channel: streamMix } as EndpointDescriptor },
    ];

    const result = computeRestoreVolumes(currentLinks, previouslyMuted);

    expect(result.commands).toHaveLength(1);
    expect(result.commands[0].volume).toBe(0.5);
  });

  it("returns empty commands when nothing was muted", () => {
    const result = computeRestoreVolumes([], []);
    expect(result.commands).toHaveLength(0);
  });
});
