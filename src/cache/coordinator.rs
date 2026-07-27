//! In-process generation tracking for cache-use / cache-bypass coordination.

use std::collections::HashSet;
use std::sync::Mutex;

use time::OffsetDateTime;

use crate::cli::ProviderId;

/// Target set for forced collections (`all` dominates).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ForcedTargets {
    All,
    Providers(HashSet<ProviderId>),
}

impl ForcedTargets {
    pub fn empty() -> Self {
        Self::Providers(HashSet::new())
    }

    pub fn is_empty(&self) -> bool {
        match self {
            Self::All => false,
            Self::Providers(set) => set.is_empty(),
        }
    }

    pub fn union(&mut self, other: ForcedTargets) {
        match (&self, other) {
            (Self::All, _) | (_, ForcedTargets::All) => *self = Self::All,
            (Self::Providers(a), ForcedTargets::Providers(b)) => {
                let mut merged = a.clone();
                merged.extend(b);
                *self = Self::Providers(merged);
            }
        }
    }

    pub fn contains(&self, id: ProviderId) -> bool {
        match self {
            Self::All => true,
            Self::Providers(set) => set.contains(&id),
        }
    }

    pub fn take(&mut self) -> ForcedTargets {
        std::mem::take(self)
    }
}

/// One live generation record for CACHE-010 bypass acceptance.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GenerationRecord {
    pub provider: ProviderId,
    pub started_at: OffsetDateTime,
    pub completed_at: Option<OffsetDateTime>,
    pub revision: u64,
}

/// Coordinates pending forced targets and generation timestamps.
#[derive(Debug, Default)]
pub struct CacheCoordinator {
    inner: Mutex<CacheCoordinatorInner>,
}

#[derive(Debug, Default)]
struct CacheCoordinatorInner {
    pending_forced: ForcedTargets,
    active: bool,
    generations: Vec<GenerationRecord>,
    next_revision: u64,
}

impl Default for ForcedTargets {
    fn default() -> Self {
        Self::empty()
    }
}

impl CacheCoordinator {
    pub fn new() -> Self {
        Self::default()
    }

    /// Union forced targets while a collection is active (CACHE-011/012).
    pub fn retain_forced(&self, targets: ForcedTargets) {
        let mut inner = self.inner.lock().unwrap_or_else(|e| e.into_inner());
        if inner.active {
            inner.pending_forced.union(targets);
        } else {
            inner.pending_forced = targets;
        }
    }

    pub fn begin_collection(&self) {
        let mut inner = self.inner.lock().unwrap_or_else(|e| e.into_inner());
        inner.active = true;
    }

    /// Complete collection and return the union of pending forced targets to run next.
    pub fn complete_collection(&self) -> ForcedTargets {
        let mut inner = self.inner.lock().unwrap_or_else(|e| e.into_inner());
        inner.active = false;
        inner.pending_forced.take()
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
    fn all_dominates_forced_union() {
        let coord = CacheCoordinator::new();
        coord.begin_collection();
        coord.retain_forced(ForcedTargets::Providers(HashSet::from([
            ProviderId::Claude,
        ])));
        coord.retain_forced(ForcedTargets::All);
        let pending = coord.complete_collection();
        assert_eq!(pending, ForcedTargets::All);
    }

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
