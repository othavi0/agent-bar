//! Usage threshold notifications (at-least-once, notify-send backend).

pub mod state;

pub use state::{
    NotificationEntry, NotificationLevel, NotificationPaths, NotificationState,
    NotificationStateStore,
};

use std::path::PathBuf;
use std::time::Duration;

use crate::cli::ProviderId;
use crate::providers::process::{ProcessRunner, ProcessSpec};
use crate::settings::schema::{DisplayMetric, Settings as SettingsDocument};
use crate::status::schema::{ProviderState, StatusEnvelope};
use crate::support::redact::strip_ansi_and_controls;

/// Planned notification before dispatch.
#[derive(Debug, Clone, PartialEq)]
pub struct PendingNotification {
    pub provider_id: ProviderId,
    pub provider_name: String,
    pub window_id: String,
    pub window_label: String,
    pub used_percent: f64,
    pub reset_at: Option<time::OffsetDateTime>,
    pub level: NotificationLevel,
}

/// Dispatch backend (production uses `notify-send`).
pub trait NotificationDispatcher: Send + Sync {
    fn dispatch(
        &self,
        pending: &PendingNotification,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<(), String>> + Send + '_>>;
}

/// `notify-send` argv builder and runner.
#[derive(Debug, Clone)]
pub struct NotifySendDispatcher<R: ProcessRunner> {
    pub runner: R,
    pub program: PathBuf,
}

impl<R: ProcessRunner> NotifySendDispatcher<R> {
    pub fn new(runner: R) -> Self {
        Self {
            runner,
            program: PathBuf::from("notify-send"),
        }
    }

    pub fn build_spec(pending: &PendingNotification) -> ProcessSpec {
        let urgency = match pending.level {
            NotificationLevel::Warning => "normal",
            NotificationLevel::Critical => "critical",
        };
        let title = match pending.level {
            NotificationLevel::Warning => {
                format!("{} usage warning", pending.provider_name)
            }
            NotificationLevel::Critical => {
                format!("{} usage critical", pending.provider_name)
            }
        };
        let used = pending.used_percent.round() as i64;
        let body = match pending.reset_at {
            Some(ts) => format!(
                "{}: {}% used. Resets {}.",
                pending.window_label,
                used,
                ts.format(&time::format_description::well_known::Rfc3339)
                    .unwrap_or_else(|_| "unknown".into())
            ),
            None => format!("{}: {}% used.", pending.window_label, used),
        };
        let title = strip_ansi_and_controls(&title);
        let body = strip_ansi_and_controls(&body);
        ProcessSpec::new(
            "notify-send",
            [
                "--app-name=Agent Bar".to_owned(),
                format!("--urgency={urgency}"),
                title,
                body,
            ],
        )
        .with_timeout(Duration::from_secs(5))
        .with_max_output(16 * 1024)
    }
}

impl<R: ProcessRunner + 'static> NotificationDispatcher for NotifySendDispatcher<R> {
    fn dispatch(
        &self,
        pending: &PendingNotification,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<(), String>> + Send + '_>> {
        let mut spec = Self::build_spec(pending);
        spec.program = self.program.clone();
        Box::pin(async move {
            let out = self
                .runner
                .run(&spec)
                .await
                .map_err(|err| err.to_string())?;
            if out.timed_out {
                return Err("notify-send timed out".into());
            }
            match out.exit_code {
                Some(0) => Ok(()),
                Some(code) => Err(format!("notify-send exited {code}")),
                None => Err("notify-send terminated by signal".into()),
            }
        })
    }
}

/// Evaluate envelope windows, dispatch escalations, persist per success.
pub struct NotificationEvaluator<'a, D: NotificationDispatcher> {
    pub store: &'a NotificationStateStore,
    pub dispatcher: &'a D,
    pub settings: &'a SettingsDocument,
}

