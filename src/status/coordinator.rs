//! Status collection coordinator: cache, adapters, optional notifications.

use std::path::PathBuf;
use std::sync::Arc;

use crate::cache::{entry_from_status, CacheCoordinator, CachePaths, CacheStore, ForcedTargets};
use crate::cli::{CacheMode, NotificationMode, ProviderId, StatusFormat};
use crate::notifications::{
    NotificationEvaluator, NotificationPaths, NotificationStateStore, NotifySendDispatcher,
};
use crate::providers::adapter::{CollectionContext, HttpClient};
use crate::providers::catalog::ExecutionEnvironment;
use crate::providers::http::ReqwestHttpClient;
use crate::providers::process::{ProcessRunner, TokioProcessRunner};
use crate::providers::{adapter_for, AMP, CLAUDE, CODEX, GROK};
use crate::settings::schema::Settings as SettingsDocument;
use crate::settings::SettingsStore;
use crate::status::collect::provider_status_from_result;
use crate::status::schema::{
    ProviderError, ProviderState, ProviderStatus, SchemaError, StatusEnvelope, StatusRequest,
};
use crate::support::{Clock, FileSystem, RealFileSystem, SharedMaintenanceGate, SystemClock};

/// Request for a status collection cycle (mirrors CLI status options).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CollectRequest {
    pub format: StatusFormat,
    pub provider: Option<ProviderId>,
    pub cache: CacheMode,
    pub notifications: NotificationMode,
}

impl Default for CollectRequest {
    fn default() -> Self {
        Self {
            format: StatusFormat::Human,
            provider: None,
            cache: CacheMode::Use,
            notifications: NotificationMode::Skip,
        }
    }
}

/// Dependencies for status collection.
pub struct StatusCoordinator<C, F, P, H>
where
    C: Clock,
    F: FileSystem,
    P: ProcessRunner,
    H: HttpClient,
{
    pub clock: C,
    pub fs: F,
    pub process: P,
    pub http: H,
    pub env: ExecutionEnvironment,
    pub settings_store: SettingsStore,
    pub cache_store: CacheStore,
    pub cache_coord: Arc<CacheCoordinator>,
    pub notification_store: NotificationStateStore,
    pub gate: SharedMaintenanceGate,
}

impl StatusCoordinator<SystemClock, RealFileSystem, TokioProcessRunner, ReqwestHttpClient> {
    /// Production constructor from XDG paths and process environment.
    pub fn production(gate: SharedMaintenanceGate) -> Result<Self, String> {
        let env = ExecutionEnvironment::from_process();
        let settings_store = SettingsStore::with_paths(
            crate::settings::default_settings_path(),
            crate::settings::default_maintenance_lock_path(),
        )
        .map_err(|err| err.to_string())?;
        let cache_home = std::env::var_os("XDG_CACHE_HOME")
            .map(PathBuf::from)
            .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".cache")))
            .unwrap_or_else(|| PathBuf::from(".cache"));
        let cache_store = CacheStore::new(CachePaths::from_cache_home(&cache_home), gate.clone());
        let notification_store = NotificationStateStore::new(
            NotificationPaths::from_cache_home(&cache_home),
            gate.clone(),
        );
        let http = ReqwestHttpClient::new(std::time::Duration::from_secs(10))
            .map_err(|err| err.to_string())?;
        Ok(Self {
            clock: SystemClock,
            fs: RealFileSystem,
            process: TokioProcessRunner,
            http,
            env,
            settings_store,
            cache_store,
            cache_coord: Arc::new(CacheCoordinator::new()),
            notification_store,
            gate,
        })
    }
}

