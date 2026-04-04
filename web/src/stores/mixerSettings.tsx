import { createContext, useContext, createEffect, onCleanup, type ParentProps } from "solid-js";
import { createStore } from "solid-js/store";

export type ThemePreference = "dark" | "light" | "system";

const THEME_STORAGE_KEY = "osg-theme";

function loadThemePreference(): ThemePreference {
  const stored = localStorage.getItem(THEME_STORAGE_KEY);
  if (stored === "dark" || stored === "light" || stored === "system") return stored;
  return "dark";
}

function resolveTheme(preference: ThemePreference): "dark" | "light" {
  if (preference === "system") {
    return window.matchMedia("(prefers-color-scheme: dark)").matches ? "dark" : "light";
  }
  return preference;
}

function applyThemeClass(resolved: "dark" | "light") {
  const el = document.documentElement;
  if (resolved === "dark") {
    el.classList.add("dark");
  } else {
    el.classList.remove("dark");
  }
}

interface MixerSettings {
  stereoMode: "mono" | "stereo";
  theme: ThemePreference;
}

interface MixerSettingsApi {
  settings: MixerSettings;
  setStereoMode: (mode: "mono" | "stereo") => void;
  setTheme: (theme: ThemePreference) => void;
}

const MixerSettingsContext = createContext<MixerSettingsApi>();

export function MixerSettingsProvider(props: ParentProps) {
  const [settings, setSettings] = createStore<MixerSettings>({
    stereoMode: "mono",
    theme: loadThemePreference(),
  });

  // Reactively apply theme class and persist preference
  createEffect(() => {
    const pref = settings.theme;
    localStorage.setItem(THEME_STORAGE_KEY, pref);
    applyThemeClass(resolveTheme(pref));
  });

  // Listen for OS theme changes when in "system" mode
  createEffect(() => {
    if (settings.theme !== "system") return;

    const mql = window.matchMedia("(prefers-color-scheme: dark)");
    const handler = (e: MediaQueryListEvent) => {
      applyThemeClass(e.matches ? "dark" : "light");
    };
    mql.addEventListener("change", handler);
    onCleanup(() => mql.removeEventListener("change", handler));
  });

  const api: MixerSettingsApi = {
    settings,
    setStereoMode: (mode) => setSettings("stereoMode", mode),
    setTheme: (theme) => setSettings("theme", theme),
  };

  return (
    <MixerSettingsContext.Provider value={api}>{props.children}</MixerSettingsContext.Provider>
  );
}

export function useMixerSettings(): MixerSettingsApi {
  const ctx = useContext(MixerSettingsContext);
  if (!ctx) throw new Error("useMixerSettings must be used within MixerSettingsProvider");
  return ctx;
}
