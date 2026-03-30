import { createContext, useContext, type ParentProps } from "solid-js";
import { createStore } from "solid-js/store";

interface MixerSettings {
  stereoMode: "mono" | "stereo";
}

interface MixerSettingsApi {
  settings: MixerSettings;
  setStereoMode: (mode: "mono" | "stereo") => void;
}

const MixerSettingsContext = createContext<MixerSettingsApi>();

export function MixerSettingsProvider(props: ParentProps) {
  const [settings, setSettings] = createStore<MixerSettings>({
    stereoMode: "mono",
  });

  const api: MixerSettingsApi = {
    settings,
    setStereoMode: (mode) => setSettings("stereoMode", mode),
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