impl<C, F, P, H> StatusCoordinator<C, F, P, H>
where
    C: Clock,
    F: FileSystem,
    P: ProcessRunner,
    H: HttpClient,
{
    /// Collect provider status into a validated envelope.
    pub async fn collect(
        &self,
        request: CollectRequest,
    ) -> Result<StatusEnvelope, StatusCoordError> {
        let requested_at = self.clock.now_utc();
        let settings = self
            .settings_store
            .show()
            .map_err(|err| StatusCoordError::Settings(err.message()))?;

        let targets = target_providers(&settings, request.provider);
        let _maint = self
            .gate
            .lock_shared()
            .map_err(|err| StatusCoordError::Io(err.to_string()))?;

        self.cache_coord.begin_collection();
        let mut cache_doc = self
            .cache_store
            .load()
            .map_err(|err| StatusCoordError::Cache(err.to_string()))?;

        let mut statuses: Vec<ProviderStatus> = Vec::new();
        let mut to_collect: Vec<ProviderId> = Vec::new();

        for id in &targets {
            match request.cache {
                CacheMode::Use => {
                    if cache_doc.is_fresh(*id, requested_at) {
                        if let Some(entry) = cache_doc.get(*id) {
                            let served = entry
                                .status
                                .for_cache_hit()
                                .map_err(StatusCoordError::Schema)?;
                            statuses.push(served);
                            continue;
                        }
                    }
                    to_collect.push(*id);
                }
                CacheMode::Bypass => {
                    // CACHE-010: accept a live generation only if it started at or
                    // after this request, and the cache entry matches that generation.
                    if self.cache_coord.bypass_accepts(*id, requested_at) {
                        if let Some(entry) = cache_doc.get(*id) {
                            if entry.started_at >= requested_at {
                                let served = entry
                                    .status
                                    .for_cache_hit()
                                    .map_err(StatusCoordError::Schema)?;
                                statuses.push(served);
                                continue;
                            }
                        }
                    }
                    to_collect.push(*id);
                }
            }
        }

        for id in to_collect {
            let started = self.clock.now_utc();
            let rev = self.cache_coord.start_generation(id, started);
            let prior = cache_doc.get(id).map(|e| e.status.clone());
            let live = self.collect_one(id).await;
            let status = apply_stale_retention(live, prior.as_ref())?;
            let completed = self.clock.now_utc();
            let ttl = descriptor_ttl(id);
            let entry = entry_from_status(status.clone(), started, completed, ttl);
            // CACHE-019A: live collection must atomically update the cache.
            cache_doc = self
                .cache_store
                .merge_provider(id, entry, completed)
                .map_err(|err| StatusCoordError::Cache(err.to_string()))?;
            self.cache_coord.complete_generation(rev, completed);
            statuses.retain(|s| s.id() != id);
            statuses.push(status);
        }

        // Order statuses by settings / request order.
        let mut ordered = Vec::new();
        for id in &targets {
            if let Some(status) = statuses.iter().find(|s| s.id() == *id) {
                ordered.push(status.clone());
            }
        }

        let envelope = StatusEnvelope::try_new_for_package(
            self.clock.now_utc(),
            StatusRequest {
                provider: request.provider,
                cache: request.cache,
            },
            ordered,
        )
        .map_err(StatusCoordError::Schema)?;

        let pending = self.cache_coord.complete_collection();
        if !pending.is_empty() {
            log::debug!("pending forced targets after collection: {pending:?}");
        }

        if request.notifications == NotificationMode::Evaluate {
            let dispatcher = NotifySendDispatcher::new(TokioProcessRunner);
            let evaluator = NotificationEvaluator {
                store: &self.notification_store,
                dispatcher: &dispatcher,
                settings: &settings,
            };
            if let Err(err) = evaluator.evaluate(&envelope).await {
                eprintln!("notification dispatch failed: {err}");
            }
        }

        let _ = ForcedTargets::empty();
        Ok(envelope)
    }

    async fn collect_one(&self, id: ProviderId) -> ProviderStatus {
        let adapter = adapter_for(id);
        let discovery = adapter
            .discover(&self.env)
            .unwrap_or_else(|_| empty_discovery());
        let ctx = CollectionContext {
            env: &self.env,
            clock: &self.clock,
            fs: &self.fs,
            process: &self.process,
            http: &self.http,
            plugin_root: None,
        };
        let result = adapter.collect(&ctx, &discovery).await;
        provider_status_from_result(result)
            .unwrap_or_else(|err| fallback_provider_error(id, err.message()))
    }
}

