//! Tier-2 icon resolution: parse desktop entries and resolve icon paths.
//!
//! Serves `GET /api/icons/:app_name` — resolves the icon for an app name
//! by scanning `.desktop` files in standard XDG locations, then locating the
//! actual icon file in XDG icon theme directories.

use std::{
    collections::HashMap,
    fs,
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
};

use axum::{
    body::Body,
    http::{HeaderValue, Response, StatusCode, header},
    response::IntoResponse,
};

// ---------------------------------------------------------------------------
// Desktop entry parsing
// ---------------------------------------------------------------------------

/// Parsed fields from a `.desktop` file that we care about.
#[derive(Debug, Clone)]
pub struct DesktopEntry {
    /// Value of the `Name=` field.
    pub name: String,
    /// Value of the `Icon=` field (may be a bare name or an absolute path).
    pub icon: String,
}

/// Parse a `.desktop` file and extract the first `[Desktop Entry]` section's
/// `Name=` and `Icon=` values.  Returns `None` if either field is absent.
pub fn parse_desktop_entry(content: &str) -> Option<DesktopEntry> {
    let mut in_section = false;
    let mut name: Option<String> = None;
    let mut icon: Option<String> = None;

    for line in content.lines() {
        let line = line.trim();
        if line.starts_with('[') {
            in_section = line == "[Desktop Entry]";
            continue;
        }
        if !in_section {
            continue;
        }
        if let Some(val) = line.strip_prefix("Name=")
            && name.is_none()
        {
            name = Some(val.to_owned());
        } else if let Some(val) = line.strip_prefix("Icon=")
            && icon.is_none()
        {
            icon = Some(val.to_owned());
        }
        if name.is_some() && icon.is_some() {
            break;
        }
    }

    Some(DesktopEntry {
        name: name?,
        icon: icon?,
    })
}

// ---------------------------------------------------------------------------
// Icon file resolution
// ---------------------------------------------------------------------------

/// Preferred sizes in descending order (we want the largest available).
const PREFERRED_SIZES: &[&str] = &["scalable", "128x128", "64x64", "48x48", "32x32"];

/// Icon theme search directories.
fn icon_search_dirs() -> Vec<PathBuf> {
    let mut dirs: Vec<PathBuf> = Vec::new();
    if let Some(home) = dirs::home_dir() {
        dirs.push(home.join(".local/share/icons"));
    }
    dirs.push(PathBuf::from("/usr/share/icons/hicolor"));
    dirs.push(PathBuf::from("/usr/share/icons"));
    dirs.push(PathBuf::from("/usr/share/pixmaps"));
    dirs
}

/// Try to resolve a bare icon name to a file path.
///
/// Search order: SVG preferred, then PNG by size (128 → 64 → 48 → 32).
pub fn resolve_icon_path(icon_name: &str) -> Option<PathBuf> {
    // If it's already an absolute path and exists, use it directly.
    let p = PathBuf::from(icon_name);
    if p.is_absolute() && p.exists() {
        return Some(p);
    }

    let base = p.file_stem().and_then(|s| s.to_str()).unwrap_or(icon_name);

    for dir in icon_search_dirs() {
        // Check pixmaps-style flat directory (SVG then PNG)
        for ext in &["svg", "png"] {
            let candidate = dir.join(format!("{base}.{ext}"));
            if candidate.exists() {
                return Some(candidate);
            }
        }

        // Check sized subdirs: hicolor/<size>/apps/<name>.<ext>
        for size in PREFERRED_SIZES {
            for ext in &["svg", "png"] {
                let candidate = dir.join(size).join("apps").join(format!("{base}.{ext}"));
                if candidate.exists() {
                    return Some(candidate);
                }
            }
        }
    }

    None
}

// ---------------------------------------------------------------------------
// Application paths for .desktop files
// ---------------------------------------------------------------------------

fn desktop_dirs() -> Vec<PathBuf> {
    let mut dirs: Vec<PathBuf> = Vec::new();
    if let Some(home) = dirs::home_dir() {
        dirs.push(home.join(".local/share/applications"));
    }
    dirs.push(PathBuf::from("/usr/share/applications"));
    dirs
}

// ---------------------------------------------------------------------------
// Icon cache
// ---------------------------------------------------------------------------

