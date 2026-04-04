import { createStore } from "solid-js/store";
import { createEffect } from "solid-js";
import { getOutputDevices } from "./MixHeader";
import type { EndpointDescriptor, Endpoint } from "../types";

export interface MixEntry {
  desc: EndpointDescriptor;
  ep: Endpoint;
}

interface GraphState {
  devices: Parameters<typeof getOutputDevices>[0];
  nodes: Parameters<typeof getOutputDevices>[1];
  defaultSinkName: string | null;
}

type SendFn = (cmd: { type: "setMixOutput"; channel: string; outputNodeId: number | null }) => void;

export function useMixOutputs(
  getMixes: () => MixEntry[],
  getChannels: () => Record<string, { outputNodeId?: number | null }>,
  getGraph: () => GraphState,
  send: SendFn,
) {
  const [mixOutputs, setMixOutputs] = createStore<Record<string, string | null>>({});
  let outputsInitialized = false;

  createEffect(() => {
    const graph = getGraph();
    const allDevs = getOutputDevices(graph.devices, graph.nodes);
    if (allDevs.length === 0 || outputsInitialized) return;

    const channels = getChannels();
    for (const m of getMixes()) {
      if (!("channel" in m.desc)) continue;
      const ch = channels[m.desc.channel];
      if (ch?.outputNodeId) {
        const dev = allDevs.find((d) => d.pwNodeId === ch.outputNodeId);
        if (dev) setMixOutputs(JSON.stringify(m.desc), dev.deviceId);
      }
    }

    const monitorMix = getMixes().find((m) => m.ep?.displayName.toLowerCase().includes("monitor"));
    if (monitorMix) {
      const monitorKey = JSON.stringify(monitorMix.desc);
      if (!mixOutputs[monitorKey]) {
        const defaultName = graph.defaultSinkName;
        const defaultDev = defaultName ? allDevs.find((d) => d.deviceId === defaultName) : null;
        const autoDeviceId = defaultDev?.deviceId ?? allDevs[0]?.deviceId;
        if (autoDeviceId) {
          setMixOutputs(monitorKey, autoDeviceId);
          if ("channel" in monitorMix.desc) {
            const dev = allDevs.find((d) => d.deviceId === autoDeviceId);
            if (dev) {
              send({
                type: "setMixOutput",
                channel: monitorMix.desc.channel,
                outputNodeId: dev.pwNodeId ?? null,
              });
            }
          }
        }
      }
    }

    outputsInitialized = true;
  });

  function setMixOutput(mixKey: string, deviceId: string | null) {
    if (deviceId) {
      for (const [key, val] of Object.entries(mixOutputs)) {
        if (val === deviceId && key !== mixKey) setMixOutputs(key, null);
      }
    }
    setMixOutputs(mixKey, deviceId);

    const desc: EndpointDescriptor = JSON.parse(mixKey);
    if ("channel" in desc) {
      const graph = getGraph();
      const allDevs = getOutputDevices(graph.devices, graph.nodes);
      const dev = deviceId ? allDevs.find((d) => d.deviceId === deviceId) : null;
      send({ type: "setMixOutput", channel: desc.channel, outputNodeId: dev?.pwNodeId ?? null });
    }
  }

  const usedDeviceIds = () => {
    const ids = new Set<string>();
    for (const val of Object.values(mixOutputs)) {
      if (val) ids.add(val);
    }
    return ids;
  };

  return { mixOutputs, setMixOutput, usedDeviceIds };
}