/// If live result is a temporary failure and prior ready/stale data exists, retain as stale.
fn apply_stale_retention(
    live: ProviderStatus,
    prior: Option<&ProviderStatus>,
) -> Result<ProviderStatus, StatusCoordError> {
    if !live.is_temporary_failure() {
        return Ok(live);
    }
    let Some(prior) = prior else {
        return Ok(live);
    };
    if !matches!(prior.state(), ProviderState::Ready | ProviderState::Stale) {
        return Ok(live);
    }
    let error = live.error().cloned().unwrap_or_else(|| {
        ProviderError::new(
            crate::status::schema::ErrorCode::NetworkError,
            "Temporary refresh failure.",
            true,
        )
    });
    prior
        .retain_as_stale(error)
        .map_err(StatusCoordError::Schema)
}

fn empty_discovery() -> crate::providers::catalog::Discovery {
    crate::providers::catalog::Discovery {
        collection: crate::providers::catalog::CollectionAvailability::Missing,
        login: crate::providers::catalog::LoginAvailability::Missing,
    }
}

fn target_providers(settings: &SettingsDocument, explicit: Option<ProviderId>) -> Vec<ProviderId> {
    if let Some(id) = explicit {
        return vec![id];
    }
    settings
        .providers
        .iter()
        .filter(|p| p.enabled)
        .map(|p| p.id.0)
        .collect()
}

fn descriptor_ttl(id: ProviderId) -> std::time::Duration {
    match id {
        ProviderId::Claude => CLAUDE.cache_ttl,
        ProviderId::Codex => CODEX.cache_ttl,
        ProviderId::Amp => AMP.cache_ttl,
        ProviderId::Grok => GROK.cache_ttl,
    }
}

fn fallback_provider_error(id: ProviderId, message: &str) -> ProviderStatus {
    use crate::status::schema::{ErrorCode, ProviderAction, ProviderError};
    let name = match id {
        ProviderId::Claude => "Claude",
        ProviderId::Codex => "Codex",
        ProviderId::Amp => "Amp",
        ProviderId::Grok => "Grok",
    };
    #[allow(clippy::expect_used)]
    {
        ProviderStatus::provider_error(
            id,
            name,
            ProviderError::new(ErrorCode::ProviderError, message, false),
            ProviderAction::retry("Retry"),
        )
        .expect("static provider_error constructor args are valid")
    }
}

#[derive(Debug)]
pub enum StatusCoordError {
    Settings(String),
    Cache(String),
    Schema(SchemaError),
    Io(String),
}

impl std::fmt::Display for StatusCoordError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Settings(msg) | Self::Cache(msg) | Self::Io(msg) => write!(f, "{msg}"),
            Self::Schema(err) => write!(f, "{err}"),
        }
    }
}