/// Thread-safe cache: normalized app name → resolved icon path (or None if
/// not found).
#[derive(Debug, Clone, Default)]
pub struct IconCache {
    inner: Arc<Mutex<HashMap<String, Option<PathBuf>>>>,
}

impl IconCache {
    pub fn new() -> Self {
        Self::default()
    }

    fn get(&self, key: &str) -> Option<Option<PathBuf>> {
        self.inner.lock().ok().and_then(|g| g.get(key).cloned())
    }

    fn set(&self, key: String, value: Option<PathBuf>) {
        if let Ok(mut g) = self.inner.lock() {
            g.insert(key, value);
        }
    }

    /// Look up the resolved icon path for `app_name`.  Populates the cache on
    /// first access by scanning desktop files.
    pub fn resolve(&self, app_name: &str) -> Option<PathBuf> {
        let key = normalize_name(app_name);

        if let Some(cached) = self.get(&key) {
            return cached;
        }

        let result = find_icon_for_app(&key);
        self.set(key, result.clone());
        result
    }
}

// ---------------------------------------------------------------------------
// Name normalization (mirrors the frontend logic)
// ---------------------------------------------------------------------------

/// Normalize an app name: lowercase, strip rev-DNS prefixes, strip version
/// suffixes, replace separators with hyphens.
pub fn normalize_name(name: &str) -> String {
    let mut s = name.to_lowercase();
    // Strip reverse-DNS prefix
    for prefix in &["org.", "com.", "net.", "io.", "app."] {
        if let Some(rest) = s.strip_prefix(prefix)
            && let Some(dot) = rest.find('.')
        {
            s = rest[dot + 1..].to_owned();
            break;
        }
    }
    // Strip version suffix
    if let Some(idx) = s.rfind(['-', '_']).filter(|&i| {
        s[i + 1..]
            .chars()
            .next()
            .map(|c| c.is_ascii_digit() || c == 'v')
            .unwrap_or(false)
    }) {
        s.truncate(idx);
    }
    s.replace(['_', ' '], "-")
}

// ---------------------------------------------------------------------------
// Desktop file scan
// ---------------------------------------------------------------------------

/// Walk desktop directories to find an icon name for `app_name`, then resolve
/// it to a file path.
fn find_icon_for_app(normalized: &str) -> Option<PathBuf> {
    for dir in desktop_dirs() {
        let Ok(entries) = fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) != Some("desktop") {
                continue;
            }
            let Ok(content) = fs::read_to_string(&path) else {
                continue;
            };
            let Some(de) = parse_desktop_entry(&content) else {
                continue;
            };
            let file_stem = path
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or_default();

            // Match by filename stem or by Name= field (normalized)
            let name_normalized = normalize_name(&de.name);
            if normalize_name(file_stem) == normalized || name_normalized == normalized {
                return resolve_icon_path(&de.icon);
            }
        }
    }
    None
}

// ---------------------------------------------------------------------------
// Icon serving
// ---------------------------------------------------------------------------

/// Returns `true` if the app name contains characters that could be used
/// for path traversal or injection.
pub fn is_valid_app_name(name: &str) -> bool {
    !name.is_empty()
        && !name.contains("..")
        && !name.contains('/')
        && !name.contains('\\')
        && !name.contains('\0')
}

/// Allowed parent directories for resolved icon paths.
fn allowed_icon_roots() -> Vec<PathBuf> {
    let mut roots = Vec::new();
    if let Some(home) = dirs::home_dir() {
        roots.push(home.join(".local/share/icons"));
        roots.push(home.join(".local/share/applications"));
    }
    roots.push(PathBuf::from("/usr/share/icons"));
    roots.push(PathBuf::from("/usr/share/pixmaps"));
    roots.push(PathBuf::from("/usr/share/applications"));
    roots
}

/// Verify that a resolved icon path is within an allowed directory.
fn path_is_allowed(path: &Path) -> bool {
    let Ok(canonical) = path.canonicalize() else {
        return false;
    };
    allowed_icon_roots()
        .iter()
        .any(|root| canonical.starts_with(root))
}

