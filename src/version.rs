//! App release version helpers: semver parse/compare, Pages URLs, releases.json.

use serde::{Deserialize, Serialize};

/// Baked at compile time via `build.rs` (`KHANATIME_APP_VERSION`).
pub const APP_VERSION: &str = env!("KHANATIME_APP_VERSION");

/// Public Pages origin for stable releases (no trailing path beyond repo).
pub const PAGES_ORIGIN: &str = "https://boormat.github.io/khanatime";

/// Absolute URL for `releases.json` on Pages.
pub const RELEASES_JSON_URL: &str = "https://boormat.github.io/khanatime/releases.json";

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct SemVer {
    pub major: u32,
    pub minor: u32,
    pub patch: u32,
}

impl SemVer {
    /// Parse `X.Y.Z` or `vX.Y.Z`. Ignores pre-release / build metadata (`-rc.1`, `+sha`).
    pub fn parse(s: &str) -> Option<Self> {
        let s = s.trim().trim_start_matches('v');
        let core = s.split(['-', '+']).next().unwrap_or(s);
        let mut parts = core.split('.');
        let major = parts.next()?.parse().ok()?;
        let minor = parts.next()?.parse().ok()?;
        let patch = parts.next()?.parse().ok()?;
        if parts.next().is_some() {
            return None;
        }
        Some(SemVer {
            major,
            minor,
            patch,
        })
    }

    /// True when major.minor match (patch may differ) — mid-event hotfix line.
    #[allow(dead_code)] // used by product rules / future pin UX
    pub fn same_minor_line(self, other: Self) -> bool {
        self.major == other.major && self.minor == other.minor
    }
}

impl std::fmt::Display for SemVer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}.{}.{}", self.major, self.minor, self.patch)
    }
}

/// True when `version` looks like a stable `X.Y.Z` (not `dev-…`).
pub fn is_stable_semver(version: &str) -> bool {
    SemVer::parse(version).is_some() && !version.contains("dev-")
}

/// Pages base URL for an event pin / invite QR.
/// Stable → `…/vX.Y.Z/`; otherwise `/latest/` (root is the version catalog).
pub fn pinned_app_base(version: &str) -> String {
    if let Some(v) = SemVer::parse(version) {
        if is_stable_semver(version) {
            return format!("{PAGES_ORIGIN}/v{v}/");
        }
    }
    format!("{PAGES_ORIGIN}/latest/")
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ReleaseEntry {
    pub version: String,
    pub url: String,
    #[serde(default)]
    pub released_at: String,
    #[serde(default)]
    pub channel: String,
    #[serde(default)]
    pub notes: String,
    #[serde(default)]
    pub notes_url: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ReleasesManifest {
    pub latest: String,
    #[serde(default)]
    pub releases: Vec<ReleaseEntry>,
}

impl ReleasesManifest {
    pub fn parse_json(s: &str) -> Result<Self, serde_json::Error> {
        serde_json::from_str(s)
    }

    pub fn latest_semver(&self) -> Option<SemVer> {
        SemVer::parse(&self.latest)
    }

    /// Entries newer than `running` (newest first), for changelog snippets.
    pub fn newer_than(&self, running: &str) -> Vec<&ReleaseEntry> {
        let Some(cur) = SemVer::parse(running) else {
            return self.releases.iter().collect();
        };
        let mut out: Vec<&ReleaseEntry> = self
            .releases
            .iter()
            .filter(|e| SemVer::parse(&e.version).is_some_and(|v| v > cur))
            .collect();
        out.sort_by(|a, b| SemVer::parse(&b.version).cmp(&SemVer::parse(&a.version)));
        out
    }
}

/// Compare running build to manifest latest.
/// Returns `(latest_version, notes_joined)` when an update exists.
pub fn update_available(running: &str, manifest: &ReleasesManifest) -> Option<(String, String)> {
    let cur = SemVer::parse(running)?;
    let latest = manifest.latest_semver()?;
    if latest <= cur {
        return None;
    }
    let notes: Vec<String> = manifest
        .newer_than(running)
        .into_iter()
        .map(|e| {
            if e.notes.is_empty() {
                format!("• {}", e.version)
            } else {
                format!("• {}: {}", e.version, e.notes)
            }
        })
        .collect();
    Some((manifest.latest.clone(), notes.join("\n")))
}

/// Fetch `releases.json` from Pages. Soft-fails (None) when offline / CORS / 404.
#[cfg(target_arch = "wasm32")]
pub async fn fetch_releases_manifest() -> Option<ReleasesManifest> {
    fetch_releases_manifest_from(RELEASES_JSON_URL).await
}

#[cfg(target_arch = "wasm32")]
pub async fn fetch_releases_manifest_from(url: &str) -> Option<ReleasesManifest> {
    let window = web_sys::window()?;
    let resp_value = wasm_bindgen_futures::JsFuture::from(window.fetch_with_str(url))
        .await
        .ok()?;
    let resp: web_sys::Response = resp_value.dyn_into().ok()?;
    if !resp.ok() {
        return None;
    }
    let text = wasm_bindgen_futures::JsFuture::from(resp.text().ok()?)
        .await
        .ok()?;
    let text: String = text.as_string()?;
    ReleasesManifest::parse_json(&text).ok()
}

#[cfg(target_arch = "wasm32")]
use wasm_bindgen::JsCast;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_semver() {
        assert_eq!(
            SemVer::parse("0.2.1").unwrap(),
            SemVer {
                major: 0,
                minor: 2,
                patch: 1
            }
        );
        assert_eq!(SemVer::parse("v1.0.0").unwrap().major, 1);
        assert_eq!(SemVer::parse("0.3.0-rc.1").unwrap().patch, 0);
        assert!(SemVer::parse("dev-abc1234").is_none());
    }

    #[test]
    fn pinned_base() {
        assert_eq!(
            pinned_app_base("0.2.1"),
            "https://boormat.github.io/khanatime/v0.2.1/"
        );
        assert_eq!(
            pinned_app_base("dev-deadbee"),
            "https://boormat.github.io/khanatime/latest/"
        );
    }

    #[test]
    fn update_detects_newer_patch() {
        let m = ReleasesManifest {
            latest: "0.2.1".into(),
            releases: vec![
                ReleaseEntry {
                    version: "0.2.1".into(),
                    url: pinned_app_base("0.2.1"),
                    released_at: String::new(),
                    channel: "stable".into(),
                    notes: "Fix Start/Stop hide".into(),
                    notes_url: String::new(),
                },
                ReleaseEntry {
                    version: "0.2.0".into(),
                    url: pinned_app_base("0.2.0"),
                    released_at: String::new(),
                    channel: "stable".into(),
                    notes: "First pin".into(),
                    notes_url: String::new(),
                },
            ],
        };
        let (latest, notes) = update_available("0.2.0", &m).unwrap();
        assert_eq!(latest, "0.2.1");
        assert!(notes.contains("Fix Start/Stop"));
        assert!(update_available("0.2.1", &m).is_none());
    }

    #[test]
    fn major_minor_never_same_as_patch_line() {
        let a = SemVer::parse("0.2.0").unwrap();
        let b = SemVer::parse("0.2.5").unwrap();
        let c = SemVer::parse("0.3.0").unwrap();
        assert!(a.same_minor_line(b));
        assert!(!a.same_minor_line(c));
    }
}
