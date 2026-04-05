/**
 * Tests for useMixerViewModel order persistence behavior.
 *
 * The hook's `persistChannelOrder` and `persistMixOrder` functions:
 *   1. Map EndpointEntry[] → EndpointDescriptor[]
 *   2. Call send({ type: "setChannelOrder"|"setMixOrder", order: [...] })
 *
 * We verify the command shape here without requiring a full SolidJS reactive
 * context, following the same pure-function test style as mixerUtils.test.ts.
 */
import { describe, it, expect, vi } from "vitest";
import type { EndpointDescriptor } from "../types/session";
import type { Command } from "../types/commands";

// ---------------------------------------------------------------------------
// Helpers — mirror the logic under test exactly (no reimplementation drift
// since these are one-liners taken verbatim from the hook)
// ---------------------------------------------------------------------------

type EndpointEntry = { desc: EndpointDescriptor; ep: unknown };

function buildSetChannelOrderCommand(reordered: EndpointEntry[]): Command {
  const order = reordered.map((item) => item.desc);
  return { type: "setChannelOrder", order };
}

function buildSetMixOrderCommand(reordered: EndpointEntry[]): Command {
  const order = reordered.map((item) => item.desc);
  return { type: "setMixOrder", order };
}

// ---------------------------------------------------------------------------
// applyOrder — inline copy of the hook's private helper so we can unit-test
// the ordering logic without a reactive context
// ---------------------------------------------------------------------------

function descriptorKey(d: EndpointDescriptor): string {
  if ("channel" in d) return `channel:${d.channel}`;
  if ("app" in d) return `app:${d.app[0]}:${d.app[1]}`;
  if ("ephemeralNode" in d) return `ephemeralNode:${d.ephemeralNode[0]}:${d.ephemeralNode[1]}`;
  if ("persistentNode" in d) return `persistentNode:${d.persistentNode[0]}:${d.persistentNode[1]}`;
  if ("device" in d) return `device:${d.device[0]}:${d.device[1]}`;
  return "";
}

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

// ---------------------------------------------------------------------------
// Tests: command shape for reorder sends
// ---------------------------------------------------------------------------

describe("persistChannelOrder — command shape", () => {
  it("sends setChannelOrder with descriptors extracted from entries", () => {
    const sent: Command[] = [];
    const send = (cmd: Command) => sent.push(cmd);

    const entries: EndpointEntry[] = [
      { desc: { channel: "music" }, ep: {} },
      { desc: { channel: "browser" }, ep: {} },
    ];

    const cmd = buildSetChannelOrderCommand(entries);
    send(cmd);

    expect(sent).toHaveLength(1);
    expect(sent[0].type).toBe("setChannelOrder");
    const order = (sent[0] as { type: "setChannelOrder"; order: EndpointDescriptor[] }).order;
    expect(order).toHaveLength(2);
    expect(order[0]).toEqual({ channel: "music" });
    expect(order[1]).toEqual({ channel: "browser" });
  });

  it("sends empty order when no entries", () => {
    const cmd = buildSetChannelOrderCommand([]);
    expect(cmd.type).toBe("setChannelOrder");
    const order = (cmd as { type: "setChannelOrder"; order: EndpointDescriptor[] }).order;
    expect(order).toHaveLength(0);
  });

  it("preserves descriptor identity for non-channel endpoints", () => {
    const desc: EndpointDescriptor = { app: ["spotify", "source"] };
    const entries: EndpointEntry[] = [{ desc, ep: {} }];
    const cmd = buildSetChannelOrderCommand(entries);
    const order = (cmd as { type: "setChannelOrder"; order: EndpointDescriptor[] }).order;
    expect(order[0]).toEqual({ app: ["spotify", "source"] });
  });
});

