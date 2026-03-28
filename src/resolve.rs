use std::collections::HashMap;
use std::path::PathBuf;

use tracing::warn;

/// Maps application binary names to their desktop entry display names and icon paths.
///
/// On construction, scans all XDG desktop entry directories and builds a lookup
/// table from the `Exec` binary name to `(Name, icon_name)`. At resolve time,
/// icon names are expanded to full filesystem paths via `freedesktop_icons::lookup`.
pub struct AppResolver {
    /// binary_name -> (display_name, icon_name)
    cache: HashMap<String, (String, Option<String>)>,
}

impl AppResolver {
    /// Scan all desktop entries from the standard XDG paths and build the cache.
    pub fn new() -> Self {
        let mut cache = HashMap::new();

        let lang_env = std::env::var("LANG").unwrap_or_default();
        let lang_base = lang_env.split('.').next().unwrap_or("");
        let lang_short = lang_base.split('_').next().unwrap_or("");
        let locale_vec: Vec<&str> = if lang_base.is_empty() {
            vec![]
        } else if lang_short == lang_base {
            vec![lang_base]
        } else {
            vec![lang_base, lang_short]
        };
        let locales: &[&str] = &locale_vec;

        tracing::debug!(locales = ?locales, "AppResolver using system locales");

        let entries = freedesktop_desktop_entry::Iter::new(
            freedesktop_desktop_entry::default_paths(),
        )
        .entries(Some(locales));

        for entry in entries {
            let name = match entry.name(locales) {
                Some(n) => n.to_string(),
                None => continue,
            };

            let icon_name = entry.icon().map(|i| i.to_string());

            let Some(exec) = entry.exec() else {
                continue;
            };

            if let Some(binary) = extract_binary(exec) {
                cache.entry(binary).or_insert((name, icon_name));
            }
        }

        tracing::info!(entries = cache.len(), "AppResolver cache built from desktop entries");
        Self { cache }
    }

    /// Resolve a binary name (and optional PulseAudio app name) to a
    /// human-readable display name and an icon file path.
    ///
    /// Lookup order:
    /// 1. Exact binary name match in the desktop entry cache.
    /// 2. `pa_app_name` match (PulseAudio sometimes reports a different name).
    /// 3. Capitalize the binary name as a fallback display name.
    pub fn resolve(
        &self,
        binary: &str,
        pa_app_name: Option<&str>,
    ) -> (String, Option<PathBuf>) {
        if let Some((display_name, icon_name)) = self.cache.get(binary) {
            let icon_path = icon_name.as_deref().and_then(resolve_icon_path);
            tracing::debug!(binary, display_name = %display_name, has_icon = icon_path.is_some(), "resolve hit (binary match)");
            return (display_name.clone(), icon_path);
        }

        if let Some(pa_name) = pa_app_name {
            let pa_lower = pa_name.to_lowercase();
            if let Some((display_name, icon_name)) = self.cache.get(&pa_lower) {
                let icon_path = icon_name.as_deref().and_then(resolve_icon_path);
                tracing::debug!(binary, pa_name, display_name = %display_name, "resolve hit (PA name match)");
                return (display_name.clone(), icon_path);
            }
        }

        let display_name = pa_app_name
            .map(String::from)
            .unwrap_or_else(|| capitalize(binary));

        tracing::debug!(binary, display_name = %display_name, "resolve miss, using fallback name");
        (display_name, None)
    }
}

/// Resolve a freedesktop icon name to an on-disk file path.
///
/// Tries a 48px lookup first (common for app icons), then falls back to
/// any size. The lookup crate checks hicolor and pixmaps as a last resort.
fn resolve_icon_path(icon_name: &str) -> Option<PathBuf> {
    // If the icon name is already an absolute path, use it directly.
    if icon_name.starts_with('/') {
        let path = PathBuf::from(icon_name);
        if path.exists() {
            tracing::debug!(icon_name, "icon resolved from absolute path");
            return Some(path);
        }
        warn!("[AppResolver] absolute icon path does not exist: {icon_name}");
        return None;
    }

    let result = freedesktop_icons::lookup(icon_name)
        .with_size(48)
        .with_cache()
        .find()
        .or_else(|| {
            freedesktop_icons::lookup(icon_name)
                .with_cache()
                .find()
        });

    tracing::debug!(icon_name, found = result.is_some(), "icon lookup result");
    result
}