/// Resolve and serve an icon for `app_name`.  Called from the axum handler in
/// `main.rs` which owns the `AppState` containing the `IconCache`.
pub fn serve_icon(cache: &IconCache, app_name: &str) -> Response<Body> {
    if !is_valid_app_name(app_name) {
        return StatusCode::BAD_REQUEST.into_response();
    }
    match cache.resolve(app_name) {
        None => StatusCode::NOT_FOUND.into_response(),
        Some(path) => {
            // CRIT-1: Verify resolved path is within allowed icon directories.
            if !path_is_allowed(&path) {
                return StatusCode::FORBIDDEN.into_response();
            }
            serve_file(&path)
        }
    }
}

fn serve_file(path: &Path) -> Response<Body> {
    let content_type = if path.extension().and_then(|e| e.to_str()) == Some("svg") {
        "image/svg+xml"
    } else {
        "image/png"
    };

    match fs::read(path) {
        Err(_) => StatusCode::NOT_FOUND.into_response(),
        Ok(bytes) => Response::builder()
            .status(StatusCode::OK)
            .header(header::CONTENT_TYPE, HeaderValue::from_static(content_type))
            .header(
                header::X_CONTENT_TYPE_OPTIONS,
                HeaderValue::from_static("nosniff"),
            )
            .body(Body::from(bytes))
            .unwrap_or_else(|_| StatusCode::INTERNAL_SERVER_ERROR.into_response()),
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    fn make_desktop(content: &str) -> NamedTempFile {
        let mut f = NamedTempFile::new().expect("tempfile");
        f.write_all(content.as_bytes()).expect("write");
        f
    }

    // --- Desktop entry parsing ---

    #[test]
    fn parse_valid_desktop_entry() {
        let content = "[Desktop Entry]\nName=Firefox\nIcon=firefox\nExec=firefox\n";
        let de = parse_desktop_entry(content).expect("should parse");
        assert_eq!(de.name, "Firefox");
        assert_eq!(de.icon, "firefox");
    }

    #[test]
    fn parse_desktop_entry_missing_icon_returns_none() {
        let content = "[Desktop Entry]\nName=Firefox\nExec=firefox\n";
        assert!(parse_desktop_entry(content).is_none());
    }

    #[test]
    fn parse_desktop_entry_missing_name_returns_none() {
        let content = "[Desktop Entry]\nIcon=firefox\nExec=firefox\n";
        assert!(parse_desktop_entry(content).is_none());
    }

    #[test]
    fn parse_desktop_ignores_non_desktop_entry_sections() {
        let content =
            "[Other Section]\nName=Other\nIcon=other\n\n[Desktop Entry]\nName=Real\nIcon=real\n";
        let de = parse_desktop_entry(content).expect("should parse");
        assert_eq!(de.name, "Real");
        assert_eq!(de.icon, "real");
    }

    // --- Icon resolution ---

    #[test]
    fn resolve_absolute_existing_path() {
        let f = make_desktop("dummy");
        let path = f.path().to_owned();
        let resolved = resolve_icon_path(path.to_str().unwrap());
        assert_eq!(resolved, Some(path));
    }

    #[test]
    fn resolve_nonexistent_name_returns_none() {
        let result = resolve_icon_path("this-app-definitely-does-not-exist-xyzzy");
        assert!(result.is_none());
    }

    // --- Name normalization ---

    #[test]
    fn normalize_strips_org_prefix() {
        assert_eq!(normalize_name("org.mozilla.firefox"), "firefox");
    }

    #[test]
    fn normalize_strips_com_prefix() {
        assert_eq!(normalize_name("com.valvesoftware.steam"), "steam");
    }

    #[test]
    fn normalize_strips_version_suffix() {
        assert_eq!(normalize_name("firefox-120"), "firefox");
    }

    #[test]
    fn normalize_lowercases() {
        assert_eq!(normalize_name("Firefox"), "firefox");
    }

    #[test]
    fn normalize_replaces_underscores() {
        assert_eq!(normalize_name("obs_studio"), "obs-studio");
    }

    // --- Cache ---

    #[test]
    fn cache_returns_none_for_unknown_app() {
        let cache = IconCache::new();
        let result = cache.resolve("this-app-definitely-does-not-exist-xyzzy");
        assert!(result.is_none());
    }
}
