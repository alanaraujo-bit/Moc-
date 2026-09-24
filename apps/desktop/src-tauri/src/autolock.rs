//! Background monitor that locks the vault on inactivity, workstation lock and sleep.

use std::sync::Arc;
use std::time::{Duration, SystemTime};
use tauri::{AppHandle, Manager};

use crate::commands::lock_now;
use crate::platform;
use crate::state::AppState;

const TICK: Duration = Duration::from_secs(2);

pub fn spawn(app: AppHandle) {
    std::thread::Builder::new()
        .name("moco-autolock".into())
        .spawn(move || {
            let mut last_wall = SystemTime::now();
            loop {
                std::thread::sleep(TICK);
                let st = app.state::<Arc<AppState>>();
                let settings = st.settings();
                let unlocked = st.account.lock().map(|g| g.as_ref().is_some_and(|a| a.is_unlocked())).unwrap_or(false);

                // A wall-clock jump much larger than our tick means the machine slept.
                let now = SystemTime::now();
                let gap = now.duration_since(last_wall).unwrap_or_default();
                last_wall = now;
                if !unlocked {
                    continue;
                }
                if settings.lock_on_sleep && gap > TICK + Duration::from_secs(20) {
                    lock_now(&app, "sleep");
                    continue;
                }
                if settings.lock_on_session_lock && platform::session_locked() {
                    lock_now(&app, "session");
                    continue;
                }
                if settings.auto_lock_minutes > 0 && platform::idle_millis() >= settings.auto_lock_minutes as u64 * 60_000 {
                    lock_now(&app, "idle");
                }
            }
        })
        .expect("spawn autolock thread");
}