/// Extract the bare binary name from a desktop entry `Exec` value.
///
/// Strips:
/// - field codes: `%u`, `%U`, `%f`, `%F`, `%d`, `%D`, `%n`, `%N`, `%i`, `%c`, `%k`, `%v`, `%m`
/// - leading path prefixes (`/usr/bin/foo` -> `foo`)
/// - env-var prefixes (`env VAR=val command` -> `command`)
///
/// Returns `None` if the exec line is empty after processing.
fn extract_binary(exec: &str) -> Option<String> {
    let mut parts = exec.split_whitespace();
    let mut first = parts.next()?;

    // Skip common wrapper commands like `env`, `flatpak`, etc.
    // `env VAR=val command ...` -> skip `env` and all `KEY=VAL` tokens.
    if first == "env" {
        for token in parts.by_ref() {
            if token.contains('=') {
                continue;
            }
            first = token;
            break;
        }
    }

    // Strip path prefix: `/usr/bin/firefox` -> `firefox`
    let binary = first
        .rsplit('/')
        .next()
        .unwrap_or(first);

    if binary.is_empty() || binary.starts_with('%') {
        return None;
    }

    Some(binary.to_string())
}

/// Capitalize the first character of a string for display purposes.
fn capitalize(s: &str) -> String {
    let mut chars = s.chars();
    match chars.next() {
        None => String::new(),
        Some(c) => c.to_uppercase().to_string() + chars.as_str(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extract_simple_binary() {
        assert_eq!(
            extract_binary("firefox %u"),
            Some("firefox".to_string())
        );
    }

    #[test]
    fn extract_binary_with_path() {
        assert_eq!(
            extract_binary("/usr/bin/firefox %U"),
            Some("firefox".to_string())
        );
    }

    #[test]
    fn extract_binary_with_env() {
        assert_eq!(
            extract_binary("env GDK_BACKEND=x11 discord"),
            Some("discord".to_string())
        );
    }

    #[test]
    fn extract_binary_no_args() {
        assert_eq!(
            extract_binary("vlc"),
            Some("vlc".to_string())
        );
    }

    #[test]
    fn extract_binary_empty() {
        assert_eq!(extract_binary(""), None);
    }

    #[test]
    fn extract_binary_only_field_code() {
        assert_eq!(extract_binary("%u"), None);
    }

    #[test]
    fn capitalize_works() {
        assert_eq!(capitalize("firefox"), "Firefox");
        assert_eq!(capitalize(""), "");
        assert_eq!(capitalize("a"), "A");
    }

    #[test]
    fn resolve_fallback_capitalizes_binary() {
        let resolver = AppResolver {
            cache: HashMap::new(),
        };
        let (name, icon) = resolver.resolve("unknown-app", None);
        assert_eq!(name, "Unknown-app");
        assert!(icon.is_none());
    }

    #[test]
    fn resolve_prefers_pa_name_when_binary_missing() {
        let resolver = AppResolver {
            cache: HashMap::new(),
        };
        let (name, icon) = resolver.resolve("missing", Some("My Player"));
        assert_eq!(name, "My Player");
        assert!(icon.is_none());
    }

    #[test]
    fn resolve_finds_cached_entry() {
        let mut cache = HashMap::new();
        cache.insert(
            "firefox".to_string(),
            ("Mozilla Firefox".to_string(), Some("firefox".to_string())),
        );
        let resolver = AppResolver { cache };

        let (name, _icon) = resolver.resolve("firefox", None);
        assert_eq!(name, "Mozilla Firefox");
    }

    /// `flatpak run org.mozilla.firefox` — `flatpak` is the actual binary being
    /// executed. The current implementation does not skip `flatpak` the way it
    /// skips `env`, so it returns `"flatpak"` as the binary name.
    #[test]
    fn test_extract_binary_flatpak() {
        assert_eq!(
            extract_binary("flatpak run org.mozilla.firefox"),
            Some("flatpak".to_string()),
            "flatpak is the real binary; the app ID is an argument"
        );
    }

    /// Multiple `KEY=VAL` tokens after `env` should all be skipped; the first
    /// non-assignment token is the binary.
    #[test]
    fn test_extract_binary_with_multiple_env_vars() {
        assert_eq!(
            extract_binary("env GDK_BACKEND=x11 QT_QPA_PLATFORM=xcb discord"),
            Some("discord".to_string()),
            "binary should be extracted after skipping env and all KEY=VAL tokens"
        );
    }
}