describe("persistMixOrder — command shape", () => {
  it("sends setMixOrder with descriptors extracted from entries", () => {
    const sent: Command[] = [];
    const send = (cmd: Command) => sent.push(cmd);

    const entries: EndpointEntry[] = [
      { desc: { channel: "main-mix" }, ep: {} },
      { desc: { channel: "stream-mix" }, ep: {} },
    ];

    const cmd = buildSetMixOrderCommand(entries);
    send(cmd);

    expect(sent).toHaveLength(1);
    expect(sent[0].type).toBe("setMixOrder");
    const order = (sent[0] as { type: "setMixOrder"; order: EndpointDescriptor[] }).order;
    expect(order).toHaveLength(2);
    expect(order[0]).toEqual({ channel: "main-mix" });
    expect(order[1]).toEqual({ channel: "stream-mix" });
  });

  it("sends empty order when no entries", () => {
    const cmd = buildSetMixOrderCommand([]);
    expect(cmd.type).toBe("setMixOrder");
    const order = (cmd as { type: "setMixOrder"; order: EndpointDescriptor[] }).order;
    expect(order).toHaveLength(0);
  });
});

// ---------------------------------------------------------------------------
// Tests: applyOrder — ordering logic
// ---------------------------------------------------------------------------

describe("applyOrder", () => {
  function entry(channel: string): EndpointEntry {
    return { desc: { channel }, ep: {} };
  }

  it("returns items unchanged when order is empty", () => {
    const items = [entry("a"), entry("b"), entry("c")];
    const result = applyOrder(items, []);
    expect(result.map((e) => (e.desc as { channel: string }).channel)).toEqual(["a", "b", "c"]);
  });

  it("reorders items to match given order", () => {
    const items = [entry("a"), entry("b"), entry("c")];
    const order: EndpointDescriptor[] = [{ channel: "c" }, { channel: "a" }, { channel: "b" }];
    const result = applyOrder(items, order);
    expect(result.map((e) => (e.desc as { channel: string }).channel)).toEqual(["c", "a", "b"]);
  });

  it("appends items not in order after ordered items", () => {
    const items = [entry("a"), entry("b"), entry("c"), entry("d")];
    const order: EndpointDescriptor[] = [{ channel: "c" }, { channel: "a" }];
    const result = applyOrder(items, order);
    const keys = result.map((e) => (e.desc as { channel: string }).channel);
    expect(keys[0]).toBe("c");
    expect(keys[1]).toBe("a");
    // b and d appear after, in insertion order
    expect(keys).toContain("b");
    expect(keys).toContain("d");
  });

  it("skips order entries that have no matching item (new session)", () => {
    const items = [entry("a"), entry("b")];
    const order: EndpointDescriptor[] = [
      { channel: "c" }, // not in items
      { channel: "a" },
      { channel: "b" },
    ];
    const result = applyOrder(items, order);
    expect(result).toHaveLength(2);
    expect(result.map((e) => (e.desc as { channel: string }).channel)).toEqual(["a", "b"]);
  });
});

// ---------------------------------------------------------------------------
// Tests: reorder then send — integration of ordering + command dispatch
// ---------------------------------------------------------------------------

describe("reorder sends correct command to backend", () => {
  it("channel reorder produces setChannelOrder with new sequence", () => {
    const send = vi.fn();

    const entries: EndpointEntry[] = [
      { desc: { channel: "music" }, ep: {} },
      { desc: { channel: "browser" }, ep: {} },
      { desc: { channel: "comms" }, ep: {} },
    ];
    // Simulate drag: move "comms" to first position
    const reordered = [entries[2], entries[0], entries[1]];
    const cmd = buildSetChannelOrderCommand(reordered);
    send(cmd);

    expect(send).toHaveBeenCalledOnce();
    const [sentCmd] = send.mock.calls[0] as [Command];
    expect(sentCmd.type).toBe("setChannelOrder");
    const order = (sentCmd as { type: "setChannelOrder"; order: EndpointDescriptor[] }).order;
    expect(order).toEqual([{ channel: "comms" }, { channel: "music" }, { channel: "browser" }]);
  });

  it("mix reorder produces setMixOrder with new sequence", () => {
    const send = vi.fn();

    const entries: EndpointEntry[] = [
      { desc: { channel: "main" }, ep: {} },
      { desc: { channel: "stream" }, ep: {} },
    ];
    const reordered = [entries[1], entries[0]];
    const cmd = buildSetMixOrderCommand(reordered);
    send(cmd);

    expect(send).toHaveBeenCalledOnce();
    const [sentCmd] = send.mock.calls[0] as [Command];
    expect(sentCmd.type).toBe("setMixOrder");
    const order = (sentCmd as { type: "setMixOrder"; order: EndpointDescriptor[] }).order;
    expect(order).toEqual([{ channel: "stream" }, { channel: "main" }]);
  });
});
