/**
 * View-model hook for the mixer grid.
 * Extracts channel/mix filtering, ordering, peak lookups, and order persistence
 * out of Mixer.tsx so the component only handles rendering.
 */
import { createEffect, createMemo, createSignal } from "solid-js";
import { useSession } from "../stores/sessionStore";
import { findEndpoint, findLink, getMixColor, descriptorKey } from "../components/mixerUtils";
import type { Endpoint, EndpointDescriptor } from "../types/session";

export type EndpointEntry = { desc: EndpointDescriptor; ep: Endpoint };

export { getMixColor, findEndpoint, findLink };

function applyOrder(items: EndpointEntry[], order: EndpointDescriptor[]): EndpointEntry[] {
  if (order.length === 0) return items;
  const orderKeys = order.map(descriptorKey);
  const byKey = new Map(items.map((item) => [descriptorKey(item.desc), item]));
  const ordered: EndpointEntry[] = [];
  for (const key of orderKeys) {
    const item = byKey.get(key);
    if (item) {
      ordered.push(item);
      byKey.delete(key);
    }
  }
  for (const item of byKey.values()) ordered.push(item);
  return ordered;
}

export interface MixerViewModel {
  /** Ordered visible source channels (non-sink). */
  channels: () => EndpointEntry[];
  /** Hidden channels (visible === false). */
  hiddenChannels: () => EndpointEntry[];
  /** Ordered visible mix destinations. */
  mixes: () => EndpointEntry[];
  /** Serialise a descriptor to a stable string key for keyed loops. */
  descKey: (d: EndpointDescriptor) => string;
  /** Persist a new channel order to backend. */
  persistChannelOrder: (reordered: EndpointEntry[]) => void;
  /** Persist a new mix order to backend. */
  persistMixOrder: (reordered: EndpointEntry[]) => void;
}

export function useMixerViewModel(): MixerViewModel {
  const { state, send } = useSession();

  // ── helpers ────────────────────────────────────────────────────────────────

  function channelKind(desc: EndpointDescriptor): string | undefined {
    if (!("channel" in desc)) return undefined;
    return state.session.channels[desc.channel]?.kind;
  }

  // ── raw lists ───────────────────────────────────────────────────────────────

  const rawChannels = () =>
    state.session.endpoints
      .filter(([desc, ep]) => "channel" in desc && channelKind(desc) !== "sink" && ep.visible)
      .map(([desc, ep]) => ({ desc, ep }));

  const sinkChannels = () =>
    state.session.endpoints
      .filter(([desc, ep]) => "channel" in desc && channelKind(desc) === "sink" && ep.visible)
      .map(([desc, ep]) => ({ desc, ep }));

  const rawMixes = () => {
    const fromSinks = state.session.activeSinks
      .map((desc) => ({ desc, ep: findEndpoint(state.session.endpoints, desc) }))
      .filter((m) => m.ep?.visible !== false);
    if (fromSinks.length > 0) return fromSinks;
    return sinkChannels();
  };

  // ── local order signals ─────────────────────────────────────────────────────

  const [localChannelOrder, setLocalChannelOrder] = createSignal<EndpointDescriptor[]>([]);
  const [localMixOrder, setLocalMixOrder] = createSignal<EndpointDescriptor[]>([]);
  let channelOrderInitialized = false;
  let mixOrderInitialized = false;

  createEffect(() => {
    const backendOrder = state.session.channelOrder;
    if (!channelOrderInitialized && backendOrder.length > 0) {
      channelOrderInitialized = true;
      setLocalChannelOrder(backendOrder);
    }
  });

  createEffect(() => {
    const backendOrder = state.session.mixOrder;
    if (!mixOrderInitialized && backendOrder.length > 0) {
      mixOrderInitialized = true;
      setLocalMixOrder(backendOrder);
    }
  });

  // ── ordered memos ───────────────────────────────────────────────────────────

  const channels = createMemo(() => applyOrder(rawChannels(), localChannelOrder()));

  const mixes = createMemo(() =>
    applyOrder(
      rawMixes().filter((m): m is EndpointEntry => m.ep != null),
      localMixOrder(),
    ),
  );

  // ── order persistence ───────────────────────────────────────────────────────

  function persistChannelOrder(reordered: EndpointEntry[]) {
    const order = reordered.map((item) => item.desc);
    setLocalChannelOrder(order);
    send({ type: "setChannelOrder", order });
  }

  function persistMixOrder(reordered: EndpointEntry[]) {
    const order = reordered.map((item) => item.desc);
    setLocalMixOrder(order);
    send({ type: "setMixOrder", order });
  }

  const hiddenChannels = createMemo(() =>
    rawChannels().filter((ch) => ch.ep?.visible === false),
  );

  return {
    channels,
    mixes,
    hiddenChannels,
    descKey: descriptorKey,
    persistChannelOrder,
    persistMixOrder,
  };
}
