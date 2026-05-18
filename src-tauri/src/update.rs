//! GitHub Releases-backed update check.
//!
//! Network exception per CLAUDE.md §5: the GET to `api.github.com` only
//! fires on an explicit "Check for updates" click, or once per launch
//! when the user has opted into the auto-check toggle (default off).
//! No payload beyond the request itself is sent.

use serde::{Deserialize, Serialize};
use std::sync::OnceLock;

/// Owner / repo qrab releases live under. Centralised so a fork can
/// point at its own repo with a single edit + recompile.
const REPO: &str = "LeninBoccardo/qrab";

/// HTTP timeout. GitHub usually responds in well under a second; cap at
/// 8 s so a flaky network never wedges the UI behind a spinner.
const TIMEOUT_SECS: u64 = 8;

/// Process-wide reqwest client. reqwest's docs recommend reusing one
/// across calls so the connection pool, DNS cache, and TLS session cache
/// pay off across the (manual + auto) update checks the app makes per
/// launch. Built lazily on first use.
fn http_client() -> &'static reqwest::Client {
    // `.expect()` is normally banned in production paths (CLAUDE.md §15);
    // `reqwest::Client::build()` only fails on invalid TLS configuration,
    // and we don't supply any custom TLS bits — so this is unreachable
    // for the configuration we ship.
    static CLIENT: OnceLock<reqwest::Client> = OnceLock::new();
    CLIENT.get_or_init(|| {
        reqwest::Client::builder()
            .user_agent(format!(
                "qrab/{} update-check",
                env!("CARGO_PKG_VERSION")
            ))
            .timeout(std::time::Duration::from_secs(TIMEOUT_SECS))
            .build()
            .expect("reqwest::Client build with default rustls config")
    })
}

/// Result of an update check. `latest_version` and `release_url` are
/// `None` only when the GET failed; the caller should treat the error
/// as already-handled (logged) and surface a generic message to the user.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateStatus {
    pub current_version: String,
    pub latest_version: Option<String>,
    pub has_update: bool,
    pub release_url: Option<String>,
}

#[derive(Debug, Deserialize)]
struct GitHubRelease {
    tag_name: String,
    html_url: String,
}

/// Query the GitHub Releases API for the latest qrab release and compare
/// its tag to `CARGO_PKG_VERSION`. Returns a populated [`UpdateStatus`]
/// on success, or a `String` error suitable for display.
pub async fn check_for_updates() -> Result<UpdateStatus, String> {
    let current = env!("CARGO_PKG_VERSION").to_string();
    let url = format!("https://api.github.com/repos/{REPO}/releases/latest");

    let resp = http_client()
        .get(&url)
        .header("Accept", "application/vnd.github+json")
        .send()
        .await
        .map_err(|e| format!("network: {e}"))?;

    let status = resp.status();
    if !status.is_success() {
        return Err(format!("github responded {status}"));
    }

    let release: GitHubRelease = resp.json().await.map_err(|e| format!("parse: {e}"))?;
    let latest = release.tag_name.trim_start_matches('v').to_string();
    let has_update = is_newer(&latest, &current);

    // Defense in depth: only forward `html_url` to the UI if it's actually
    // a github.com URL. TLS already authenticates the response, but a
    // malformed manifest or a misconfigured fork shouldn't be able to
    // route the user's browser to `javascript:`, `file:`, or some random
    // host via the "View release" button.
    let release_url = if release.html_url.starts_with("https://github.com/") {
        Some(release.html_url)
    } else {
        log::warn!(
            "update check: dropping non-github release URL: {}",
            release.html_url
        );
        None
    };

    Ok(UpdateStatus {
        current_version: current,
        latest_version: Some(latest),
        has_update,
        release_url,
    })
}

/// True iff `latest` is strictly greater than `current` under semver
/// triple ordering. Missing components default to 0 (a tag of "1" is
/// treated as 1.0.0).
fn is_newer(latest: &str, current: &str) -> bool {
    to_triple(latest) > to_triple(current)
}

fn to_triple(s: &str) -> (u32, u32, u32) {
    // GitHub tags often look like "v1.2.3" or "v1.2.3-rc.1". We compare
    // numeric prefixes only — pre-release suffixes are ignored, which
    // means rc.1 == final for our purposes. Acceptable for the
    // update-prompt UX; we'd revisit if we ever publish prereleases.
    let head = s.split('-').next().unwrap_or(s);
    let mut parts = head.split('.');
    let a = parts.next().and_then(|x| x.parse().ok()).unwrap_or(0);
    let b = parts.next().and_then(|x| x.parse().ok()).unwrap_or(0);
    let c = parts.next().and_then(|x| x.parse().ok()).unwrap_or(0);
    (a, b, c)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn newer_minor_is_newer() {
        assert!(is_newer("1.1.0", "1.0.0"));
    }

    #[test]
    fn newer_patch_is_newer() {
        assert!(is_newer("1.0.1", "1.0.0"));
    }

    #[test]
    fn newer_major_is_newer() {
        assert!(is_newer("2.0.0", "1.99.99"));
    }

    #[test]
    fn equal_is_not_newer() {
        assert!(!is_newer("1.0.0", "1.0.0"));
    }

    #[test]
    fn older_is_not_newer() {
        assert!(!is_newer("0.9.9", "1.0.0"));
    }

    #[test]
    fn to_triple_does_not_strip_leading_v() {
        // `to_triple` is a pure numeric parser; stripping the GitHub
        // tag's leading `v` is `check_for_updates`'s job. If `v1.2.3`
        // somehow reaches `to_triple`, the leading `v1` fails to parse
        // and the major component falls back to 0 — so a stripped vs.
        // un-stripped tag would compare incorrectly. This test pins
        // the contract.
        assert_eq!(to_triple("v1.2.3"), (0, 2, 3));
        assert_eq!(to_triple("1.2.3"), (1, 2, 3));
    }

    #[test]
    fn pre_release_suffix_is_ignored() {
        assert_eq!(to_triple("1.2.3-rc.1"), (1, 2, 3));
        assert!(!is_newer("1.2.3-rc.1", "1.2.3"));
    }

    #[test]
    fn partial_versions_default_to_zero() {
        assert_eq!(to_triple("2"), (2, 0, 0));
        assert_eq!(to_triple("2.5"), (2, 5, 0));
        assert!(is_newer("2", "1.99.99"));
    }
}
