//! Injectable wall-clock seam for deterministic tests.

use time::OffsetDateTime;

/// Produces the current UTC instant.
pub trait Clock: Send + Sync {
    fn now_utc(&self) -> OffsetDateTime;
}

/// Production clock backed by the system wall clock.
#[derive(Debug, Default, Clone, Copy)]
pub struct SystemClock;

impl Clock for SystemClock {
    fn now_utc(&self) -> OffsetDateTime {
        OffsetDateTime::now_utc()
    }
}
