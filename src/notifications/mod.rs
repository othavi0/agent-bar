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
use crate::status::schema::{DataSource, ProviderState, StatusEnvelope};
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
        let reminder =
            time::Duration::minutes(i64::from(self.settings.notifications.reminder_minutes));

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

            // Pruning runs before the window loop, so the rearms, upserts and
            // de-escalations below are never undone by it. The elapsed-reset
            // branch needs a live reading: a Ready provider can be replayed
            // from cache for up to its TTL while still reporting the
            // pre-reset timestamp, and treating that as proof the window
            // restarted would fire an alert about a window that already reset.
            let live_reading = provider.source() == Some(DataSource::Live);
            let live: Vec<&str> = provider.windows().iter().map(|w| w.id()).collect();
            state.prune_ready_provider(id, &live, self.now, live_reading);

            for window in provider.windows() {
                let used = window.used_percent();
                let observed = window.resets_at();
                let Some(level) = NotificationLevel::from_used_percent(used) else {
                    // Recovery below the warning threshold rearms.
                    state.remove_key(id, window.id());
                    continue;
                };
                let saved = state.entry_for(id, window.id()).cloned();
                let should_emit = match saved.as_ref() {
                    // Never spoken for this window.
                    None => true,
                    // The window advanced; a genuinely new quota period.
                    Some(prev) if !NotificationState::same_window(prev.reset_at, observed) => true,
                    // NOTIFY-002: severity only ever escalates.
                    Some(prev) if level > prev.level => true,
                    // Same severity: the reminder decides, not the poll.
                    Some(prev) if level == prev.level => self.now - prev.notified_at >= reminder,
                    // De-escalation inside the same window. NOTIFY-002 forbids
                    // speaking, but the tracked severity must follow the window
                    // down or the reminder can never match again, silencing a
                    // window that is still above its threshold.
                    Some(prev) => {
                        state.upsert(NotificationEntry {
                            provider_id: id.as_str().to_owned(),
                            window_id: window.id().to_owned(),
                            reset_at: observed,
                            level,
                            notified_at: prev.notified_at,
                        });
                        false
                    }
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
                        reset_at: observed,
                        reset_in: observed
                            .map(|ts| crate::support::countdown::reset_countdown(self.now, ts)),
                        level,
                    });
                }
            }
        }

        // Persist silent rearms and de-escalations first.
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
                    if let Err(err) = self.store.save(&state) {
                        // The incident this replaces ran for days behind a bare
                        // stderr line nobody reads.
                        log::warn!(
                            "notification state save failed for {}/{}: {err}",
                            item.provider_id.as_str(),
                            item.window_id
                        );
                        return Err(err.to_string());
                    }
                }
                Err(err) => {
                    // Leave the row unadvanced; stop later notifications.
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

    #[tokio::test]
    async fn a_new_window_renotifies() {
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
        eval.evaluate(&envelope_with_reset(
            96.0,
            datetime!(2026-08-21 11:59:59 UTC),
        ))
        .await
        .unwrap();
        eval.evaluate(&envelope_with_reset(
            96.0,
            datetime!(2026-08-28 11:59:59 UTC),
        ))
        .await
        .unwrap();
        assert_eq!(dispatcher.runner.specs.lock().unwrap().len(), 2);
        let state = store.load().unwrap();
        assert_eq!(
            state.entries.len(),
            1,
            "the advance replaces, never appends"
        );
    }

    #[tokio::test]
    async fn the_same_level_repeats_only_after_the_reminder_elapses() {
        let dir = tempfile::tempdir().unwrap();
        let store = store_in(&dir);
        let runner = ScriptedNotify {
            specs: Mutex::new(Vec::new()),
            fail: false,
        };
        let dispatcher = NotifySendDispatcher::new(runner);
        let settings = SettingsDocument::defaults(); // reminderMinutes == 120
        let reset = datetime!(2026-08-21 22:00:00 UTC);
        let first = datetime!(2026-08-21 10:00:00 UTC);
        let envelope = envelope_with_reset(96.0, reset);

        // A fresh evaluator per instant: `now` is a field, not an argument.
        NotificationEvaluator {
            store: &store,
            dispatcher: &dispatcher,
            settings: &settings,
            now: first,
        }
        .evaluate(&envelope)
        .await
        .unwrap();

        NotificationEvaluator {
            store: &store,
            dispatcher: &dispatcher,
            settings: &settings,
            now: first + time::Duration::minutes(119),
        }
        .evaluate(&envelope)
        .await
        .unwrap();
        assert_eq!(
            dispatcher.runner.specs.lock().unwrap().len(),
            1,
            "one minute short of the reminder must stay silent"
        );

        NotificationEvaluator {
            store: &store,
            dispatcher: &dispatcher,
            settings: &settings,
            now: first + time::Duration::minutes(120),
        }
        .evaluate(&envelope)
        .await
        .unwrap();
        assert_eq!(dispatcher.runner.specs.lock().unwrap().len(), 2);
    }

    #[tokio::test]
    async fn de_escalation_lowers_the_tracked_level_without_notifying() {
        // Critical -> Warning while still above 90 must not dispatch
        // (NOTIFY-002), but the stored level has to follow the window down.
        // If it stays Critical, the reminder arm never matches again and the
        // user stops hearing about a window that is still at 92 percent.
        let dir = tempfile::tempdir().unwrap();
        let store = store_in(&dir);
        let runner = ScriptedNotify {
            specs: Mutex::new(Vec::new()),
            fail: false,
        };
        let dispatcher = NotifySendDispatcher::new(runner);
        let settings = SettingsDocument::defaults();
        let reset = datetime!(2026-08-21 22:00:00 UTC);
        let first = datetime!(2026-08-21 10:00:00 UTC);

        NotificationEvaluator {
            store: &store,
            dispatcher: &dispatcher,
            settings: &settings,
            now: first,
        }
        .evaluate(&envelope_with_reset(96.0, reset))
        .await
        .unwrap();

        NotificationEvaluator {
            store: &store,
            dispatcher: &dispatcher,
            settings: &settings,
            now: first + time::Duration::minutes(5),
        }
        .evaluate(&envelope_with_reset(92.0, reset))
        .await
        .unwrap();
        assert_eq!(
            dispatcher.runner.specs.lock().unwrap().len(),
            1,
            "dropping a level never speaks"
        );
        let state = store.load().unwrap();
        assert_eq!(state.entries[0].level, NotificationLevel::Warning);

        // The reminder now fires at the level the window is actually at.
        NotificationEvaluator {
            store: &store,
            dispatcher: &dispatcher,
            settings: &settings,
            now: first + time::Duration::minutes(121),
        }
        .evaluate(&envelope_with_reset(92.0, reset))
        .await
        .unwrap();
        assert_eq!(dispatcher.runner.specs.lock().unwrap().len(), 2);
    }

    #[tokio::test]
    async fn evaluate_prunes_rows_for_windows_a_ready_provider_no_longer_reports() {
        let dir = tempfile::tempdir().unwrap();
        let store = store_in(&dir);
        let mut seeded = NotificationState::empty();
        seeded.upsert(NotificationEntry {
            provider_id: "claude".into(),
            window_id: "weekly-model:retired".into(),
            reset_at: Some(datetime!(2026-08-28 12:00:00 UTC)),
            level: NotificationLevel::Critical,
            notified_at: datetime!(2026-08-20 10:00:00 UTC),
        });
        store.save(&seeded).unwrap();

        let runner = ScriptedNotify {
            specs: Mutex::new(Vec::new()),
            fail: false,
        };
        let dispatcher = NotifySendDispatcher::new(runner);
        let settings = SettingsDocument::defaults();
        NotificationEvaluator {
            store: &store,
            dispatcher: &dispatcher,
            settings: &settings,
            now: datetime!(2026-08-21 10:00:00 UTC),
        }
        .evaluate(&envelope_with_used(10.0))
        .await
        .unwrap();

        let state = store.load().unwrap();
        assert!(
            state.entries.is_empty(),
            "a window the provider stopped reporting must not linger forever"
        );
    }

    #[tokio::test]
    async fn evaluate_keeps_rows_for_providers_absent_from_the_envelope() {
        // Pruning is Ready-only. A provider missing from this envelope has
        // confirmed nothing, so its dedupe must survive or it notifies again
        // the moment it recovers. The sibling case — present but not Ready —
        // is covered by evaluate_keeps_rows_for_a_stale_provider below.
        let dir = tempfile::tempdir().unwrap();
        let store = store_in(&dir);
        let mut seeded = NotificationState::empty();
        seeded.upsert(NotificationEntry {
            provider_id: "amp".into(),
            window_id: "daily".into(),
            reset_at: Some(datetime!(2026-08-12 00:00:00 UTC)),
            level: NotificationLevel::Critical,
            notified_at: datetime!(2026-08-11 23:00:00 UTC),
        });
        store.save(&seeded).unwrap();

        let runner = ScriptedNotify {
            specs: Mutex::new(Vec::new()),
            fail: false,
        };
        let dispatcher = NotifySendDispatcher::new(runner);
        let settings = SettingsDocument::defaults();
        NotificationEvaluator {
            store: &store,
            dispatcher: &dispatcher,
            settings: &settings,
            now: datetime!(2026-08-21 10:00:00 UTC),
        }
        .evaluate(&envelope_with_used(10.0))
        .await
        .unwrap();

        assert_eq!(store.load().unwrap().entries.len(), 1);
    }

    #[tokio::test]
    async fn evaluate_keeps_rows_for_a_stale_provider() {
        // Present in the envelope but not Ready: the provider confirmed
        // nothing this cycle, so neither pruning nor rearming may touch it
        // (NOTIFY-006). No pre-existing test covered this — the guard was
        // only a comment.
        use crate::status::schema::{ErrorCode, ProviderAction, ProviderError};

        let dir = tempfile::tempdir().unwrap();
        let store = store_in(&dir);
        let mut seeded = NotificationState::empty();
        seeded.upsert(NotificationEntry {
            provider_id: "claude".into(),
            window_id: "session".into(),
            reset_at: Some(datetime!(2026-08-12 00:00:00 UTC)),
            level: NotificationLevel::Critical,
            notified_at: datetime!(2026-08-11 23:00:00 UTC),
        });
        store.save(&seeded).unwrap();

        let window = UsageWindow::try_new("session", "Session", 5.0, 95.0, None).unwrap();
        let stale = ProviderStatus::stale(
            ProviderId::Claude,
            "Claude",
            None,
            None,
            vec![window],
            datetime!(2026-08-20 10:00:00 UTC),
            ProviderError::new(ErrorCode::NetworkError, "Network error.", true),
            ProviderAction::retry("Retry"),
        )
        .unwrap();
        let envelope = StatusEnvelope::try_new_for_package(
            datetime!(2026-08-21 10:00:00 UTC),
            StatusRequest {
                provider: None,
                cache: CacheMode::Use,
            },
            vec![stale],
        )
        .unwrap();

        let runner = ScriptedNotify {
            specs: Mutex::new(Vec::new()),
            fail: false,
        };
        let dispatcher = NotifySendDispatcher::new(runner);
        let settings = SettingsDocument::defaults();
        NotificationEvaluator {
            store: &store,
            dispatcher: &dispatcher,
            settings: &settings,
            now: datetime!(2026-08-21 10:00:00 UTC),
        }
        .evaluate(&envelope)
        .await
        .unwrap();

        assert_eq!(store.load().unwrap().entries.len(), 1);
        assert!(dispatcher.runner.specs.lock().unwrap().is_empty());
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
