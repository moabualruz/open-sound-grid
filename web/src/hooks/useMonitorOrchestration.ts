/**
 * Encapsulates monitor-solo orchestration: muting other cells/endpoints,
 * restoring volumes on disable, and auto-cleanup on unmount.
 */
import { onCleanup } from "solid-js";
import { computeMutedLinks, computeRestoreVolumes } from "../eq/monitorLogic";
import type { Command } from "../types/commands";
import type { EndpointDescriptor, MixerLink, MixerSession } from "../types/session";
import { descriptorsEqual } from "../components/mixerUtils";

interface MonitorApi {
  state: { monitoredCell: { source: EndpointDescriptor; target: EndpointDescriptor } | null };
  startMonitoring: (source: EndpointDescriptor, target: EndpointDescriptor) => void;
  stopMonitoring: () => void;
}

export interface MonitorOrchestration {
  isMonitoringActive: () => boolean;
  toggleMonitoring: () => void;
  enableMonitoring: () => void;
  disableMonitoring: () => void;
}

interface Options {
  /** Accessor returning the current session (re-read on every call, never stale). */
  getSession: () => MixerSession;
  monitorStore: MonitorApi;
  getSend: () => (cmd: Command) => void;
  /** Accessor for the sink descriptor of the currently-viewed EQ target. */
  getSinkDescriptor: () => EndpointDescriptor | undefined;
  /** Accessor for the cell source descriptor — present for cell EQ, absent for mix EQ. */
  getCellSource: () => EndpointDescriptor | undefined;
  /** Accessor for the mix endpoint descriptor — present for mix EQ, absent for cell EQ. */
  getEndpoint: () => EndpointDescriptor | undefined;
}

export function useMonitorOrchestration(opts: Options): MonitorOrchestration {
  /** Links muted by this monitoring session (IDs only — volumes re-read at restore time). */
  let mutedLinkIds: { source: EndpointDescriptor; target: EndpointDescriptor }[] = [];
  /** Endpoint mute states captured at mix-monitoring start. */
  let mutedEndpoints: { endpoint: EndpointDescriptor; wasMuted: boolean }[] = [];

  onCleanup(() => {
    if (opts.monitorStore.state.monitoredCell !== null && isMonitoringActive()) {
      disableMonitoring();
    }
  });

  function isMonitoringActive(): boolean {
    const sinkDesc = opts.getSinkDescriptor();
    if (!sinkDesc) return false;
    const cellSource = opts.getCellSource();
    if (cellSource) {
      return (
        opts.monitorStore.state.monitoredCell !== null &&
        descriptorsEqual(opts.monitorStore.state.monitoredCell.source, cellSource) &&
        descriptorsEqual(opts.monitorStore.state.monitoredCell.target, sinkDesc)
      );
    }
    return opts.monitorStore.state.monitoredCell !== null;
  }

  function enableMonitoring(): void {
    const sinkDesc = opts.getSinkDescriptor();
    if (!sinkDesc) return;
    const cellSource = opts.getCellSource();
    const endpoint = opts.getEndpoint();
    const send = opts.getSend();
    const session = opts.getSession();

    if (cellSource) {
      const result = computeMutedLinks(session.links as MixerLink[], cellSource, sinkDesc);

      if (result.monitoredLink) {
        send({
          type: "setLinkVolume",
          source: result.monitoredLink.source,
          target: result.monitoredLink.target,
          volume: 1,
        });
      }

      for (const m of result.linksToMute) {
        send({ type: "setLinkVolume", source: m.source, target: m.target, volume: 0 });
      }

      mutedLinkIds = result.linksToMute.map((m) => ({ source: m.source, target: m.target }));
      opts.monitorStore.startMonitoring(cellSource, sinkDesc);
    } else if (endpoint) {
      const thisMixDesc = sinkDesc;
      mutedEndpoints = [];

      for (const [desc, ep] of session.endpoints) {
        if (!("channel" in desc)) continue;
        const ch = session.channels[desc.channel];
        if (!ch || ch.kind !== "sink") continue;
        const isMuted =
          ep.volumeLockedMuted === "mutedLocked" || ep.volumeLockedMuted === "mutedUnlocked";
        if (descriptorsEqual(desc, thisMixDesc)) {
          if (isMuted) {
            mutedEndpoints.push({ endpoint: desc, wasMuted: true });
            send({ type: "setMute", endpoint: desc, muted: false });
          }
        } else {
          mutedEndpoints.push({ endpoint: desc, wasMuted: isMuted });
          if (!isMuted) {
            send({ type: "setMute", endpoint: desc, muted: true });
          }
        }
      }

      opts.monitorStore.startMonitoring(endpoint, sinkDesc);
    }
  }

  function disableMonitoring(): void {
    const send = opts.getSend();
    const session = opts.getSession();
    const restore = computeRestoreVolumes(session.links as MixerLink[], mutedLinkIds);
    for (const cmd of restore.commands) {
      send({
        type: "setLinkVolume",
        source: cmd.source,
        target: cmd.target,
        volume: cmd.volume,
      });
    }
    mutedLinkIds = [];

    for (const { endpoint, wasMuted } of mutedEndpoints) {
      send({ type: "setMute", endpoint, muted: wasMuted });
    }
    mutedEndpoints = [];

    opts.monitorStore.stopMonitoring();
  }

  function toggleMonitoring(): void {
    if (opts.monitorStore.state.monitoredCell !== null && isMonitoringActive()) {
      disableMonitoring();
    } else {
      enableMonitoring();
    }
  }

  return { isMonitoringActive, toggleMonitoring, enableMonitoring, disableMonitoring };
}
