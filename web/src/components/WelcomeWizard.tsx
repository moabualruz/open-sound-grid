import { For, Show, createSignal, onMount, onCleanup } from "solid-js";
import type { JSX } from "solid-js";
import {
  Globe,
  Gamepad2,
  Music2,
  MessageCircle,
  LayoutGrid,
  Mic,
  Speaker,
  X,
  Sparkles,
} from "lucide-solid";
import { useSession } from "../stores/sessionStore";
import type { App } from "../types/session";

// ---------------------------------------------------------------------------
// App category detection
// ---------------------------------------------------------------------------

type AppCategory = "Browsers" | "Games" | "Music" | "Communication" | "Other";

const BROWSER_KEYWORDS = [
  "firefox",
  "chrome",
  "chromium",
  "brave",
  "opera",
  "vivaldi",
  "edge",
  "safari",
  "browser",
];
const GAME_KEYWORDS = [
  "steam",
  "game",
  "lutris",
  "heroic",
  "wine",
  "proton",
  "minecraft",
  "csgo",
  "dota",
  "overwatch",
  "valorant",
];
const MUSIC_KEYWORDS = [
  "spotify",
  "vlc",
  "mpv",
  "rhythmbox",
  "clementine",
  "amarok",
  "cantata",
  "deadbeef",
  "audacious",
  "cmus",
  "mopidy",
  "music",
  "soundcloud",
];
const COMM_KEYWORDS = [
  "discord",
  "teamspeak",
  "mumble",
  "telegram",
  "signal",
  "zoom",
  "teams",
  "slack",
  "skype",
  "element",
  "matrix",
  "mumble",
];

function detectCategory(app: App): AppCategory {
  const needle = (app.binary + " " + app.name).toLowerCase();
  if (BROWSER_KEYWORDS.some((k) => needle.includes(k))) return "Browsers";
  if (GAME_KEYWORDS.some((k) => needle.includes(k))) return "Games";
  if (MUSIC_KEYWORDS.some((k) => needle.includes(k))) return "Music";
  if (COMM_KEYWORDS.some((k) => needle.includes(k))) return "Communication";
  return "Other";
}

const CATEGORY_ORDER: AppCategory[] = ["Browsers", "Games", "Music", "Communication", "Other"];

function categoryIcon(cat: AppCategory): JSX.Element {
  switch (cat) {
    case "Browsers":
      return <Globe size={14} />;
    case "Games":
      return <Gamepad2 size={14} />;
    case "Music":
      return <Music2 size={14} />;
    case "Communication":
      return <MessageCircle size={14} />;
    default:
      return <LayoutGrid size={14} />;
  }
}

// ---------------------------------------------------------------------------
// WelcomeWizard
// ---------------------------------------------------------------------------

interface WelcomeWizardProps {
  onDone: () => void;
}

