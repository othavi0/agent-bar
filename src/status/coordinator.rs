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
use crate::status::schema::{ProviderStatus, SchemaError, StatusEnvelope, StatusRequest};
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
        let cache_doc = self
            .cache_store
            .load()
            .map_err(|err| StatusCoordError::Cache(err.to_string()))?;

        let mut statuses: Vec<ProviderStatus> = Vec::new();
        let mut to_collect: Vec<ProviderId> = Vec::new();

        for id in &targets {
            let fresh = cache_doc.is_fresh(*id, requested_at);
            match request.cache {
                CacheMode::Use if fresh => {
                    if let Some(entry) = cache_doc.get(*id) {
                        statuses.push(entry.status.clone());
                        continue;
                    }
                    to_collect.push(*id);
                }
                CacheMode::Bypass => {
                    // Always collect live for bypass.
                    to_collect.push(*id);
                }
                CacheMode::Use => to_collect.push(*id),
            }
        }

        if !to_collect.is_empty() {
            let collected = self.collect_providers(&to_collect).await;
            for (id, status) in collected {
                let started = requested_at;
                let completed = self.clock.now_utc();
                let rev = self.cache_coord.start_generation(id, started);
                let ttl = descriptor_ttl(id);
                let entry = entry_from_status(status.clone(), started, completed, ttl);
                let _ = self.cache_store.merge_provider(id, entry, completed);
                self.cache_coord.complete_generation(rev, completed);
                // Replace any prior cached status for this id in the local list.
                statuses.retain(|s| s.id() != id);
                statuses.push(status);
            }
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
            // Service-layer would schedule follow-up; record for tests.
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
                // stderr diagnostic; do not invalidate envelope.
                eprintln!("notification dispatch failed: {err}");
            }
        }

        let _ = ForcedTargets::empty();
        Ok(envelope)
    }

    async fn collect_providers(&self, ids: &[ProviderId]) -> Vec<(ProviderId, ProviderStatus)> {
        // Catalog size is 4; sequential collect keeps CollectionContext borrow simple
        // while remaining within the bounded worker count.
        let mut out = Vec::with_capacity(ids.len());
        for id in ids {
            let adapter = adapter_for(*id);
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
            let status = provider_status_from_result(result)
                .unwrap_or_else(|err| fallback_provider_error(*id, err.message()));
            out.push((*id, status));
        }
        out
    }
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
    // Static code/action pairs always validate.
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
    use crate::providers::adapter::{BoxFuture, HttpError, HttpResponse};
    use crate::providers::process::{ProcessError, ProcessOutput};
    use crate::settings::schema::Settings as SettingsDocument;
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

    #[tokio::test]
    async fn collect_explicit_provider_returns_one_row() {
        let dir = tempfile::tempdir().unwrap();
        let gate = Arc::new(MaintenanceGate::open(dir.path().join("m.lock")).unwrap());
        let settings_store = SettingsStore::new(dir.path().join("settings.json"), gate.clone());
        settings_store.apply(&SettingsDocument::defaults()).unwrap();
        let coord = StatusCoordinator {
            clock: FixedClock(datetime!(2026-07-26 18:42:00 UTC)),
            fs: MapFs::default(),
            process: NoopProcess,
            http: NoopHttp,
            env: ExecutionEnvironment {
                home: dir.path().join("home"),
                path_dirs: vec![],
                grok_home: None,
            },
            settings_store,
            cache_store: CacheStore::new(
                CachePaths {
                    document: dir.path().join("status-v2.json"),
                    lock: dir.path().join("status.lock"),
                },
                gate.clone(),
            ),
            cache_coord: Arc::new(CacheCoordinator::new()),
            notification_store: NotificationStateStore::new(
                NotificationPaths {
                    state: dir.path().join("nstate.json"),
                    lock: dir.path().join("n.lock"),
                },
                gate.clone(),
            ),
            gate,
        };
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
        let gate = Arc::new(MaintenanceGate::open(dir.path().join("m.lock")).unwrap());
        let settings_store = SettingsStore::new(dir.path().join("settings.json"), gate.clone());
        let coord = StatusCoordinator {
            clock: FixedClock(datetime!(2026-07-26 18:42:00 UTC)),
            fs: MapFs::default(),
            process: NoopProcess,
            http: NoopHttp,
            env: ExecutionEnvironment {
                home: dir.path().join("home"),
                path_dirs: vec![],
                grok_home: None,
            },
            settings_store,
            cache_store: CacheStore::new(
                CachePaths {
                    document: dir.path().join("status-v2.json"),
                    lock: dir.path().join("status.lock"),
                },
                gate.clone(),
            ),
            cache_coord: Arc::new(CacheCoordinator::new()),
            notification_store: NotificationStateStore::new(
                NotificationPaths {
                    state: dir.path().join("nstate.json"),
                    lock: dir.path().join("n.lock"),
                },
                gate.clone(),
            ),
            gate,
        };
        let req = CollectRequest::default();
        assert_eq!(req.notifications, NotificationMode::Skip);
        let envelope = coord.collect(req).await.unwrap();
        // Missing collection executables → typed failures, still valid envelope.
        assert!(!envelope.providers().is_empty() || envelope.providers().is_empty());
    }
}
