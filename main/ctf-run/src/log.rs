//! Progress lines on stderr, stamped with milliseconds since the process started.

use std::sync::OnceLock;
use std::time::{Duration, Instant};

static T0: OnceLock<Instant> = OnceLock::new();

pub fn init() {
    T0.get_or_init(Instant::now);
}

pub fn elapsed() -> Duration {
    T0.get_or_init(Instant::now).elapsed()
}

pub fn ms(d: Duration) -> f64 {
    d.as_secs_f64() * 1e3
}

/// `[  1234.5] tag  message`
pub fn line(tag: &str, msg: &str) {
    eprintln!("[{:8.1}] {tag:<9} {msg}", ms(elapsed()));
}

#[macro_export]
macro_rules! say {
    ($tag:expr, $($arg:tt)*) => { $crate::log::line($tag, &format!($($arg)*)) };
}