export default function WelcomeWizard(props: WelcomeWizardProps): JSX.Element {
  const { state, send } = useSession();

  // --- App selection state ---
  const apps = () => Object.values(state.session.apps);

  const [checkedApps, setCheckedApps] = createSignal<Set<string>>(
    new Set(Object.keys(state.session.apps)),
  );

  // --- Output device selection ---
  const devices = () => Object.entries(state.session.devices) as [string, unknown][];
  const [selectedOutput, setSelectedOutput] = createSignal<string>(devices()[0]?.[0] ?? "");

  // Group apps by category
  const grouped = (): Array<{ category: AppCategory; apps: App[] }> => {
    const byCategory = new Map<AppCategory, App[]>();
    for (const app of apps()) {
      const cat = detectCategory(app);
      if (!byCategory.has(cat)) byCategory.set(cat, []);
      byCategory.get(cat)!.push(app);
    }
    return CATEGORY_ORDER.filter((c) => byCategory.has(c)).map((c) => ({
      category: c,
      apps: byCategory.get(c)!,
    }));
  };

  function toggleApp(id: string) {
    setCheckedApps((prev) => {
      const next = new Set(prev);
      if (next.has(id)) {
        next.delete(id);
      } else {
        next.add(id);
      }
      return next;
    });
  }

  // --- Focus trap ---
  let dialogRef: HTMLDivElement | undefined;

  function trapFocus(e: KeyboardEvent) {
    if (!dialogRef) return;
    if (e.key === "Escape") {
      handleSkip();
      return;
    }
    if (e.key !== "Tab") return;
    const focusable = dialogRef.querySelectorAll<HTMLElement>(
      'button, [href], input, select, textarea, [tabindex]:not([tabindex="-1"])',
    );
    if (focusable.length === 0) return;
    const first = focusable[0];
    const last = focusable[focusable.length - 1];
    if (e.shiftKey) {
      if (document.activeElement === first) {
        e.preventDefault();
        last.focus();
      }
    } else {
      if (document.activeElement === last) {
        e.preventDefault();
        first.focus();
      }
    }
  }

  onMount(() => {
    document.addEventListener("keydown", trapFocus);
    // Focus the dialog on mount
    dialogRef?.focus();
  });

  onCleanup(() => {
    document.removeEventListener("keydown", trapFocus);
  });

  // --- Actions ---
  function handleSkip() {
    send({ type: "dismissWelcome" });
    props.onDone();
  }

  function handleCreateMixer() {
    const checked = checkedApps();

    // 1. Create a channel for each checked app
    for (const app of apps()) {
      if (!checked.has(app.id)) continue;
      send({ type: "createChannel", name: app.name, kind: "duplex" });
    }

    // 2. Create hardware mic channels (source type)
    // Hardware inputs are detected in the session as devices — they already
    // appear in the session; the wizard just sends createChannel for app streams.

    // 3. Dismiss the wizard and persist the flag
    send({ type: "dismissWelcome" });
    props.onDone();
  }

  const checkedCount = () => checkedApps().size;
  const hasApps = () => apps().length > 0;
  const hasDevices = () => devices().length > 0;

  return (
    <div
      class="fixed inset-0 z-50 flex items-center justify-center"
      style={{ "backdrop-filter": "blur(4px)", background: "rgba(0,0,0,0.6)" }}
      aria-modal="true"
      role="dialog"
      aria-labelledby="welcome-wizard-title"
    >
      <div
        ref={dialogRef}
        tabIndex={-1}
        class="relative z-50 mx-4 w-full max-w-lg rounded-xl border shadow-2xl outline-none"
        style={{
          "background-color": "var(--color-bg-elevated)",
          "border-color": "var(--color-border)",
        }}
      >
        {/* Header */}
        <div
          class="flex items-center justify-between border-b px-6 py-4"
          style={{ "border-color": "var(--color-border)" }}
        >
          <div class="flex items-center gap-2.5">
            <Sparkles size={18} style={{ color: "var(--color-accent)" }} />
            <h2
              id="welcome-wizard-title"
              class="text-base font-semibold"
              style={{ color: "var(--color-text-primary)" }}
            >
              Welcome to OpenSoundGrid
            </h2>
          </div>
          <button
            onClick={handleSkip}
            aria-label="Skip setup"
            class="rounded p-1 transition-colors"
            style={{ color: "var(--color-text-muted)" }}
          >
            <X size={16} />
          </button>
        </div>

        {/* Body */}
        <div class="max-h-[60vh] overflow-y-auto px-6 py-4 space-y-5">
          {/* Detected Audio Sources */}
          <section aria-label="Detected audio sources">
            <div class="mb-3 flex items-center gap-2">
              <LayoutGrid size={14} style={{ color: "var(--color-text-muted)" }} />
              <h3
                class="text-[11px] font-semibold uppercase tracking-widest"
                style={{ color: "var(--color-text-muted)" }}
              >
                Detected Audio Sources
              </h3>
            </div>

            <Show
              when={hasApps()}
              fallback={
                <p class="text-xs py-2" style={{ color: "var(--color-text-muted)" }}>
                  No running audio apps detected. You can add channels manually after setup.
                </p>
              }
            >
              <div class="space-y-3">
                <For each={grouped()}>
                  {({ category, apps: catApps }) => (
                    <div>
                      <div
                        class="mb-1.5 flex items-center gap-1.5 text-[10px] font-semibold uppercase tracking-wider"
                        style={{ color: "var(--color-text-muted)" }}
                      >
                        {categoryIcon(category)}
                        <span>{category}</span>
                      </div>
                      <div class="space-y-1">
                        <For each={catApps}>
                          {(app) => (
                            <label
                              class="flex cursor-pointer items-center gap-3 rounded-md px-3 py-2 transition-colors"
                              style={{
                                "background-color": checkedApps().has(app.id)
                                  ? "var(--color-bg-hover)"
                                  : "transparent",
                              }}
                            >
                              <input
                                type="checkbox"
                                checked={checkedApps().has(app.id)}
                                onChange={() => toggleApp(app.id)}
                                class="h-3.5 w-3.5 rounded accent-[var(--color-accent)]"
                                aria-label={`Include ${app.name}`}
                              />
                              <span
                                class="flex-1 truncate text-xs"
                                style={{ color: "var(--color-text-secondary)" }}
                              >
                                {app.name}
                              </span>
                              <span
                                class="shrink-0 text-[10px]"
                                style={{ color: "var(--color-text-muted)" }}
                              >
                                {app.binary}
                              </span>
                            </label>
                          )}
                        </For>
                      </div>
                    </div>
                  )}
                </For>
              </div>
            </Show>
          </section>

          {/* Output Device */}
          <Show when={hasDevices()}>
            <section aria-label="Output device">
              <div class="mb-3 flex items-center gap-2">
                <Speaker size={14} style={{ color: "var(--color-text-muted)" }} />
                <h3
                  class="text-[11px] font-semibold uppercase tracking-widest"
                  style={{ color: "var(--color-text-muted)" }}
                >
                  Output Device
                </h3>
              </div>
              <select
                value={selectedOutput()}
                onChange={(e) => setSelectedOutput(e.currentTarget.value)}
                class="w-full rounded-md border px-3 py-2 text-xs"
                style={{
                  "background-color": "var(--color-bg-primary)",
                  "border-color": "var(--color-border)",
                  color: "var(--color-text-primary)",
                }}
                aria-label="Primary output device"
              >
                <For each={devices()}>
                {([id]) => {
                  const label = id.replace(/[_-]+/g, " ").replace(/\s+/g, " ").trim();
                  return <option value={id}>{label || id}</option>;
                }}
              </For>
              </select>
            </section>
          </Show>

          {/* Hardware Inputs section — informational */}
          <section aria-label="Hardware inputs">
            <div class="mb-2 flex items-center gap-2">
              <Mic size={14} style={{ color: "var(--color-text-muted)" }} />
              <h3
                class="text-[11px] font-semibold uppercase tracking-widest"
                style={{ color: "var(--color-text-muted)" }}
              >
                Hardware Inputs
              </h3>
            </div>
            <p class="text-xs" style={{ color: "var(--color-text-muted)" }}>
              Microphones and hardware inputs will be available in the channel creator after setup.
            </p>
          </section>
        </div>

        {/* Footer */}
        <div
          class="flex items-center justify-between border-t px-6 py-4"
          style={{ "border-color": "var(--color-border)" }}
        >
          <button
            onClick={handleSkip}
            class="rounded-md px-4 py-2 text-xs transition-colors"
            style={{ color: "var(--color-text-muted)" }}
          >
            Skip
          </button>
          <button
            onClick={handleCreateMixer}
            class="flex items-center gap-2 rounded-md px-5 py-2 text-xs font-semibold text-[var(--color-text-on-accent,#fff)] transition-colors"
            style={{ "background-color": "var(--color-accent)" }}
            aria-label={
              checkedCount() > 0
                ? `Create mixer with ${checkedCount()} channel${checkedCount() === 1 ? "" : "s"}`
                : "Create mixer"
            }
          >
            <Sparkles size={14} />
            {checkedCount() > 0
              ? `Create Mixer (${checkedCount()} channel${checkedCount() === 1 ? "" : "s"})`
              : "Create Mixer"}
          </button>
        </div>
      </div>
    </div>
  );
}
