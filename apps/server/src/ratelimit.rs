//! Small in-memory fixed-window limiter for unauthenticated endpoints (per IP and per
//! account). Good enough for a single instance; swap for Redis when scaling out.

use std::collections::HashMap;
use std::sync::Mutex;
use std::time::{Duration, Instant};

#[derive(Default)]
pub struct Limiter {
    windows: Mutex<HashMap<String, (Instant, u32)>>,
}

impl Limiter {
    /// True if the call is allowed.
    pub fn check(&self, key: &str, max: u32, window: Duration) -> bool {
        let mut g = self.windows.lock().expect("limiter");
        let now = Instant::now();
        if g.len() > 50_000 {
            g.retain(|_, (start, _)| now.duration_since(*start) < window);
        }
        let entry = g.entry(key.to_string()).or_insert((now, 0));
        if now.duration_since(entry.0) >= window {
            *entry = (now, 0);
        }
        entry.1 += 1;
        entry.1 <= max
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn limits_within_window() {
        let l = Limiter::default();
        for _ in 0..3 {
            assert!(l.check("a", 3, Duration::from_secs(60)));
        }
        assert!(!l.check("a", 3, Duration::from_secs(60)));
        assert!(l.check("b", 3, Duration::from_secs(60)));
    }
}
