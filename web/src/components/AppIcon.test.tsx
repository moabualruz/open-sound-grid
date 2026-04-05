import { describe, it, expect, vi } from "vitest";
import { render } from "@solidjs/testing-library";
import AppIcon, { normalizeAppName, findIconKey } from "./AppIcon";

// ---------------------------------------------------------------------------
// normalizeAppName
// ---------------------------------------------------------------------------

describe("normalizeAppName", () => {
  it("lowercases the name", () => {
    expect(normalizeAppName("Firefox")).toBe("firefox");
  });

  it("strips org.mozilla. reverse-DNS prefix", () => {
    expect(normalizeAppName("org.mozilla.firefox")).toBe("firefox");
  });

  it("strips com. reverse-DNS prefix", () => {
    expect(normalizeAppName("com.valvesoftware.steam")).toBe("steam");
  });

  it("strips version suffixes", () => {
    expect(normalizeAppName("firefox-120")).toBe("firefox");
  });

  it("replaces underscores with hyphens", () => {
    expect(normalizeAppName("obs_studio")).toBe("obs-studio");
  });

  it("trims whitespace", () => {
    expect(normalizeAppName("  vlc  ")).toBe("vlc");
  });
});

// ---------------------------------------------------------------------------
// findIconKey (fuzzy matching)
// ---------------------------------------------------------------------------

describe("findIconKey", () => {
  it("finds exact match for firefox", () => {
    expect(findIconKey("firefox")).toBe("firefox");
  });

  it("finds chrome via alias", () => {
    const key = findIconKey("chrome");
    expect(key).not.toBeNull();
  });

  it("returns null for completely unknown app", () => {
    expect(findIconKey("this-app-xyzzy-unknown-12345")).toBeNull();
  });

  it("matches org.mozilla.firefox after normalization", () => {
    const normalized = normalizeAppName("org.mozilla.firefox");
    const key = findIconKey(normalized);
    expect(key).toBe("firefox");
  });

  it("finds spotify", () => {
    expect(findIconKey("spotify")).toBe("spotify");
  });

  it("finds discord", () => {
    expect(findIconKey("discord")).toBe("discord");
  });
});

// ---------------------------------------------------------------------------
// AppIcon component rendering
// ---------------------------------------------------------------------------

describe("AppIcon", () => {
  it("renders a span with SVG content for a known bundled name", () => {
    const { container } = render(() => <AppIcon name="firefox" />);
    const span = container.querySelector("span");
    expect(span).toBeTruthy();
    // Inner HTML should contain svg markup
    expect(span?.innerHTML).toContain("<svg");
  });

  it("renders an img tag for an unknown name (tier 2)", () => {
    const { container } = render(() => (
      <AppIcon name="this-app-xyzzy-unknown-12345" />
    ));
    const img = container.querySelector("img");
    expect(img).toBeTruthy();
    expect(img?.getAttribute("src")).toContain("/api/icons/");
  });

  it("renders letter avatar after img onError for unknown name", async () => {
    const { container } = render(() => (
      <AppIcon name="this-app-xyzzy-unknown-12345" />
    ));
    const img = container.querySelector("img");
    // Simulate load failure
    img?.dispatchEvent(new Event("error"));
    // After error, should show letter avatar SVG
    await new Promise((r) => setTimeout(r, 0));
    const svgEl = container.querySelector("svg");
    expect(svgEl).toBeTruthy();
  });

  it("uses correct size attribute", () => {
    const { container } = render(() => <AppIcon name="spotify" size={48} />);
    const span = container.querySelector("span");
    expect(span?.style.width).toBe("48px");
    expect(span?.style.height).toBe("48px");
  });

  it("defaults to size 32 when no size prop given", () => {
    const { container } = render(() => <AppIcon name="vlc" />);
    const span = container.querySelector("span");
    expect(span?.style.width).toBe("32px");
  });
});
