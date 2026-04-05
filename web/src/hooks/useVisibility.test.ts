/**
 * Tests for hide/show + disable/enable channel commands (F6).
 *
 * Following the same pure-function style as useMixerViewModel.test.ts:
 * tests verify command shapes and filtering logic without a reactive context.
 */
import { describe, it, expect } from "vitest";
import type { Endpoint, EndpointDescriptor } from "../types/session";
import type { Command } from "../types/commands";

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

function makeEndpoint(overrides: Partial<Endpoint> = {}): Endpoint {
  return {
    descriptor: { channel: "test-id" },
    isPlaceholder: false,
    displayName: "Test Channel",
    customName: null,
    iconName: "speaker",
    details: [],
    volume: 1.0,
    volumeLeft: 1.0,
    volumeRight: 1.0,
    volumeMixed: false,
    volumeLockedMuted: "unmutedUnlocked",
    visible: true,
    disabled: false,
    ...overrides,
  };
}

type EndpointEntry = { desc: EndpointDescriptor; ep: Endpoint };

function makeEntry(channel: string, overrides: Partial<Endpoint> = {}): EndpointEntry {
  return { desc: { channel }, ep: makeEndpoint(overrides) };
}

/** Mirror of the rawChannels filter in useMixerViewModel. */
function filterVisible(entries: EndpointEntry[]): EndpointEntry[] {
  return entries.filter(({ ep }) => ep.visible);
}

/** Mirror of the rawHiddenChannels filter in useMixerViewModel. */
function filterHidden(entries: EndpointEntry[]): EndpointEntry[] {
  return entries.filter(({ ep }) => !ep.visible);
}

// ---------------------------------------------------------------------------
// Command shape — setEndpointVisible
// ---------------------------------------------------------------------------

describe("setEndpointVisible command shape", () => {
  it("hide produces {type: setEndpointVisible, visible: false}", () => {
    const desc: EndpointDescriptor = { channel: "music" };
    const cmd: Command = { type: "setEndpointVisible", endpoint: desc, visible: false };
    expect(cmd.type).toBe("setEndpointVisible");
    expect(
      (cmd as { type: "setEndpointVisible"; endpoint: EndpointDescriptor; visible: boolean })
        .visible,
    ).toBe(false);
  });

  it("show produces {type: setEndpointVisible, visible: true}", () => {
    const desc: EndpointDescriptor = { channel: "music" };
    const cmd: Command = { type: "setEndpointVisible", endpoint: desc, visible: true };
    expect(
      (cmd as { type: "setEndpointVisible"; endpoint: EndpointDescriptor; visible: boolean })
        .visible,
    ).toBe(true);
  });

  it("endpoint is preserved in command", () => {
    const desc: EndpointDescriptor = { channel: "browser" };
    const cmd: Command = { type: "setEndpointVisible", endpoint: desc, visible: false };
    const typed = cmd as {
      type: "setEndpointVisible";
      endpoint: EndpointDescriptor;
      visible: boolean;
    };
    expect(typed.endpoint).toEqual({ channel: "browser" });
  });
});

// ---------------------------------------------------------------------------
// Command shape — setEndpointDisabled
// ---------------------------------------------------------------------------

describe("setEndpointDisabled command shape", () => {
  it("disable produces {type: setEndpointDisabled, disabled: true}", () => {
    const desc: EndpointDescriptor = { channel: "music" };
    const cmd: Command = { type: "setEndpointDisabled", endpoint: desc, disabled: true };
    expect(cmd.type).toBe("setEndpointDisabled");
    const typed = cmd as {
      type: "setEndpointDisabled";
      endpoint: EndpointDescriptor;
      disabled: boolean;
    };
    expect(typed.disabled).toBe(true);
  });

  it("re-enable produces {type: setEndpointDisabled, disabled: false}", () => {
    const desc: EndpointDescriptor = { channel: "music" };
    const cmd: Command = { type: "setEndpointDisabled", endpoint: desc, disabled: false };
    const typed = cmd as {
      type: "setEndpointDisabled";
      endpoint: EndpointDescriptor;
      disabled: boolean;
    };
    expect(typed.disabled).toBe(false);
  });

  it("endpoint is preserved in command", () => {
    const desc: EndpointDescriptor = { channel: "game" };
    const cmd: Command = { type: "setEndpointDisabled", endpoint: desc, disabled: true };
    const typed = cmd as {
      type: "setEndpointDisabled";
      endpoint: EndpointDescriptor;
      disabled: boolean;
    };
    expect(typed.endpoint).toEqual({ channel: "game" });
  });
});

// ---------------------------------------------------------------------------
// Filtering — visible/hidden channel lists (mirrors useMixerViewModel logic)
// ---------------------------------------------------------------------------

describe("channel visibility filtering", () => {
  it("visible channels are included in main grid list", () => {
    const entries = [
      makeEntry("music", { visible: true }),
      makeEntry("browser", { visible: true }),
    ];
    expect(filterVisible(entries)).toHaveLength(2);
  });

  it("hidden channels are excluded from main grid list", () => {
    const entries = [
      makeEntry("music", { visible: true }),
      makeEntry("browser", { visible: false }),
    ];
    expect(filterVisible(entries)).toHaveLength(1);
    expect((filterVisible(entries)[0].desc as { channel: string }).channel).toBe("music");
  });

  it("hidden channels appear in the hidden list", () => {
    const entries = [
      makeEntry("music", { visible: true }),
      makeEntry("browser", { visible: false }),
    ];
    const hidden = filterHidden(entries);
    expect(hidden).toHaveLength(1);
    expect((hidden[0].desc as { channel: string }).channel).toBe("browser");
  });

  it("all hidden — main grid is empty, hidden list has all", () => {
    const entries = [makeEntry("music", { visible: false }), makeEntry("game", { visible: false })];
    expect(filterVisible(entries)).toHaveLength(0);
    expect(filterHidden(entries)).toHaveLength(2);
  });

  it("none hidden — hidden list is empty", () => {
    const entries = [makeEntry("music", { visible: true }), makeEntry("game", { visible: true })];
    expect(filterHidden(entries)).toHaveLength(0);
  });
});

// ---------------------------------------------------------------------------
// Disabled state — independent from visible
// ---------------------------------------------------------------------------

describe("disabled state is independent from visible", () => {
  it("channel can be visible and disabled simultaneously", () => {
    const ep = makeEndpoint({ visible: true, disabled: true });
    expect(ep.visible).toBe(true);
    expect(ep.disabled).toBe(true);
  });

  it("channel can be hidden and disabled simultaneously", () => {
    const ep = makeEndpoint({ visible: false, disabled: true });
    expect(ep.visible).toBe(false);
    expect(ep.disabled).toBe(true);
  });

  it("hiding a channel does not affect disabled field in Endpoint type", () => {
    const ep = makeEndpoint({ visible: true, disabled: false });
    // Simulate what the backend does: visible=false, disabled stays false
    const updated = { ...ep, visible: false };
    expect(updated.visible).toBe(false);
    expect(updated.disabled).toBe(false);
  });

  it("disabling does not change visible field", () => {
    const ep = makeEndpoint({ visible: true, disabled: false });
    const updated = { ...ep, disabled: true };
    expect(updated.visible).toBe(true);
    expect(updated.disabled).toBe(true);
  });
});
