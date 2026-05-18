//! User-facing settings (CLAUDE.md §9). Persisted to `settings.json` in the
//! app data dir via `tauri-plugin-store`. The in-memory [`SettingsStore`]
//! is the live source of truth; the JSON file is the durable copy that's
//! re-read on startup. Hotkey + autostart side effects live in this module
//! so `commands::set_settings` stays declarative.

use anyhow::Context;
use serde::{Deserialize, Serialize};
use std::sync::{Arc, Mutex};
use tauri::{AppHandle, Runtime};
use tauri_plugin_autostart::ManagerExt;
use tauri_plugin_store::StoreExt;

const STORE_FILE: &str = "settings.json";
const STORE_KEY: &str = "settings";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Theme {
    System,
    Light,
    Dark,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Settings {
    pub hotkey: String,
    pub autostart: bool,
    pub auto_copy_on_single_result: bool,
    pub theme: Theme,
    pub close_after_copy: bool,
    pub close_after_open: bool,
    /// Opt-in: when true, qrab pings `api.github.com` once per launch to
    /// check for a newer release. Default `false` per the CLAUDE.md §5
    /// privacy posture — the user has to flip this themselves.
    #[serde(default)]
    pub check_for_updates_on_launch: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            hotkey: crate::hotkey::DEFAULT_HOTKEY.to_string(),
            autostart: false,
            auto_copy_on_single_result: false,
            theme: Theme::System,
            close_after_copy: false,
            close_after_open: false,
            check_for_updates_on_launch: false,
        }
    }
}

/// Thread-safe handle to the current settings. Cloning the handle shares the
/// underlying mutex — every clone observes the same state.
#[derive(Clone, Default)]
pub struct SettingsStore {
    inner: Arc<Mutex<Settings>>,
}

impl SettingsStore {
    pub fn new(initial: Settings) -> Self {
        Self {
            inner: Arc::new(Mutex::new(initial)),
        }
    }

    pub fn get(&self) -> Settings {
        self.inner.lock().unwrap_or_else(|p| p.into_inner()).clone()
    }

    pub fn set(&self, new: Settings) {
        let mut guard = self.inner.lock().unwrap_or_else(|p| p.into_inner());
        *guard = new;
    }
}

/// Load settings from the on-disk store. Missing file, missing key, and
/// deserialization errors all fall back to [`Settings::default`] with a
/// warning logged — the user never sees a hard failure for a corrupted
/// store file, just their settings reset.
pub fn load_from_store<R: Runtime>(app: &AppHandle<R>) -> Settings {
    let store = match app.store(STORE_FILE) {
        Ok(s) => s,
        Err(e) => {
            log::warn!("settings store open failed, using defaults: {e}");
            return Settings::default();
        }
    };
    let Some(value) = store.get(STORE_KEY) else {
        return Settings::default();
    };
    match serde_json::from_value::<Settings>(value) {
        Ok(s) => s,
        Err(e) => {
            log::warn!("settings deserialize failed, using defaults: {e}");
            Settings::default()
        }
    }
}

/// Persist the current settings. Caller is expected to have already
/// updated [`SettingsStore`] — this is just the durability hop.
pub fn save_to_store<R: Runtime>(app: &AppHandle<R>, settings: &Settings) -> anyhow::Result<()> {
    let store = app.store(STORE_FILE).context("open settings store")?;
    let value = serde_json::to_value(settings).context("serialize settings")?;
    store.set(STORE_KEY, value);
    store.save().context("flush settings store")?;
    Ok(())
}

/// Bring the OS autostart entry in line with `enabled`. The autostart
/// plugin already no-ops when called with the current state.
pub fn sync_autostart<R: Runtime>(app: &AppHandle<R>, enabled: bool) -> anyhow::Result<()> {
    let manager = app.autolaunch();
    if enabled {
        manager.enable().context("enable autostart")?;
    } else {
        manager.disable().context("disable autostart")?;
    }
    Ok(())
}

/// Reconcile the cached autostart preference with the actual OS state at
/// startup. The user may have flipped the OS-level startup entry between
/// runs (Task Manager / System Settings / .desktop edit); we trust the OS
/// as the source of truth and update our stored setting to match, so the
/// UI shows reality and `set_settings` doesn't silently re-enable an
/// entry the user deliberately removed.
///
/// Returns `true` if `settings.autostart` was mutated.
pub fn reconcile_autostart<R: Runtime>(app: &AppHandle<R>, settings: &mut Settings) -> bool {
    match app.autolaunch().is_enabled() {
        Ok(actual) if actual != settings.autostart => {
            log::info!(
                "autostart drift detected (stored: {}, OS: {}); using OS state",
                settings.autostart,
                actual,
            );
            settings.autostart = actual;
            true
        }
        Ok(_) => false,
        Err(e) => {
            log::warn!("autostart is_enabled check failed: {e}");
            false
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_matches_documented_baseline() {
        let s = Settings::default();
        assert_eq!(s.hotkey, crate::hotkey::DEFAULT_HOTKEY);
        assert!(!s.autostart);
        assert!(!s.auto_copy_on_single_result);
        assert_eq!(s.theme, Theme::System);
        assert!(!s.close_after_copy);
        assert!(!s.close_after_open);
        assert!(!s.check_for_updates_on_launch);
    }

    #[test]
    fn store_set_then_get_roundtrips() {
        let store = SettingsStore::new(Settings::default());
        let mut next = store.get();
        next.theme = Theme::Dark;
        next.autostart = true;
        store.set(next.clone());
        assert_eq!(store.get(), next);
    }

    #[test]
    fn clones_share_state() {
        let a = SettingsStore::new(Settings::default());
        let b = a.clone();
        let mut next = a.get();
        next.theme = Theme::Light;
        a.set(next.clone());
        assert_eq!(b.get(), next);
    }
}
