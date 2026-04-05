/**
 * Bundled SVG icon pack for common Linux applications.
 * Keys are normalized app names (lowercase, no spaces, no version suffixes).
 */

import firefoxSvg from "./firefox.svg?raw";
import chromiumSvg from "./chromium.svg?raw";
import googleChromeSvg from "./google-chrome.svg?raw";
import spotifySvg from "./spotify.svg?raw";
import discordSvg from "./discord.svg?raw";
import obsStudioSvg from "./obs-studio.svg?raw";
import steamSvg from "./steam.svg?raw";
import vlcSvg from "./vlc.svg?raw";
import mpvSvg from "./mpv.svg?raw";
import rhythmboxSvg from "./rhythmbox.svg?raw";
import thunderbirdSvg from "./thunderbird.svg?raw";
import telegramSvg from "./telegram.svg?raw";
import slackSvg from "./slack.svg?raw";
import zoomSvg from "./zoom.svg?raw";
import teamsSvg from "./teams.svg?raw";
import gimpSvg from "./gimp.svg?raw";
import audacitySvg from "./audacity.svg?raw";
import kdenlivesSvg from "./kdenlive.svg?raw";
import blenderSvg from "./blender.svg?raw";
import libreofficeSvg from "./libreoffice.svg?raw";
import codeSvg from "./code.svg?raw";
import alacrittySvg from "./alacritty.svg?raw";
import kittySvg from "./kitty.svg?raw";
import weztermSvg from "./wezterm.svg?raw";
import footSvg from "./foot.svg?raw";
import nautilusSvg from "./nautilus.svg?raw";
import dolphinSvg from "./dolphin.svg?raw";
import pavucontrolSvg from "./pavucontrol.svg?raw";
import easyeffectsSvg from "./easyeffects.svg?raw";
import pipewireSvg from "./pipewire.svg?raw";

/** Normalized icon pack: key → raw SVG string */
export const ICON_PACK: Record<string, string> = {
  firefox: firefoxSvg,
  chromium: chromiumSvg,
  "google-chrome": googleChromeSvg,
  chrome: googleChromeSvg,
  spotify: spotifySvg,
  discord: discordSvg,
  "obs-studio": obsStudioSvg,
  obs: obsStudioSvg,
  steam: steamSvg,
  vlc: vlcSvg,
  mpv: mpvSvg,
  rhythmbox: rhythmboxSvg,
  thunderbird: thunderbirdSvg,
  telegram: telegramSvg,
  "telegram-desktop": telegramSvg,
  slack: slackSvg,
  zoom: zoomSvg,
  teams: teamsSvg,
  "microsoft-teams": teamsSvg,
  gimp: gimpSvg,
  audacity: audacitySvg,
  kdenlive: kdenlivesSvg,
  blender: blenderSvg,
  libreoffice: libreofficeSvg,
  "libreoffice-writer": libreofficeSvg,
  "libreoffice-calc": libreofficeSvg,
  "libreoffice-impress": libreofficeSvg,
  code: codeSvg,
  vscode: codeSvg,
  "visual-studio-code": codeSvg,
  alacritty: alacrittySvg,
  kitty: kittySvg,
  wezterm: weztermSvg,
  foot: footSvg,
  nautilus: nautilusSvg,
  "org.gnome.nautilus": nautilusSvg,
  dolphin: dolphinSvg,
  pavucontrol: pavucontrolSvg,
  easyeffects: easyeffectsSvg,
  pipewire: pipewireSvg,
};

/** All normalized keys in the pack for fuzzy matching */
export const ICON_KEYS = Object.keys(ICON_PACK);
