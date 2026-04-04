import { describe, it, expect } from "vitest";
import { descriptorsEqual, descriptorKey } from "./mixerUtils";
import type { EndpointDescriptor } from "../types/session";

describe("descriptorsEqual", () => {
  it("matches identical channel descriptors", () => {
    const a: EndpointDescriptor = { channel: "music" };
    const b: EndpointDescriptor = { channel: "music" };
    expect(descriptorsEqual(a, b)).toBe(true);
  });

  it("rejects different channel descriptors", () => {
    const a: EndpointDescriptor = { channel: "music" };
    const b: EndpointDescriptor = { channel: "browser" };
    expect(descriptorsEqual(a, b)).toBe(false);
  });

  it("rejects descriptors of different variants", () => {
    const a: EndpointDescriptor = { channel: "music" };
    const b: EndpointDescriptor = { app: ["music", "source"] };
    expect(descriptorsEqual(a, b)).toBe(false);
  });

  it("matches identical app descriptors", () => {
    const a: EndpointDescriptor = { app: ["spotify", "source"] };
    const b: EndpointDescriptor = { app: ["spotify", "source"] };
    expect(descriptorsEqual(a, b)).toBe(true);
  });

  it("rejects app descriptors with different port kinds", () => {
    const a: EndpointDescriptor = { app: ["spotify", "source"] };
    const b: EndpointDescriptor = { app: ["spotify", "sink"] };
    expect(descriptorsEqual(a, b)).toBe(false);
  });

  it("matches identical ephemeralNode descriptors", () => {
    const a: EndpointDescriptor = { ephemeralNode: [42, "source"] };
    const b: EndpointDescriptor = { ephemeralNode: [42, "source"] };
    expect(descriptorsEqual(a, b)).toBe(true);
  });

  it("rejects ephemeralNode descriptors with different ids", () => {
    const a: EndpointDescriptor = { ephemeralNode: [42, "source"] };
    const b: EndpointDescriptor = { ephemeralNode: [99, "source"] };
    expect(descriptorsEqual(a, b)).toBe(false);
  });

  it("matches identical persistentNode descriptors", () => {
    const a: EndpointDescriptor = { persistentNode: ["hw:0", "sink"] };
    const b: EndpointDescriptor = { persistentNode: ["hw:0", "sink"] };
    expect(descriptorsEqual(a, b)).toBe(true);
  });

  it("matches identical device descriptors", () => {
    const a: EndpointDescriptor = { device: ["alsa_output.pci", "sink"] };
    const b: EndpointDescriptor = { device: ["alsa_output.pci", "sink"] };
    expect(descriptorsEqual(a, b)).toBe(true);
  });
});

describe("descriptorKey", () => {
  it("produces stable string key for channel", () => {
    const d: EndpointDescriptor = { channel: "music" };
    expect(descriptorKey(d)).toBe("channel:music");
  });

  it("produces stable string key for app", () => {
    const d: EndpointDescriptor = { app: ["spotify", "source"] };
    expect(descriptorKey(d)).toBe("app:spotify:source");
  });

  it("produces stable string key for ephemeralNode", () => {
    const d: EndpointDescriptor = { ephemeralNode: [42, "sink"] };
    expect(descriptorKey(d)).toBe("ephemeralNode:42:sink");
  });

  it("produces stable string key for persistentNode", () => {
    const d: EndpointDescriptor = { persistentNode: ["hw:0", "source"] };
    expect(descriptorKey(d)).toBe("persistentNode:hw:0:source");
  });

  it("produces stable string key for device", () => {
    const d: EndpointDescriptor = { device: ["alsa_output.pci", "sink"] };
    expect(descriptorKey(d)).toBe("device:alsa_output.pci:sink");
  });

  it("two equal descriptors produce the same key", () => {
    const a: EndpointDescriptor = { channel: "browser" };
    const b: EndpointDescriptor = { channel: "browser" };
    expect(descriptorKey(a)).toBe(descriptorKey(b));
  });

  it("two different descriptors produce different keys", () => {
    const a: EndpointDescriptor = { channel: "music" };
    const b: EndpointDescriptor = { channel: "browser" };
    expect(descriptorKey(a)).not.toBe(descriptorKey(b));
  });
});
