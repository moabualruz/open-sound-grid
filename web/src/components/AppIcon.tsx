import { Show, createSignal, createMemo } from "solid-js";
import { ICON_PACK, ICON_KEYS } from "../assets/icons/index";

/**
 * Sanitize an SVG string by stripping dangerous elements and attributes.
 * Removes <script>, <foreignObject>, and event handler attributes (on*=).
 */
function sanitizeSvg(svg: string): string {
  return svg
    .replace(/<script[\s\S]*?<\/script>/gi, "")
    .replace(/<script[\s\S]*?\/>/gi, "")
    .replace(/<foreignObject[\s\S]*?<\/foreignObject>/gi, "")
    .replace(/<foreignObject[\s\S]*?\/>/gi, "")
    .replace(/\s+on\w+\s*=\s*(?:"[^"]*"|'[^']*'|[^\s>]+)/gi, "");
}

// 12-color palette for deterministic letter avatars
const AVATAR_COLORS = [
  "#e74c3c",
  "#e67e22",
  "#f1c40f",
  "#2ecc71",
  "#1abc9c",
  "#3498db",
  "#2980b9",
  "#9b59b6",
  "#8e44ad",
  "#e91e63",
  "#00bcd4",
  "#607d8b",
];

/** Deterministic color from app name */
function avatarColor(name: string): string {
  let hash = 0;
  for (let i = 0; i < name.length; i++) {
    hash = (hash * 31 + name.charCodeAt(i)) >>> 0;
  }
  return AVATAR_COLORS[hash % AVATAR_COLORS.length];
}

/**
 * Normalize an app name for icon lookup:
 * - lowercase
 * - strip reverse-DNS prefixes (org.mozilla., com.valvesoftware., etc.)
 * - strip version suffixes (-1.2.3, _v2, etc.)
 * - replace underscores/spaces with hyphens
 */
export function normalizeAppName(name: string): string {
  let n = name.toLowerCase().trim();
  // Strip reverse-DNS prefix: org.foo.Bar → bar
  n = n.replace(/^(?:org|com|net|io|app)\.[^.]+\./, "");
  // Strip version suffix: app-1.2.3 → app, app_v2 → app
  n = n.replace(/[-_]v?\d[\d.]*$/, "");
  // Normalize separators
  n = n.replace(/[_ ]+/g, "-");
  return n;
}

const FUZZY_THRESHOLD = 0.6;

/** Simple similarity: longest common prefix ratio + substring bonus */
function similarity(a: string, b: string): number {
  if (a === b) return 1;
  if (b.includes(a) || a.includes(b)) {
    const longer = Math.max(a.length, b.length);
    const shorter = Math.min(a.length, b.length);
    return 0.6 + 0.4 * (shorter / longer);
  }
  // Character overlap ratio
  const setA = new Set(a.split(""));
  const setB = new Set(b.split(""));
  const intersection = [...setA].filter((c) => setB.has(c)).length;
  const union = new Set([...setA, ...setB]).size;
  return intersection / union;
}

/** Find the best matching icon key for a normalized name, or null if below threshold */
export function findIconKey(normalizedName: string): string | null {
  // Exact match
  if (ICON_PACK[normalizedName]) return normalizedName;

  // starts-with match
  const startsMatch = ICON_KEYS.find(
    (k) => k.startsWith(normalizedName) || normalizedName.startsWith(k),
  );
  if (startsMatch) return startsMatch;

  // includes match
  const includesMatch = ICON_KEYS.find(
    (k) => k.includes(normalizedName) || normalizedName.includes(k),
  );
  if (includesMatch) return includesMatch;

  // Fuzzy similarity
  let bestKey: string | null = null;
  let bestScore = FUZZY_THRESHOLD;
  for (const key of ICON_KEYS) {
    const score = similarity(normalizedName, key);
    if (score > bestScore) {
      bestScore = score;
      bestKey = key;
    }
  }
  return bestKey;
}

interface AppIconProps {
  name: string;
  size?: number;
}

/** Letter avatar SVG rendered inline */
function LetterAvatar(props: { name: string; size: number }) {
  const letter = () => (props.name || "?")[0].toUpperCase();
  const color = () => avatarColor(props.name);
  const r = () => props.size / 2;
  const fontSize = () => Math.round(props.size * 0.45);

  return (
    <svg
      width={props.size}
      height={props.size}
      viewBox={`0 0 ${props.size} ${props.size}`}
      aria-label={props.name}
      role="img"
    >
      <circle cx={r()} cy={r()} r={r()} fill={color()} />
      <text
        x={r()}
        y={r()}
        text-anchor="middle"
        dominant-baseline="central"
        fill="white"
        font-size={fontSize()}
        font-family="system-ui, sans-serif"
        font-weight="600"
        style={{ "text-anchor": "middle", "dominant-baseline": "central" }}
      >
        {letter()}
      </text>
    </svg>
  );
}

/**
 * AppIcon resolves app icons in 3 tiers:
 * 1. Bundled SVG pack (instant, no network)
 * 2. Backend /api/icons/:name (desktop entry lookup)
 * 3. Letter avatar fallback (always works)
 */
export default function AppIcon(props: AppIconProps) {
  const size = () => props.size ?? 32;

  const bundledSvg = createMemo(() => {
    const normalized = normalizeAppName(props.name);
    const key = findIconKey(normalized);
    return key ? ICON_PACK[key] : null;
  });

  const [desktopFailed, setDesktopFailed] = createSignal(false);

  const apiUrl = () => `/api/icons/${encodeURIComponent(normalizeAppName(props.name))}`;

  return (
    <Show
      when={bundledSvg()}
      fallback={
        <Show when={!desktopFailed()} fallback={<LetterAvatar name={props.name} size={size()} />}>
          {/* Tier 2: desktop entry icon via backend */}
          <img
            src={apiUrl()}
            width={size()}
            height={size()}
            alt={props.name}
            loading="lazy"
            onError={() => setDesktopFailed(true)}
            style={{ "object-fit": "contain" }}
          />
        </Show>
      }
    >
      {/* Tier 1: bundled SVG */}
      <span
        style={{ display: "inline-flex", width: `${size()}px`, height: `${size()}px` }}
        aria-label={props.name}
        // eslint-disable-next-line solid/no-innerhtml
        innerHTML={sanitizeSvg(
          bundledSvg()!.replace(/<svg /, `<svg width="${size()}" height="${size()}" `),
        )}
      />
    </Show>
  );
}
