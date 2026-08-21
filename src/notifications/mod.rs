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
    /// What fired the notification. Always the trigger, never the sentence.
    pub used_percent: f64,
    pub remaining_percent: f64,
    /// The unit the user chose in Settings (copy design §6.2).
    pub metric: DisplayMetric,
    pub reset_at: Option<time::OffsetDateTime>,
    /// Humanised countdown at evaluation time; `None` when the window carries
    /// no reset timestamp. Precomputed by the evaluator, which owns the clock,
    /// so `build_spec` stays a pure function of this struct.
    pub reset_in: Option<String>,
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
        // Copy design §5.5: the title names what is running out. The old
        // "{Name} usage warning" said the category, not the thing.
        let title = match pending.level {
            NotificationLevel::Warning => format!(
                "{} {} is running low",
                pending.provider_name, pending.window_label
            ),
            NotificationLevel::Critical => format!(
                "{} {} is almost out",
                pending.provider_name, pending.window_label
            ),
        };
        // §6.2: one unit across the product. The threshold that fired this is
        // always usedPercent; the sentence follows the user's chosen metric.
        let (value, unit) = match pending.metric {
            DisplayMetric::Used => (pending.used_percent, "used"),
            DisplayMetric::Remaining => (pending.remaining_percent, "left"),
        };
        let value = value.round() as i64;
        let body = match pending.reset_in.as_deref() {
            // "Resets in now." is not English; the popup avoids it the same way.
            Some("now") => format!("{value}% {unit}. Resets now."),
            Some(countdown) => format!("{value}% {unit}. Resets in {countdown}."),
            // §5.5: with no timestamp the clause is omitted, not filled in.
            None => format!("{value}% {unit}."),
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
    /// Supplied by the caller that already read the clock for this collect
    /// cycle, so the countdown agrees with the rest of the envelope.
    pub now: time::OffsetDateTime,
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
                    // Recovery below the warning threshold rearms.
                    state.remove_key(id, window.id());
                    continue;
                };
                let prev = state.entry_for(id, window.id()).map(|e| e.level);
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
                        remaining_percent: window.remaining_percent(),
                        metric: self.settings.display.metric,
                        reset_at: window.resets_at(),
                        reset_in: window
                            .resets_at()
                            .map(|ts| crate::support::countdown::reset_countdown(self.now, ts)),
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
                        notified_at: self.now,
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
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cli::{CacheMode, ProviderId};
    use crate::providers::process::{ProcessError, ProcessOutput, ProcessRunner};
    use crate::settings::schema::DisplayMetric;
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

    /// Sibling of `envelope_with_used` for tests that need a window with a
    /// real `resets_at`, since `envelope_with_used` hardcodes `None`.
    fn envelope_with_reset(used: f64, resets_at: time::OffsetDateTime) -> StatusEnvelope {
        let window =
            UsageWindow::try_new("session", "Session", used, 100.0 - used, Some(resets_at))
                .unwrap();
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

    fn store_in(dir: &tempfile::TempDir) -> NotificationStateStore {
        NotificationStateStore::new(
            NotificationPaths {
                state: dir.path().join("nstate.json"),
                lock: dir.path().join("n.lock"),
            },
            Arc::new(MaintenanceGate::open(dir.path().join("m.lock")).unwrap()),
        )
    }

    #[tokio::test]
    async fn sub_second_reset_jitter_does_not_renotify() {
        // The incident, end to end: three consecutive collections of the same
        // Claude window, each carrying a different microsecond reset. Before
        // this task that dispatched three times and persisted nothing.
        let dir = tempfile::tempdir().unwrap();
        let store = store_in(&dir);
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
            now: datetime!(2026-08-21 10:31:56 UTC),
        };
        for reset in [
            datetime!(2026-08-21 11:59:59.707742 UTC),
            datetime!(2026-08-21 11:59:59.854947 UTC),
            datetime!(2026-08-21 12:00:00.024238 UTC),
        ] {
            eval.evaluate(&envelope_with_reset(96.0, reset))
                .await
                .unwrap();
        }
        assert_eq!(dispatcher.runner.specs.lock().unwrap().len(), 1);
        let state = store.load().unwrap();
        assert_eq!(state.entries.len(), 1, "one row per window, not per reset");
    }

    #[tokio::test]
    async fn recovery_below_the_threshold_clears_the_row() {
        let dir = tempfile::tempdir().unwrap();
        let store = store_in(&dir);
        let runner = ScriptedNotify {
            specs: Mutex::new(Vec::new()),
            fail: false,
        };
        let dispatcher = NotifySendDispatcher::new(runner);
        let settings = SettingsDocument::defaults();
        let reset = datetime!(2026-08-21 22:00:00 UTC);
        let eval = NotificationEvaluator {
            store: &store,
            dispatcher: &dispatcher,
            settings: &settings,
            now: datetime!(2026-08-21 10:00:00 UTC),
        };
        eval.evaluate(&envelope_with_reset(96.0, reset))
            .await
            .unwrap();
        assert_eq!(store.load().unwrap().entries.len(), 1);
        eval.evaluate(&envelope_with_reset(10.0, reset))
            .await
            .unwrap();
        assert!(store.load().unwrap().entries.is_empty());
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
            now: datetime!(2026-07-26 18:42:00 UTC),
        };
        eval.evaluate(&envelope_with_used(90.0)).await.unwrap();
        eval.evaluate(&envelope_with_used(90.0)).await.unwrap(); // no second warning
        eval.evaluate(&envelope_with_used(96.0)).await.unwrap();
        let specs = dispatcher.runner.specs.lock().unwrap();
        assert_eq!(specs.len(), 2);
        assert!(specs[0].args.iter().any(|a| a.contains("is running low")));
        assert!(specs[1].args.iter().any(|a| a.contains("is almost out")));
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
            now: datetime!(2026-07-26 18:42:00 UTC),
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
            now: datetime!(2026-07-26 18:42:00 UTC),
        };
        eval.evaluate(&envelope_with_used(99.0)).await.unwrap();
        assert!(dispatcher.runner.specs.lock().unwrap().is_empty());
    }

    #[tokio::test]
    async fn evaluate_threads_its_own_clock_into_the_dispatched_countdown() {
        // Every other evaluate()-driven test above uses envelope_with_used(),
        // whose window carries resets_at: None, so the reset_in closure in
        // the pending push (`reset_countdown(self.now, ts)`) never runs.
        // This test gives the window a real reset timestamp and asserts the
        // exact dispatched body, so it fails if the arguments to
        // reset_countdown were ever swapped, or if self.now were ever
        // replaced by a fresh OffsetDateTime::now_utc() call.
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
        // Fixed instants, both already years behind the real wall clock this
        // suite runs under, so a stray OffsetDateTime::now_utc() would land
        // the reset far in the past and read "now", not "3h 1m" — it cannot
        // coincidentally reproduce the expected string.
        let now = datetime!(2026-07-26 18:42:00 UTC);
        let resets_at = datetime!(2026-07-26 21:43:00 UTC); // now + 3h 1m
        let eval = NotificationEvaluator {
            store: &store,
            dispatcher: &dispatcher,
            settings: &settings,
            now,
        };
        eval.evaluate(&envelope_with_reset(92.0, resets_at))
            .await
            .unwrap();
        let specs = dispatcher.runner.specs.lock().unwrap();
        assert_eq!(specs.len(), 1);
        // Default display metric is Remaining: 100 - 92 = 8% left.
        assert_eq!(specs[0].args[3], "8% left. Resets in 3h 1m.");
    }

    #[test]
    fn notify_send_argv_shape() {
        let pending = PendingNotification {
            provider_id: ProviderId::Claude,
            provider_name: "Claude".into(),
            window_id: "session".into(),
            window_label: "Session (5h)".into(),
            used_percent: 91.4,
            remaining_percent: 8.6,
            metric: DisplayMetric::Remaining,
            reset_at: Some(datetime!(2026-07-26 22:00:00 UTC)),
            reset_in: Some("3h 1m".to_owned()),
            level: NotificationLevel::Warning,
        };
        let spec = NotifySendDispatcher::<ScriptedNotify>::build_spec(&pending);
        assert_eq!(spec.program, PathBuf::from("notify-send"));
        assert_eq!(spec.args[0], "--app-name=Agent Bar");
        assert_eq!(spec.args[1], "--urgency=normal");
        assert_eq!(spec.args[2], "Claude Session (5h) is running low");
        assert_eq!(spec.args[3], "9% left. Resets in 3h 1m.");
    }

    #[test]
    fn notification_body_follows_the_display_metric() {
        // The trigger is always usedPercent, but the sentence is not: the
        // notification must not be the one surface speaking a different unit.
        let mut pending = PendingNotification {
            provider_id: ProviderId::Claude,
            provider_name: "Claude".into(),
            window_id: "session".into(),
            window_label: "Session (5h)".into(),
            used_percent: 96.0,
            remaining_percent: 4.0,
            metric: DisplayMetric::Remaining,
            reset_at: None,
            reset_in: None,
            level: NotificationLevel::Critical,
        };
        let spec = NotifySendDispatcher::<ScriptedNotify>::build_spec(&pending);
        assert_eq!(spec.args[1], "--urgency=critical");
        assert_eq!(spec.args[2], "Claude Session (5h) is almost out");
        // No timestamp: the reset clause is omitted entirely, not filled with
        // a placeholder.
        assert_eq!(spec.args[3], "4% left.");

        pending.metric = DisplayMetric::Used;
        let spec = NotifySendDispatcher::<ScriptedNotify>::build_spec(&pending);
        assert_eq!(spec.args[3], "96% used.");
    }

    #[test]
    fn elapsed_reset_never_reads_resets_in_now() {
        let pending = PendingNotification {
            provider_id: ProviderId::Claude,
            provider_name: "Claude".into(),
            window_id: "session".into(),
            window_label: "Session (5h)".into(),
            used_percent: 96.0,
            remaining_percent: 4.0,
            metric: DisplayMetric::Remaining,
            reset_at: Some(datetime!(2026-07-26 22:00:00 UTC)),
            reset_in: Some("now".to_owned()),
            level: NotificationLevel::Critical,
        };
        let spec = NotifySendDispatcher::<ScriptedNotify>::build_spec(&pending);
        assert_eq!(spec.args[3], "4% left. Resets now.");
    }
}
