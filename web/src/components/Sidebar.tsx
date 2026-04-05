/**
 * Sidebar navigation.
 * Currently exposes: Mixer (home), Analyzer.
 * Uses hash-based routing consistent with App.tsx.
 */
import { Activity, Grid2x2 } from "lucide-solid";

interface NavItem {
  label: string;
  hash: string;
  icon: () => unknown;
  ariaLabel: string;
}

const NAV_ITEMS: NavItem[] = [
  {
    label: "Mixer",
    hash: "",
    icon: () => Grid2x2({ size: 16 }),
    ariaLabel: "Mixer",
  },
  {
    label: "Analyzer",
    hash: "#analyzer",
    icon: () => Activity({ size: 16 }),
    ariaLabel: "Spectrum Analyzer",
  },
];

interface SidebarProps {
  currentHash: string;
}

export default function Sidebar(props: SidebarProps) {
  return (
    <nav
      class="flex flex-col gap-1 p-2 border-r"
      style={{
        "background-color": "var(--color-bg-secondary)",
        "border-color": "var(--color-border)",
        width: "3.25rem",
      }}
      aria-label="Main navigation"
    >
      {NAV_ITEMS.map((item) => {
        const isActive = () => props.currentHash === item.hash;
        return (
          <a
            href={item.hash === "" ? "#" : item.hash}
            class="flex flex-col items-center gap-1 rounded p-2 text-[9px] font-medium transition-colors"
            style={{
              color: isActive() ? "var(--color-accent)" : "var(--color-text-muted)",
              "background-color": isActive() ? "var(--color-bg-hover)" : "transparent",
              "text-decoration": "none",
            }}
            aria-label={item.ariaLabel}
            aria-current={isActive() ? "page" : undefined}
          >
            {item.icon()}
            <span>{item.label}</span>
          </a>
        );
      })}
    </nav>
  );
}