impl<'a, D: NotificationDispatcher> NotificationEvaluator<'a, D> {
    pub async fn evaluate(&self, envelope: &StatusEnvelope) -> Result<(), String> {
        if !self.settings.notifications.enabled {
            return Ok(());
        }

        let mut state = self.store.load().map_err(|err| err.to_string())?;
        let order: Vec<ProviderId> = self.settings.providers.iter().map(|p| p.id.0).collect();

        // Collect candidates in settings provider order, then window order.
        let mut pending: Vec<PendingNotification> = Vec::new();
        for id in order {
            let Some(provider) = envelope.providers().iter().find(|p| p.id() == id) else {
                continue;
            };
            if provider.state() != ProviderState::Ready {
                // NOTIFY-006: stale/failures do not trigger.
                continue;
            }
            for window in provider.windows() {
                let used = window.used_percent();
                let Some(level) = NotificationLevel::from_used_percent(used) else {
                    // Recovery below 90 → rearm (remove key).
                    state.remove_key(id, window.id(), window.resets_at());
                    continue;
                };
                let prev = state.level_for(id, window.id(), window.resets_at());
                let should_emit = match prev {
                    None => true,
                    Some(prev_level) if level > prev_level => true,
                    Some(_) => false, // same level once (NOTIFY-003)
                };
                if should_emit {
                    pending.push(PendingNotification {
                        provider_id: id,
                        provider_name: provider.name().to_owned(),
                        window_id: window.id().to_owned(),
                        window_label: window.label().to_owned(),
                        used_percent: used,
                        reset_at: window.resets_at(),
                        level,
                    });
                }
            }
        }

        // Persist silent rearms first.
        self.store.save(&state).map_err(|err| err.to_string())?;

        for item in pending {
            match self.dispatcher.dispatch(&item).await {
                Ok(()) => {
                    state.upsert(NotificationEntry {
                        provider_id: item.provider_id.as_str().to_owned(),
                        window_id: item.window_id.clone(),
                        reset_at: item.reset_at,
                        level: item.level,
                    });
                    // Persist after each success (at-least-once algorithm).
                    self.store.save(&state).map_err(|err| err.to_string())?;
                }
                Err(err) => {
                    // Leave key unadvanced; stop later notifications.
                    return Err(err);
                }
            }
        }
        let _ = DisplayMetric::Remaining; // settings display not used for thresholds
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cli::{CacheMode, ProviderId};
    use crate::providers::process::{ProcessError, ProcessOutput, ProcessRunner};
    use crate::status::schema::{
        DataSource, ProviderStatus, StatusEnvelope, StatusRequest, UsageWindow,
    };
    use crate::support::maintenance_gate::MaintenanceGate;
    use std::sync::{Arc, Mutex};
    use time::macros::datetime;

    struct ScriptedNotify {
        pub specs: Mutex<Vec<ProcessSpec>>,
        pub fail: bool,
    }

    impl ProcessRunner for ScriptedNotify {
        fn run<'a>(
            &'a self,
            spec: &'a ProcessSpec,
        ) -> std::pin::Pin<
            Box<dyn std::future::Future<Output = Result<ProcessOutput, ProcessError>> + Send + 'a>,
        > {
            self.specs.lock().unwrap().push(spec.clone());
            let fail = self.fail;
            Box::pin(async move {
                if fail {
                    Ok(ProcessOutput {
                        exit_code: Some(1),
                        stdout: String::new(),
                        stderr: String::new(),
                        timed_out: false,
                        stdout_truncated: false,
                        stderr_truncated: false,
                    })
                } else {
                    Ok(ProcessOutput {
                        exit_code: Some(0),
                        stdout: String::new(),
                        stderr: String::new(),
                        timed_out: false,
                        stdout_truncated: false,
                        stderr_truncated: false,
                    })
                }
            })
        }
    }

    fn envelope_with_used(used: f64) -> StatusEnvelope {
        let window = UsageWindow::try_new("session", "Session", used, 100.0 - used, None).unwrap();
        let provider = ProviderStatus::ready(
            ProviderId::Claude,
            "Claude",
            DataSource::Live,
            None,
            None,
            vec![window],
            datetime!(2026-07-26 18:42:00 UTC),
        )
        .unwrap();
        StatusEnvelope::try_new_for_package(
            datetime!(2026-07-26 18:42:00 UTC),
            StatusRequest {
                provider: None,
                cache: CacheMode::Use,
            },
            vec![provider],
        )
        .unwrap()
    }

