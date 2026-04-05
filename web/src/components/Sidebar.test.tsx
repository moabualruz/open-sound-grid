/**
 * Tests for Sidebar — collapsible navigation with Devices, Mixes & Effects, Settings.
 */
import { describe, it, expect, vi } from "vitest";
import { render, fireEvent } from "@solidjs/testing-library";
import Sidebar from "./Sidebar";
import type { SidebarProps } from "./Sidebar";
import type { PwDevice } from "../types/graph";

function makeDevice(id: number, name: string): PwDevice {
  return {
    id,
    name,
    client: 0,
    nodes: [],
    activeRoutes: [],
  };
}

function renderSidebar(overrides: Partial<SidebarProps> = {}) {
  const props: SidebarProps = {
    currentHash: "",
    devices: {},
    onOpenSettings: vi.fn(),
    ...overrides,
  };
  return { ...render(() => <Sidebar {...props} />), props };
}

describe("Sidebar", () => {
  it("renders with nav landmark", () => {
    const { getByRole } = renderSidebar();
    expect(getByRole("navigation", { name: "Main navigation" })).toBeTruthy();
  });

  it("shows Mixes and Analyzer nav items", () => {
    const { getByText } = renderSidebar();
    expect(getByText("Mixes")).toBeTruthy();
    expect(getByText("Analyzer")).toBeTruthy();
  });

  it("shows Devices section header", () => {
    const { getByText } = renderSidebar();
    expect(getByText("Devices")).toBeTruthy();
  });

  it("shows Mixes & Effects section header", () => {
    const { getByText } = renderSidebar();
    expect(getByText("Mixes & Effects")).toBeTruthy();
  });

  it("shows Settings button", () => {
    const { getByRole } = renderSidebar();
    expect(getByRole("button", { name: "Settings" })).toBeTruthy();
  });

  it("lists hardware devices by name", () => {
    const devices: Record<string, PwDevice> = {
      "1": makeDevice(1, "Focusrite Scarlett 2i2"),
      "2": makeDevice(2, "Blue Yeti"),
    };
    const { getByText } = renderSidebar({ devices });
    expect(getByText("Focusrite Scarlett 2i2")).toBeTruthy();
    expect(getByText("Blue Yeti")).toBeTruthy();
  });

  it("shows 'No devices' when device list is empty", () => {
    const { getByText } = renderSidebar({ devices: {} });
    expect(getByText("No devices")).toBeTruthy();
  });

  it("marks active nav item with aria-current", () => {
    const { getByRole } = renderSidebar({ currentHash: "#analyzer" });
    const analyzerLink = getByRole("link", { name: "Spectrum Analyzer" });
    expect(analyzerLink.getAttribute("aria-current")).toBe("page");
  });

  it("does not mark inactive nav items with aria-current", () => {
    const { getByRole } = renderSidebar({ currentHash: "#analyzer" });
    const mixerLink = getByRole("link", { name: "Mixer matrix view" });
    expect(mixerLink.getAttribute("aria-current")).toBeNull();
  });

  it("collapses to icon-only mode on toggle", async () => {
    const { getByRole, queryByText } = renderSidebar();
    const toggle = getByRole("button", { name: "Collapse sidebar" });

    // Before collapse: section headers visible
    expect(queryByText("Devices")).toBeTruthy();

    await fireEvent.click(toggle);

    // After collapse: section headers hidden
    expect(queryByText("Devices")).toBeNull();
    expect(queryByText("Mixes & Effects")).toBeNull();
  });

  it("expands back from collapsed mode", async () => {
    const { getByRole, queryByText } = renderSidebar();

    // Collapse
    await fireEvent.click(getByRole("button", { name: "Collapse sidebar" }));
    expect(queryByText("Devices")).toBeNull();

    // Expand
    await fireEvent.click(getByRole("button", { name: "Expand sidebar" }));
    expect(queryByText("Devices")).toBeTruthy();
  });

  it("calls onOpenSettings when Settings button is clicked", async () => {
    const onOpenSettings = vi.fn();
    const { getByRole } = renderSidebar({ onOpenSettings });

    await fireEvent.click(getByRole("button", { name: "Settings" }));
    expect(onOpenSettings).toHaveBeenCalledOnce();
  });

  it("active nav item has accent bar element", () => {
    const { getByRole } = renderSidebar({ currentHash: "" });
    const mixerLink = getByRole("link", { name: "Mixer matrix view" });
    // The accent bar is a span child with absolute positioning
    const accentBar = mixerLink.querySelector("span.absolute");
    expect(accentBar).toBeTruthy();
  });
});
