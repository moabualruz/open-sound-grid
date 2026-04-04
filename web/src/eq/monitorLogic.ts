/**
 * Pure functions for monitor-solo computation.
 * No framework dependencies — easily testable.
 */
import type { EndpointDescriptor, MixerLink } from "../types/session";
import { descriptorsEqual } from "../components/mixerUtils";

export interface MonitorMuteResult {
  /** Links to mute (volume set to 0). */
  linksToMute: { source: EndpointDescriptor; target: EndpointDescriptor; prevVolume: number }[];
  /** The monitored link to boost to 100%. */
  monitoredLink: {
    source: EndpointDescriptor;
    target: EndpointDescriptor;
    prevVolume: number;
  } | null;
}

export interface RestoreVolumeResult {
  /** Commands to restore volumes. */
  commands: { source: EndpointDescriptor; target: EndpointDescriptor; volume: number }[];
}

/**
 * Compute which links should be muted when monitoring a specific cell.
 * Mutes ALL other links across ALL mixes — not just same-mix.
 */
export function computeMutedLinks(
  allLinks: MixerLink[],
  monitoredSource: EndpointDescriptor,
  monitoredTarget: EndpointDescriptor,
): MonitorMuteResult {
  const result: MonitorMuteResult = { linksToMute: [], monitoredLink: null };

  for (const link of allLinks) {
    const isMonitored =
      descriptorsEqual(link.start, monitoredSource) && descriptorsEqual(link.end, monitoredTarget);

    if (isMonitored) {
      result.monitoredLink = {
        source: link.start,
        target: link.end,
        prevVolume: link.cellVolume,
      };
    } else {
      result.linksToMute.push({
        source: link.start,
        target: link.end,
        prevVolume: link.cellVolume,
      });
    }
  }

  return result;
}

/**
 * Compute restore commands from the current state of muted links.
 * This avoids stale-volume bugs by re-reading current volumes at restore time.
 */
export function computeRestoreVolumes(
  currentLinks: MixerLink[],
  previouslyMutedLinks: { source: EndpointDescriptor; target: EndpointDescriptor }[],
): RestoreVolumeResult {
  const commands: RestoreVolumeResult["commands"] = [];

  for (const muted of previouslyMutedLinks) {
    const currentLink = currentLinks.find(
      (l) => descriptorsEqual(l.start, muted.source) && descriptorsEqual(l.end, muted.target),
    );
    if (currentLink) {
      commands.push({
        source: muted.source,
        target: muted.target,
        volume: currentLink.cellVolume,
      });
    }
  }

  return { commands };
}