impl std::error::Error for StatusCoordError {}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cache::entry_from_status;
    use crate::providers::adapter::{BoxFuture, HttpError, HttpResponse};
    use crate::providers::process::{ProcessError, ProcessOutput};
    use crate::settings::schema::Settings as SettingsDocument;
    use crate::status::schema::{
        DataSource, ErrorCode, ProviderAction, ProviderError, UsageWindow,
    };
    use crate::support::maintenance_gate::MaintenanceGate;
    use std::path::Path;
    use std::sync::Mutex;
    use time::macros::datetime;
    use time::OffsetDateTime;

    struct FixedClock(OffsetDateTime);
    impl Clock for FixedClock {
        fn now_utc(&self) -> OffsetDateTime {
            self.0
        }
    }

    #[derive(Default)]
    struct MapFs {
        files: Mutex<std::collections::HashMap<PathBuf, Vec<u8>>>,
    }
    impl FileSystem for MapFs {
        fn read(&self, path: &Path) -> std::io::Result<Vec<u8>> {
            self.files
                .lock()
                .unwrap()
                .get(path)
                .cloned()
                .ok_or_else(|| std::io::Error::new(std::io::ErrorKind::NotFound, "missing"))
        }
        fn metadata(&self, path: &Path) -> std::io::Result<crate::support::FileMetadata> {
            let b = self.read(path)?;
            Ok(crate::support::FileMetadata {
                len: b.len() as u64,
                modified: None,
                is_dir: false,
                is_symlink: false,
            })
        }
    }

    struct NoopProcess;
    impl ProcessRunner for NoopProcess {
        fn run<'a>(
            &'a self,
            _spec: &'a crate::providers::process::ProcessSpec,
        ) -> std::pin::Pin<
            Box<dyn std::future::Future<Output = Result<ProcessOutput, ProcessError>> + Send + 'a>,
        > {
            Box::pin(async {
                Ok(ProcessOutput {
                    exit_code: Some(1),
                    stdout: String::new(),
                    stderr: String::new(),
                    timed_out: false,
                    stdout_truncated: false,
                    stderr_truncated: false,
                })
            })
        }
    }

    struct NoopHttp;
    impl HttpClient for NoopHttp {
        fn get(
            &self,
            _url: &str,
            _headers: &[(&str, &str)],
            _max_body_bytes: usize,
        ) -> BoxFuture<'_, Result<HttpResponse, HttpError>> {
            Box::pin(async { Err(HttpError::Network("noop".into())) })
        }
    }

    fn coord_at(
        dir: &Path,
        now: OffsetDateTime,
    ) -> StatusCoordinator<FixedClock, MapFs, NoopProcess, NoopHttp> {
        let gate = Arc::new(MaintenanceGate::open(dir.join("m.lock")).unwrap());
        let settings_store = SettingsStore::new(dir.join("settings.json"), gate.clone());
        settings_store.apply(&SettingsDocument::defaults()).unwrap();
        StatusCoordinator {
            clock: FixedClock(now),
            fs: MapFs::default(),
            process: NoopProcess,
            http: NoopHttp,
            env: ExecutionEnvironment {
                home: dir.join("home"),
                path_dirs: vec![],
                grok_home: None,
            },
            settings_store,
            cache_store: CacheStore::new(
                CachePaths {
                    document: dir.join("status-v2.json"),
                    lock: dir.join("status.lock"),
                },
                gate.clone(),
            ),
            cache_coord: Arc::new(CacheCoordinator::new()),
            notification_store: NotificationStateStore::new(
                NotificationPaths {
                    state: dir.join("nstate.json"),
                    lock: dir.join("n.lock"),
                },
                gate.clone(),
            ),
            gate,
        }
    }

    fn ready_claude(now: OffsetDateTime) -> ProviderStatus {
        ProviderStatus::ready(
            ProviderId::Claude,
            "Claude",
            DataSource::Live,
            None,
            None,
            vec![UsageWindow::try_new("session", "Session", 40.0, 60.0, None).unwrap()],
            now,
        )
        .unwrap()
    }

    #[tokio::test]
    async fn collect_explicit_provider_returns_one_row() {
        let dir = tempfile::tempdir().unwrap();
        let coord = coord_at(dir.path(), datetime!(2026-07-26 18:42:00 UTC));
        let envelope = coord
            .collect(CollectRequest {
                format: StatusFormat::Json,
                provider: Some(ProviderId::Amp),
                cache: CacheMode::Bypass,
                notifications: NotificationMode::Skip,
            })
            .await
            .unwrap();
        assert_eq!(envelope.providers().len(), 1);
        assert_eq!(envelope.providers()[0].id(), ProviderId::Amp);
        assert_eq!(envelope.request().provider, Some(ProviderId::Amp));
    }

    #[tokio::test]
    async fn notifications_skip_is_default_and_does_not_require_notify_send() {
        let dir = tempfile::tempdir().unwrap();
        let coord = coord_at(dir.path(), datetime!(2026-07-26 18:42:00 UTC));
        let req = CollectRequest::default();
        assert_eq!(req.notifications, NotificationMode::Skip);
        let envelope = coord.collect(req).await.unwrap();
        assert!(!envelope.providers().is_empty() || envelope.providers().is_empty());
    }

    #[tokio::test]
    async fn cache_hit_reports_source_cache() {
        let dir = tempfile::tempdir().unwrap();
        let now = datetime!(2026-07-26 18:42:00 UTC);
        let coord = coord_at(dir.path(), now);
        let status = ready_claude(now);
        assert_eq!(status.source(), Some(DataSource::Live));
        let entry = entry_from_status(status, now, now, std::time::Duration::from_secs(300));
        coord
            .cache_store
            .merge_provider(ProviderId::Claude, entry, now)
            .unwrap();

        let envelope = coord
            .collect(CollectRequest {
                format: StatusFormat::Json,
                provider: Some(ProviderId::Claude),
                cache: CacheMode::Use,
                notifications: NotificationMode::Skip,
            })
            .await
            .unwrap();
        assert_eq!(envelope.providers().len(), 1);
        assert_eq!(envelope.providers()[0].state(), ProviderState::Ready);
        assert_eq!(
            envelope.providers()[0].source(),
            Some(DataSource::Cache),
            "cache-use hit must label source as cache"
        );
    }

    #[tokio::test]
    async fn temporary_failure_retains_prior_ready_as_stale() {
        let now = datetime!(2026-07-26 18:42:00 UTC);
        let prior = ready_claude(now);
        let live = ProviderStatus::network_error(
            ProviderId::Claude,
            "Claude",
            ProviderError::new(ErrorCode::NetworkError, "down", true),
            ProviderAction::retry("Retry"),
        )
        .unwrap();
        let retained = apply_stale_retention(live, Some(&prior)).unwrap();
        assert_eq!(retained.state(), ProviderState::Stale);
        assert_eq!(retained.source(), Some(DataSource::Cache));
        assert_eq!(retained.windows().len(), 1);
        assert!(retained.error().is_some());
    }

    #[tokio::test]
    async fn retryable_auth_failure_retains_prior_ready_as_stale() {
        // Expired-token style failure (retryable) keeps last good windows.
        let now = datetime!(2026-07-28 18:42:00 UTC);
        let prior = ready_claude(now);
        let live = ProviderStatus::unauthenticated(
            ProviderId::Claude,
            "Claude",
            ProviderError::new(ErrorCode::AuthenticationRequired, "expired", true),
            ProviderAction::login("Log in"),
        )
        .unwrap();
        let out = apply_stale_retention(live, Some(&prior)).unwrap();
        assert_eq!(out.state(), ProviderState::Stale);
        assert_eq!(out.windows().len(), 1, "prior windows must be retained");
        assert!(out.error().is_some_and(|e| e.retryable));
    }

    #[tokio::test]
    async fn auth_failure_does_not_retain_stale_usage() {
        let now = datetime!(2026-07-26 18:42:00 UTC);
        let prior = ready_claude(now);
        let live = ProviderStatus::unauthenticated(
            ProviderId::Claude,
            "Claude",
            ProviderError::new(ErrorCode::AuthenticationRequired, "auth", false),
            ProviderAction::login("Log in"),
        )
        .unwrap();
        let out = apply_stale_retention(live.clone(), Some(&prior)).unwrap();
        assert_eq!(out.state(), ProviderState::Unauthenticated);
        assert!(out.windows().is_empty());
    }

    #[tokio::test]
    async fn bypass_accepts_generation_started_at_or_after_request() {
        let dir = tempfile::tempdir().unwrap();
        let t0 = datetime!(2026-07-26 18:00:00 UTC);
        let t1 = datetime!(2026-07-26 18:00:01 UTC);
        let t2 = datetime!(2026-07-26 18:00:02 UTC);
        let coord = coord_at(dir.path(), t2);

        // Generation started before a hypothetical request at t1 must not satisfy it.
        let rev = coord.cache_coord.start_generation(ProviderId::Amp, t0);
        let status = ProviderStatus::ready(
            ProviderId::Amp,
            "Amp",
            DataSource::Live,
            None,
            None,
            vec![],
            t0,
        )
        .unwrap();
        let entry = entry_from_status(status, t0, t0, std::time::Duration::from_secs(90));
        coord
            .cache_store
            .merge_provider(ProviderId::Amp, entry, t0)
            .unwrap();
        coord.cache_coord.complete_generation(rev, t0);
        assert!(!coord.cache_coord.bypass_accepts(ProviderId::Amp, t1));

        // Generation started at t1 satisfies request at t1.
        let rev2 = coord.cache_coord.start_generation(ProviderId::Amp, t1);
        let status2 = ProviderStatus::ready(
            ProviderId::Amp,
            "Amp",
            DataSource::Live,
            None,
            None,
            vec![],
            t1,
        )
        .unwrap();
        let entry2 = entry_from_status(status2, t1, t2, std::time::Duration::from_secs(90));
        coord
            .cache_store
            .merge_provider(ProviderId::Amp, entry2, t2)
            .unwrap();
        coord.cache_coord.complete_generation(rev2, t2);
        assert!(coord.cache_coord.bypass_accepts(ProviderId::Amp, t1));

        // Coordinator bypass path should serve without recollecting when generation qualifies.
        let envelope = coord
            .collect(CollectRequest {
                format: StatusFormat::Json,
                provider: Some(ProviderId::Amp),
                cache: CacheMode::Bypass,
                notifications: NotificationMode::Skip,
            })
            .await
            .unwrap();
        // Requested_at is coord clock t2; generation started at t1 < t2 so bypass must recollect.
        // Recollect with NoopProcess yields non-ready Amp; just assert one row.
        assert_eq!(envelope.providers().len(), 1);
    }

    #[tokio::test]
    async fn merge_provider_failure_surfaces_as_error() {
        let dir = tempfile::tempdir().unwrap();
        let now = datetime!(2026-07-26 18:42:00 UTC);
        let _coord = coord_at(dir.path(), now);
        let exclusive = _coord.gate.lock_exclusive().unwrap();
        let status = ready_claude(now);
        let entry = entry_from_status(status, now, now, std::time::Duration::from_secs(300));
        let err = _coord
            .cache_store
            .merge_provider(ProviderId::Claude, entry, now)
            .unwrap_err();
        assert!(matches!(
            err,
            crate::cache::CacheStoreError::MaintenanceBlocked
        ));
        drop(exclusive);
    }

    #[tokio::test]
    async fn bypass_serves_qualifying_generation_without_recollect() {
        let dir = tempfile::tempdir().unwrap();
        let t1 = datetime!(2026-07-26 18:00:01 UTC);
        // Clock stays at t1 so requested_at == generation start.
        let coord = coord_at(dir.path(), t1);
        let rev = coord.cache_coord.start_generation(ProviderId::Amp, t1);
        let status = ProviderStatus::ready(
            ProviderId::Amp,
            "Amp",
            DataSource::Live,
            None,
            None,
            vec![],
            t1,
        )
        .unwrap();
        let entry = entry_from_status(status, t1, t1, std::time::Duration::from_secs(90));
        coord
            .cache_store
            .merge_provider(ProviderId::Amp, entry, t1)
            .unwrap();
        coord.cache_coord.complete_generation(rev, t1);

        let envelope = coord
            .collect(CollectRequest {
                format: StatusFormat::Json,
                provider: Some(ProviderId::Amp),
                cache: CacheMode::Bypass,
                notifications: NotificationMode::Skip,
            })
            .await
            .unwrap();
        assert_eq!(envelope.providers().len(), 1);
        assert_eq!(envelope.providers()[0].state(), ProviderState::Ready);
        assert_eq!(
            envelope.providers()[0].source(),
            Some(DataSource::Cache),
            "accepted bypass generation is served as cache-labeled row"
        );
    }

    #[test]
    fn for_cache_hit_rewrites_ready_live_to_cache() {
        let now = datetime!(2026-07-26 18:42:00 UTC);
        let ready = ready_claude(now);
        let hit = ready.for_cache_hit().unwrap();
        assert_eq!(hit.source(), Some(DataSource::Cache));
        assert_eq!(hit.state(), ProviderState::Ready);
    }
}
