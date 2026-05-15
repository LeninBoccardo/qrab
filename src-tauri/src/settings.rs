//! User-facing settings (CLAUDE.md §9).
//!
//! C19 holds these in memory only; C20 swaps the backing to
//! `tauri-plugin-store` and adds live hotkey re-registration.

use serde::{Deserialize, Serialize};
use std::sync::{Arc, Mutex};

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
        Self { inner: Arc::new(Mutex::new(initial)) }
    }

    pub fn get(&self) -> Settings {
        self.inner.lock().unwrap_or_else(|p| p.into_inner()).clone()
    }

    pub fn set(&self, new: Settings) {
        let mut guard = self.inner.lock().unwrap_or_else(|p| p.into_inner());
        *guard = new;
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
