//! In-process generation tracking for cache-use / cache-bypass coordination.

use std::sync::Mutex;

use time::OffsetDateTime;

use crate::cli::ProviderId;

/// One live generation record for CACHE-010 bypass acceptance.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GenerationRecord {
    pub provider: ProviderId,
    pub started_at: OffsetDateTime,
    pub completed_at: Option<OffsetDateTime>,
    pub revision: u64,
}

/// Coordinates in-flight collection state and generation timestamps.
#[derive(Debug, Default)]
pub struct CacheCoordinator {
    inner: Mutex<CacheCoordinatorInner>,
}

#[derive(Debug, Default)]
struct CacheCoordinatorInner {
    active: bool,
    generations: Vec<GenerationRecord>,
    next_revision: u64,
}

impl CacheCoordinator {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn begin_collection(&self) {
        let mut inner = self.inner.lock().unwrap_or_else(|e| e.into_inner());
        inner.active = true;
    }

    /// Mark the in-flight collection as complete.
    pub fn complete_collection(&self) {
        let mut inner = self.inner.lock().unwrap_or_else(|e| e.into_inner());
        inner.active = false;
    }

    pub fn start_generation(&self, provider: ProviderId, started_at: OffsetDateTime) -> u64 {
        let mut inner = self.inner.lock().unwrap_or_else(|e| e.into_inner());
        inner.next_revision = inner.next_revision.saturating_add(1);
        let revision = inner.next_revision;
        inner.generations.push(GenerationRecord {
            provider,
            started_at,
            completed_at: None,
            revision,
        });
        revision
    }

    pub fn complete_generation(&self, revision: u64, completed_at: OffsetDateTime) {
        let mut inner = self.inner.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(gen) = inner
            .generations
            .iter_mut()
            .find(|g| g.revision == revision)
        {
            gen.completed_at = Some(completed_at);
        }
    }

    /// CACHE-010: bypass accepts a generation only if started_at >= requested_at.
    pub fn bypass_accepts(&self, provider: ProviderId, requested_at: OffsetDateTime) -> bool {
        let inner = self.inner.lock().unwrap_or_else(|e| e.into_inner());
        inner.generations.iter().any(|g| {
            g.provider == provider && g.completed_at.is_some() && g.started_at >= requested_at
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use time::macros::datetime;

    #[test]
    fn bypass_rejects_generation_started_before_request() {
        let coord = CacheCoordinator::new();
        let started = datetime!(2026-07-26 18:00:00 UTC);
        let requested = datetime!(2026-07-26 18:00:01 UTC);
        let rev = coord.start_generation(ProviderId::Amp, started);
        coord.complete_generation(rev, datetime!(2026-07-26 18:00:02 UTC));
        assert!(!coord.bypass_accepts(ProviderId::Amp, requested));
        let rev2 = coord.start_generation(ProviderId::Amp, requested);
        coord.complete_generation(rev2, datetime!(2026-07-26 18:00:03 UTC));
        assert!(coord.bypass_accepts(ProviderId::Amp, requested));
    }
}
