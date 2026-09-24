//! App preferences. Nothing secret lives here: they must be readable while locked
//! (theme on the lock screen, the global shortcut, auto-lock policy).

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct Settings {
    /// "system" | "light" | "dark"
    pub theme: String,
    /// Minutes of system-wide inactivity before locking. 0 = never.
    pub auto_lock_minutes: u32,
    pub lock_on_sleep: bool,
    pub lock_on_session_lock: bool,
    pub lock_on_minimize: bool,
    /// Seconds before copied secrets are cleared from the clipboard. 0 = never.
    pub clipboard_clear_seconds: u32,
    /// Keep running in the tray when the window is closed.
    pub close_to_tray: bool,
    pub launch_at_startup: bool,
    /// Global shortcut that opens Quick Access, e.g. "CommandOrControl+Shift+Space".
    pub quick_access_shortcut: String,
    /// Hide the window from screenshots and screen sharing.
    pub screen_capture_protection: bool,
    /// Check passwords against known breaches (k-anonymity, opt-in).
    pub breach_check: bool,
    /// Download site icons directly from each site (opt-in).
    pub site_icons: bool,
    /// Ask for the master password every N days even with Windows Hello. 0 = never.
    pub require_password_days: u32,
    pub update_channel: String,
    pub auto_update: bool,
    /// Density of lists: "comfortable" | "compact".
    pub density: String,
    /// Onboarding checklist entries the user finished or dismissed.
    pub dismissed_tips: Vec<String>,
    /// Last "what's new" version the user has seen.
    pub last_seen_version: String,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            theme: "system".into(),
            auto_lock_minutes: 10,
            lock_on_sleep: true,
            lock_on_session_lock: true,
            lock_on_minimize: false,
            clipboard_clear_seconds: 90,
            close_to_tray: true,
            launch_at_startup: false,
            quick_access_shortcut: "CommandOrControl+Shift+Space".into(),
            screen_capture_protection: true,
            breach_check: false,
            site_icons: false,
            require_password_days: 14,
            update_channel: "stable".into(),
            auto_update: true,
            density: "comfortable".into(),
            dismissed_tips: vec![],
            last_seen_version: String::new(),
        }
    }
}

impl Settings {
    pub fn path(dir: &Path) -> PathBuf {
        dir.join("settings.json")
    }

    pub fn load(dir: &Path) -> Self {
        std::fs::read(Self::path(dir))
            .ok()
            .and_then(|raw| serde_json::from_slice::<Settings>(&raw).ok())
            .map(|s| s.sanitized())
            .unwrap_or_default()
    }

    pub fn save(&self, dir: &Path) -> std::io::Result<()> {
        std::fs::create_dir_all(dir)?;
        let tmp = dir.join("settings.json.tmp");
        std::fs::write(&tmp, serde_json::to_vec_pretty(self).expect("settings serialize"))?;
        std::fs::rename(tmp, Self::path(dir))
    }

    /// Clamps values a hand-edited file could push out of safe ranges.
    pub fn sanitized(mut self) -> Self {
        if !matches!(self.theme.as_str(), "system" | "light" | "dark") {
            self.theme = "system".into();
        }
        self.auto_lock_minutes = self.auto_lock_minutes.min(24 * 60);
        self.clipboard_clear_seconds = self.clipboard_clear_seconds.min(600);
        self.require_password_days = self.require_password_days.min(90);
        if !matches!(self.update_channel.as_str(), "stable" | "beta") {
            self.update_channel = "stable".into();
        }
        if !matches!(self.density.as_str(), "comfortable" | "compact") {
            self.density = "comfortable".into();
        }
        self
    }
}