    #[tokio::test]
    async fn escalates_warning_then_critical_once_each() {
        let dir = tempfile::tempdir().unwrap();
        let store = NotificationStateStore::new(
            NotificationPaths {
                state: dir.path().join("nstate.json"),
                lock: dir.path().join("n.lock"),
            },
            Arc::new(MaintenanceGate::open(dir.path().join("m.lock")).unwrap()),
        );
        let runner = ScriptedNotify {
            specs: Mutex::new(Vec::new()),
            fail: false,
        };
        let dispatcher = NotifySendDispatcher::new(runner);
        let settings = SettingsDocument::defaults();

        let eval = NotificationEvaluator {
            store: &store,
            dispatcher: &dispatcher,
            settings: &settings,
        };
        eval.evaluate(&envelope_with_used(90.0)).await.unwrap();
        eval.evaluate(&envelope_with_used(90.0)).await.unwrap(); // no second warning
        eval.evaluate(&envelope_with_used(96.0)).await.unwrap();
        let specs = dispatcher.runner.specs.lock().unwrap();
        assert_eq!(specs.len(), 2);
        assert!(specs[0].args.iter().any(|a| a.contains("warning")));
        assert!(specs[1].args.iter().any(|a| a.contains("critical")));
        assert!(specs[0].args.iter().any(|a| a == "--app-name=Agent Bar"));
    }

    #[tokio::test]
    async fn dispatch_failure_does_not_persist_level() {
        let dir = tempfile::tempdir().unwrap();
        let store = NotificationStateStore::new(
            NotificationPaths {
                state: dir.path().join("nstate.json"),
                lock: dir.path().join("n.lock"),
            },
            Arc::new(MaintenanceGate::open(dir.path().join("m.lock")).unwrap()),
        );
        let runner = ScriptedNotify {
            specs: Mutex::new(Vec::new()),
            fail: true,
        };
        let dispatcher = NotifySendDispatcher::new(runner);
        let settings = SettingsDocument::defaults();
        let eval = NotificationEvaluator {
            store: &store,
            dispatcher: &dispatcher,
            settings: &settings,
        };
        let err = eval.evaluate(&envelope_with_used(92.0)).await.unwrap_err();
        assert!(!err.is_empty());
        let state = store.load().unwrap();
        assert!(state.entries.is_empty());
    }

    #[tokio::test]
    async fn disabled_settings_skip_dispatch() {
        let dir = tempfile::tempdir().unwrap();
        let store = NotificationStateStore::new(
            NotificationPaths {
                state: dir.path().join("nstate.json"),
                lock: dir.path().join("n.lock"),
            },
            Arc::new(MaintenanceGate::open(dir.path().join("m.lock")).unwrap()),
        );
        let runner = ScriptedNotify {
            specs: Mutex::new(Vec::new()),
            fail: false,
        };
        let dispatcher = NotifySendDispatcher::new(runner);
        let mut settings = SettingsDocument::defaults();
        settings.notifications.enabled = false;
        let eval = NotificationEvaluator {
            store: &store,
            dispatcher: &dispatcher,
            settings: &settings,
        };
        eval.evaluate(&envelope_with_used(99.0)).await.unwrap();
        assert!(dispatcher.runner.specs.lock().unwrap().is_empty());
    }

    #[test]
    fn notify_send_argv_shape() {
        let pending = PendingNotification {
            provider_id: ProviderId::Claude,
            provider_name: "Claude".into(),
            window_id: "session".into(),
            window_label: "Session".into(),
            used_percent: 91.4,
            reset_at: Some(datetime!(2026-07-26 22:00:00 UTC)),
            level: NotificationLevel::Warning,
        };
        let spec = NotifySendDispatcher::<ScriptedNotify>::build_spec(&pending);
        assert_eq!(spec.program, PathBuf::from("notify-send"));
        assert_eq!(spec.args[0], "--app-name=Agent Bar");
        assert_eq!(spec.args[1], "--urgency=normal");
        assert_eq!(spec.args[2], "Claude usage warning");
        assert!(spec.args[3].contains("91% used"));
        assert!(spec.args[3].contains("Resets"));
    }
}
