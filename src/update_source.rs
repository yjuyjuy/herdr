//! Update-source selection seam.
//!
//! Herdr resolves its self-update and remote-attach manifests from a single
//! source base. This module is the one place that decides where those
//! manifests come from, so the source can be repointed without scattering
//! host names through the updater and remote-attach code.
//!
//! Resolution order, highest priority first:
//! 1. `HERDR_UPDATE_MANIFEST_BASE` environment override. Operators can point a
//!    build at an alternate manifest host for testing or private mirrors.
//! 2. The compiled-in default base. This fork ships `yjuyjuy/herdr` as the
//!    default so the fork build cannot silently update itself back to an
//!    upstream release that lacks the fork's agent manifests.
//!
//! The base is a directory URL. Concrete manifest URLs are the base joined
//! with `latest.json` (stable channel) and `preview.json` (preview channel).

use std::env;

/// Environment variable that overrides the update-manifest base URL.
pub(crate) const UPDATE_MANIFEST_BASE_ENV: &str = "HERDR_UPDATE_MANIFEST_BASE";

/// Compiled-in default update-manifest base for this fork.
///
/// Points at the captain-owned `yjuyjuy/herdr` fork's committed manifests on
/// the `master` branch instead of the upstream `herdr.dev` host. The fork's
/// `website/latest.json` and `website/preview.json` describe the fork's own
/// release assets, so an update check can never resolve an upstream release
/// that drops the fork's Jcode (and other fork-only) agent manifests.
pub(crate) const DEFAULT_UPDATE_MANIFEST_BASE: &str =
    "https://raw.githubusercontent.com/yjuyjuy/herdr/master/website";

/// Stable-channel manifest file name, joined onto the resolved base.
const STABLE_MANIFEST_FILE: &str = "latest.json";
/// Preview-channel manifest file name, joined onto the resolved base.
const PREVIEW_MANIFEST_FILE: &str = "preview.json";

/// Resolve the update-manifest base URL, honoring the environment override.
///
/// A blank or whitespace-only override is ignored so an accidentally empty
/// variable cannot disable updates. A trailing slash on the base is removed so
/// joining never produces a double slash.
pub(crate) fn manifest_base() -> String {
    let base = env::var(UPDATE_MANIFEST_BASE_ENV)
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| DEFAULT_UPDATE_MANIFEST_BASE.to_string());
    base.trim_end_matches('/').to_string()
}

/// Join a manifest file name onto the resolved base.
fn manifest_url(file: &str) -> String {
    format!("{}/{}", manifest_base(), file)
}

/// Resolved stable-channel update-manifest URL.
pub(crate) fn stable_manifest_url() -> String {
    manifest_url(STABLE_MANIFEST_FILE)
}

/// Resolved preview-channel update-manifest URL.
pub(crate) fn preview_manifest_url() -> String {
    manifest_url(PREVIEW_MANIFEST_FILE)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Mutex, MutexGuard};

    // Serialize tests that mutate the shared process environment variable.
    static ENV_LOCK: Mutex<()> = Mutex::new(());

    struct EnvGuard<'a> {
        _lock: MutexGuard<'a, ()>,
        prior: Option<String>,
    }

    impl<'a> EnvGuard<'a> {
        fn set(value: Option<&str>) -> Self {
            let lock = ENV_LOCK
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            let prior = env::var(UPDATE_MANIFEST_BASE_ENV).ok();
            match value {
                Some(value) => env::set_var(UPDATE_MANIFEST_BASE_ENV, value),
                None => env::remove_var(UPDATE_MANIFEST_BASE_ENV),
            }
            Self { _lock: lock, prior }
        }
    }

    impl Drop for EnvGuard<'_> {
        fn drop(&mut self) {
            match &self.prior {
                Some(value) => env::set_var(UPDATE_MANIFEST_BASE_ENV, value),
                None => env::remove_var(UPDATE_MANIFEST_BASE_ENV),
            }
        }
    }

    #[test]
    fn default_source_targets_the_captain_fork() {
        let _guard = EnvGuard::set(None);
        assert_eq!(manifest_base(), DEFAULT_UPDATE_MANIFEST_BASE);
        assert_eq!(
            stable_manifest_url(),
            "https://raw.githubusercontent.com/yjuyjuy/herdr/master/website/latest.json"
        );
        assert_eq!(
            preview_manifest_url(),
            "https://raw.githubusercontent.com/yjuyjuy/herdr/master/website/preview.json"
        );
    }

    #[test]
    fn default_source_is_not_upstream_herdr_dev() {
        let _guard = EnvGuard::set(None);
        assert!(!manifest_base().contains("herdr.dev"));
        assert!(manifest_base().contains("yjuyjuy/herdr"));
    }

    #[test]
    fn environment_override_repoints_both_channels() {
        let _guard = EnvGuard::set(Some("https://example.test/mirror"));
        assert_eq!(manifest_base(), "https://example.test/mirror");
        assert_eq!(
            stable_manifest_url(),
            "https://example.test/mirror/latest.json"
        );
        assert_eq!(
            preview_manifest_url(),
            "https://example.test/mirror/preview.json"
        );
    }

    #[test]
    fn trailing_slash_on_override_is_normalized() {
        let _guard = EnvGuard::set(Some("https://example.test/mirror/"));
        assert_eq!(
            stable_manifest_url(),
            "https://example.test/mirror/latest.json"
        );
    }

    #[test]
    fn blank_override_falls_back_to_the_fork_default() {
        let _guard = EnvGuard::set(Some("   "));
        assert_eq!(manifest_base(), DEFAULT_UPDATE_MANIFEST_BASE);
    }
}
