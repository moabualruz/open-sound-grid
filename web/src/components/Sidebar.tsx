/**
 * Collapsible sidebar navigation.
 *
 * Sections:
 *   - Devices: hardware inputs from AudioGraph
 *   - Mixes & Effects: Mixes (matrix view), Analyzer (spectrum page)
 *   - Settings: opens settings panel (pinned to bottom)
 *
 * 200px expanded, 48px icon-only when collapsed.
 * Active nav item shows a left-border accent bar.
 */
import { For, Show, createSignal } from "solid-js";
import type { JSX } from "solid-js";
import type { PwDevice } from "../types/graph";
import {
  Activity,
  ChevronLeft,
  ChevronRight,
  Grid2x2,
  Headphones,
  Settings,
} from "lucide-solid";

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

interface NavItem {
  id: string;
  label: string;
  hash: string;
  icon: (size: number) => JSX.Element;
  ariaLabel: string;
}

interface DeviceEntry {
  id: string;
  name: string;
}

export interface SidebarProps {
  currentHash: string;
  devices: Record<string, PwDevice>;
  onOpenSettings: () => void;
}

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

const EXPANDED_WIDTH = "200px";
const COLLAPSED_WIDTH = "48px";

const MIXES_EFFECTS_ITEMS: NavItem[] = [
  {
    id: "mixer",
    label: "Mixes",
    hash: "",
    icon: (s) => Grid2x2({ size: s }),
    ariaLabel: "Mixer matrix view",
  },
  {
    id: "analyzer",
    label: "Analyzer",
    hash: "#analyzer",
    icon: (s) => Activity({ size: s }),
    ariaLabel: "Spectrum Analyzer",
  },
];

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

function deriveDevices(devices: Record<string, PwDevice>): DeviceEntry[] {
  return Object.values(devices).map((d) => ({
    id: String(d.id),
    name: d.name,
  }));
}

function isActive(currentHash: string, itemHash: string): boolean {
  return currentHash === itemHash;
}

// ---------------------------------------------------------------------------
// Sub-components
// ---------------------------------------------------------------------------

function SectionHeader(props: { label: string; collapsed: boolean }) {
  return (
    <Show when={!props.collapsed}>
      <span
        class="mb-1 block px-3 text-[10px] font-semibold uppercase tracking-wider"
        style={{ color: "var(--color-text-muted)" }}
      >
        {props.label}
      </span>
    </Show>
  );
}

function NavLink(props: {
  item: NavItem;
  active: boolean;
  collapsed: boolean;
}) {
  const iconSize = () => (props.collapsed ? 18 : 16);
  return (
    <a
      href={props.item.hash === "" ? "#" : props.item.hash}
      class="relative flex items-center gap-2 rounded px-3 py-1.5 text-xs font-medium transition-colors"
      style={{
        color: props.active ? "var(--color-accent)" : "var(--color-text-muted)",
        "background-color": props.active ? "var(--color-bg-hover)" : "transparent",
        "text-decoration": "none",
        "justify-content": props.collapsed ? "center" : "flex-start",
      }}
      aria-label={props.item.ariaLabel}
      aria-current={props.active ? "page" : undefined}
      title={props.collapsed ? props.item.label : undefined}
    >
      {/* Left accent bar */}
      <Show when={props.active}>
        <span
          class="absolute left-0 top-1 bottom-1 w-[3px] rounded-r"
          style={{ "background-color": "var(--color-accent)" }}
        />
      </Show>
      {props.item.icon(iconSize())}
      <Show when={!props.collapsed}>
        <span>{props.item.label}</span>
      </Show>
    </a>
  );
}

function DeviceItem(props: { device: DeviceEntry; collapsed: boolean }) {
  return (
    <div
      class="flex items-center gap-2 rounded px-3 py-1 text-xs transition-colors"
      style={{
        color: "var(--color-text-secondary)",
        "justify-content": props.collapsed ? "center" : "flex-start",
      }}
      title={props.collapsed ? props.device.name : undefined}
    >
      <Headphones size={props.collapsed ? 16 : 14} />
      <Show when={!props.collapsed}>
        <span class="truncate">{props.device.name}</span>
      </Show>
    </div>
  );
}

// ---------------------------------------------------------------------------
// Sidebar
// ---------------------------------------------------------------------------

export default function Sidebar(props: SidebarProps) {
  const [collapsed, setCollapsed] = createSignal(false);
  const devices = () => deriveDevices(props.devices);

  return (
    <nav
      class="flex flex-col border-r"
      style={{
        "background-color": "var(--color-bg-secondary)",
        "border-color": "var(--color-border)",
        width: collapsed() ? COLLAPSED_WIDTH : EXPANDED_WIDTH,
        "min-width": collapsed() ? COLLAPSED_WIDTH : EXPANDED_WIDTH,
        transition: "width 150ms ease, min-width 150ms ease",
      }}
      aria-label="Main navigation"
    >
      {/* Collapse toggle */}
      <div
        class="flex items-center border-b px-2 py-2"
        style={{
          "border-color": "var(--color-border)",
          "justify-content": collapsed() ? "center" : "flex-end",
        }}
      >
        <button
          class="flex items-center justify-center rounded p-1 transition-colors"
          style={{ color: "var(--color-text-muted)" }}
          onClick={() => setCollapsed((v) => !v)}
          aria-label={collapsed() ? "Expand sidebar" : "Collapse sidebar"}
          title={collapsed() ? "Expand sidebar" : "Collapse sidebar"}
        >
          <Show when={collapsed()} fallback={<ChevronLeft size={16} />}>
            <ChevronRight size={16} />
          </Show>
        </button>
      </div>

      {/* Scrollable nav content */}
      <div class="flex flex-1 flex-col gap-3 overflow-y-auto py-2">
        {/* Devices section */}
        <div>
          <SectionHeader label="Devices" collapsed={collapsed()} />
          <Show
            when={devices().length > 0}
            fallback={
              <Show when={!collapsed()}>
                <span
                  class="block px-3 text-[11px] italic"
                  style={{ color: "var(--color-text-muted)" }}
                >
                  No devices
                </span>
              </Show>
            }
          >
            <div class="flex flex-col gap-0.5">
              <For each={devices()}>
                {(device) => <DeviceItem device={device} collapsed={collapsed()} />}
              </For>
            </div>
          </Show>
        </div>

        {/* Mixes & Effects section */}
        <div>
          <SectionHeader label="Mixes & Effects" collapsed={collapsed()} />
          <div class="flex flex-col gap-0.5">
            <For each={MIXES_EFFECTS_ITEMS}>
              {(item) => (
                <NavLink
                  item={item}
                  active={isActive(props.currentHash, item.hash)}
                  collapsed={collapsed()}
                />
              )}
            </For>
          </div>
        </div>
      </div>

      {/* Settings pinned to bottom */}
      <div class="border-t py-2" style={{ "border-color": "var(--color-border)" }}>
        <button
          class="flex w-full items-center gap-2 rounded px-3 py-1.5 text-xs font-medium transition-colors"
          style={{
            color: "var(--color-text-muted)",
            background: "none",
            border: "none",
            cursor: "pointer",
            "justify-content": collapsed() ? "center" : "flex-start",
          }}
          onClick={() => props.onOpenSettings()}
          aria-label="Settings"
          title={collapsed() ? "Settings" : undefined}
        >
          <Settings size={collapsed() ? 18 : 16} />
          <Show when={!collapsed()}>
            <span>Settings</span>
          </Show>
        </button>
      </div>
    </nav>
  );
}
