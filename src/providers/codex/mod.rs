//! Codex provider. Estende `QuotaSource`. Duas fontes (app-server JSON-RPC +
//! fallback session-log) normalizadas para `CodexRateLimits`. Port fiel de
//! `src/providers/codex.ts`.

mod app_server;
mod normalize;
mod session_log;
mod types;

pub use app_server::{
    normalize_appserver_rate_limits, run_appserver_protocol, CodexAppServerAccount,
    CodexAppServerAccountReadResult, CodexAppServerCredits, CodexAppServerLimitBucket,
    CodexAppServerRateLimitsReadResult, CodexAppServerWindow,
};
pub use normalize::build_codex_quota;
pub use session_log::{extract_rate_limits, find_latest_session_file};
pub use types::{CodexCredits, CodexLimitBucket, CodexRateLimits, CodexWindowRaw};

use async_trait::async_trait;

use super::base::{base_get_quota, QuotaSource};
use super::error::{CodexError, ProviderError};
use super::types::ProviderQuota;
use super::{Ctx, Provider};
use app_server::fetch_via_appserver;

// ---- Provider (QuotaSource + Provider impls) ----

pub struct CodexProvider;

#[async_trait(?Send)]
impl QuotaSource for CodexProvider {
    type Raw = CodexRateLimits;

    fn id(&self) -> &'static str {
        "codex"
    }

    fn name(&self) -> &'static str {
        "Codex"
    }

    fn cache_key(&self) -> &'static str {
        "codex-quota"
    }

    async fn is_available(&self, ctx: &Ctx<'_>) -> bool {
        ctx.paths.codex_auth.exists()
    }

    async fn fetch_raw(&self, ctx: &Ctx<'_>) -> Result<CodexRateLimits, ProviderError> {
        if let Some(limits) = fetch_via_appserver(ctx.version).await {
            return Ok(limits);
        }
        log::warn!("Codex app-server unavailable, falling back to session log");
        let session =
            find_latest_session_file(&ctx.paths.codex_sessions, ctx.now_ms, ctx.local_offset)
                .ok_or(CodexError::NoSessionData)?;
        extract_rate_limits(&session).ok_or(ProviderError::Codex(CodexError::NoRateLimitData))
    }

    fn build_quota(
        &self,
        raw: CodexRateLimits,
        base: ProviderQuota,
        _ctx: &Ctx<'_>,
    ) -> ProviderQuota {
        build_codex_quota(&raw, base)
    }

    fn unavailable_error(&self) -> String {
        CodexError::NotLoggedIn.to_string()
    }

    fn to_user_facing_error(&self, error: &ProviderError) -> String {
        match error {
            ProviderError::Codex(e) => e.to_string(),
            _ => CodexError::Generic.to_string(),
        }
    }
}

#[async_trait(?Send)]
impl Provider for CodexProvider {
    fn id(&self) -> &'static str {
        "codex"
    }

    fn name(&self) -> &'static str {
        "Codex"
    }

    fn cache_key(&self) -> &'static str {
        "codex-quota"
    }

    async fn is_available(&self, ctx: &Ctx<'_>) -> bool {
        QuotaSource::is_available(self, ctx).await
    }

    async fn get_quota(&self, ctx: &Ctx<'_>) -> ProviderQuota {
        base_get_quota(self, ctx).await
    }
}

#[cfg(test)]
mod tests {
    use super::super::iso_from_ms;
    use super::super::types::{CodexQuotaExtra, ProviderExtra, ProviderQuota};
    use super::*;
    use indexmap::IndexMap;
    use time::UtcOffset;

    fn base() -> ProviderQuota {
        ProviderQuota {
            provider: "codex".into(),
            display_name: "Codex".into(),
            available: false,
            account: None,
            plan: None,
            plan_type: None,
            primary: None,
            secondary: None,
            models: None,
            extra: None,
            error: None,
            stale_reason: None,
        }
    }

    fn win(used: f64, mins: i64, resets: i64) -> CodexWindowRaw {
        CodexWindowRaw {
            used_percent: used,
            window_minutes: mins,
            resets_at: resets,
        }
    }

    fn codex_extra(q: &ProviderQuota) -> &CodexQuotaExtra {
        match q.extra.as_ref() {
            Some(ProviderExtra::Codex(c)) => c,
            _ => panic!("expected Codex extra"),
        }
    }

    // Future timestamp for tests that need a non-zero resets_at
    fn future_unix() -> i64 {
        // 2030-01-01T00:00:00Z in unix seconds
        1893456000
    }

    // -----------------------------------------------------------------------
    // primary/secondary basics
    // -----------------------------------------------------------------------

    #[test]
    fn primary_used_40_remaining_60_with_window() {
        let limits = CodexRateLimits {
            primary: Some(win(40.0, 300, 0)),
            ..Default::default()
        };
        let q = build_codex_quota(&limits, base());
        assert!(q.available);
        assert_eq!(q.primary.as_ref().unwrap().remaining, 60.0);
        assert_eq!(q.primary.as_ref().unwrap().window_minutes, Some(300));
        assert!(q.primary.as_ref().unwrap().resets_at.is_none()); // resets 0
    }

    #[test]
    fn primary_used_0_remaining_100() {
        let limits = CodexRateLimits {
            primary: Some(win(0.0, 300, future_unix())),
            ..Default::default()
        };
        let q = build_codex_quota(&limits, base());
        assert_eq!(q.primary.as_ref().unwrap().remaining, 100.0);
    }

    #[test]
    fn primary_used_100_remaining_0() {
        let limits = CodexRateLimits {
            primary: Some(win(100.0, 300, future_unix())),
            ..Default::default()
        };
        let q = build_codex_quota(&limits, base());
        assert_eq!(q.primary.as_ref().unwrap().remaining, 0.0);
    }

    #[test]
    fn secondary_used_20_remaining_80() {
        let limits = CodexRateLimits {
            primary: Some(win(40.0, 300, future_unix())),
            secondary: Some(win(20.0, 10080, future_unix())),
            ..Default::default()
        };
        let q = build_codex_quota(&limits, base());
        assert_eq!(q.secondary.as_ref().unwrap().remaining, 80.0);
        assert_eq!(q.secondary.as_ref().unwrap().window_minutes, Some(10080));
    }

    #[test]
    fn resets_at_iso_string_from_unix_timestamp() {
        let ts: i64 = 1711540800; // fixed known timestamp
        let limits = CodexRateLimits {
            primary: Some(win(0.0, 300, ts)),
            ..Default::default()
        };
        let q = build_codex_quota(&limits, base());
        // 1711540800 seconds → 1711540800000 ms → iso
        let expected = iso_from_ms((ts as u64) * 1000);
        assert_eq!(
            q.primary.as_ref().unwrap().resets_at.as_deref(),
            Some(expected.as_str())
        );
    }

    #[test]
    fn resets_at_null_when_zero() {
        let limits = CodexRateLimits {
            primary: Some(win(50.0, 300, 0)),
            ..Default::default()
        };
        let q = build_codex_quota(&limits, base());
        assert!(q.primary.as_ref().unwrap().resets_at.is_none());
    }

    // -----------------------------------------------------------------------
    // No usable data → error
    // -----------------------------------------------------------------------

    #[test]
    fn no_usable_data_errors() {
        let limits = CodexRateLimits::default();
        let q = build_codex_quota(&limits, base());
        assert_eq!(q.error.as_deref(), Some("No quota windows found"));
        assert!(!q.available);
    }

    #[test]
    fn plan_type_only_no_windows_errors() {
        let limits = CodexRateLimits {
            plan_type: Some("pro".into()),
            ..Default::default()
        };
        let q = build_codex_quota(&limits, base());
        assert_eq!(q.error.as_deref(), Some("No quota windows found"));
        assert!(!q.available);
    }

    // -----------------------------------------------------------------------
    // Plan type mapping
    // -----------------------------------------------------------------------

    #[test]
    fn plan_type_enterprise_maps() {
        let limits = CodexRateLimits {
            primary: Some(win(10.0, 300, 0)),
            plan_type: Some("enterprise".into()),
            ..Default::default()
        };
        let q = build_codex_quota(&limits, base());
        assert_eq!(q.plan.as_deref(), Some("Enterprise"));
        assert_eq!(q.plan_type.as_deref(), Some("enterprise"));
    }

    #[test]
    fn plan_type_null_omitted() {
        let limits = CodexRateLimits {
            primary: Some(win(10.0, 300, 0)),
            plan_type: None,
            ..Default::default()
        };
        let q = build_codex_quota(&limits, base());
        assert!(q.plan.is_none());
        assert!(q.plan_type.is_none());
    }

    #[test]
    fn plan_type_various_cases() {
        let cases = [
            ("free", "Free"),
            ("pro", "Pro"),
            ("team", "Business"),
            ("business", "Business"),
            ("enterprise", "Enterprise"),
            ("edu", "Edu"),
            ("education", "Edu"),
            ("go", "Go"),
            ("plus", "Plus"),
            ("apikey", "API Key"),
            ("api_key", "API Key"),
        ];
        for (input, expected) in cases {
            let limits = CodexRateLimits {
                primary: Some(win(10.0, 300, 0)),
                plan_type: Some(input.into()),
                ..Default::default()
            };
            let q = build_codex_quota(&limits, base());
            assert_eq!(q.plan.as_deref(), Some(expected), "plan_type '{input}'");
            assert_eq!(q.plan_type.as_deref(), Some(input));
        }
    }

    #[test]
    fn unknown_plan_type_titlecased() {
        let limits = CodexRateLimits {
            primary: Some(win(10.0, 300, 0)),
            plan_type: Some("custom_plan_xyz".into()),
            ..Default::default()
        };
        let q = build_codex_quota(&limits, base());
        assert_eq!(q.plan.as_deref(), Some("Custom Plan Xyz"));
    }

    // -----------------------------------------------------------------------
    // Credits / extraUsage
    // -----------------------------------------------------------------------

    #[test]
    fn credits_has_credits_true_sets_extra_usage() {
        let limits = CodexRateLimits {
            primary: Some(win(20.0, 300, 0)),
            credits: Some(CodexCredits {
                has_credits: true,
                unlimited: false,
                balance: "10.50".into(),
            }),
            ..Default::default()
        };
        let q = build_codex_quota(&limits, base());
        let eu = codex_extra(&q).extra_usage.as_ref().unwrap();
        assert!(eu.enabled);
        assert_eq!(eu.remaining, 11.0); // min(100, round(10.50)) = 11
        assert_eq!(eu.limit, 0.0);
        assert_eq!(eu.used, 0.0);
    }

    #[test]
    fn credits_capped_and_unlimited() {
        let limits = CodexRateLimits {
            primary: Some(win(10.0, 300, 0)),
            credits: Some(CodexCredits {
                has_credits: true,
                unlimited: false,
                balance: "250".into(),
            }),
            ..Default::default()
        };
        let q = build_codex_quota(&limits, base());
        let eu = codex_extra(&q).extra_usage.as_ref().unwrap();
        assert_eq!(eu.remaining, 100.0); // min(100, 250)
        assert_eq!(eu.limit, 0.0);

        // TS test uses has_credits: true here (brief had has_credits: false — mismatch vs codex.test.ts:569)
        let limits2 = CodexRateLimits {
            primary: Some(win(10.0, 300, 0)),
            credits: Some(CodexCredits {
                has_credits: true,
                unlimited: true,
                balance: "0".into(),
            }),
            ..Default::default()
        };
        let eu2 = codex_extra(&build_codex_quota(&limits2, base()))
            .extra_usage
            .clone()
            .unwrap();
        assert_eq!(eu2.remaining, 100.0);
        assert_eq!(eu2.limit, -1.0);
    }

    #[test]
    fn credits_balance_gt_zero_without_has_credits() {
        let limits = CodexRateLimits {
            primary: Some(win(20.0, 300, 0)),
            credits: Some(CodexCredits {
                has_credits: false,
                unlimited: false,
                balance: "5.00".into(),
            }),
            ..Default::default()
        };
        let q = build_codex_quota(&limits, base());
        let eu = codex_extra(&q).extra_usage.as_ref().unwrap();
        assert!(eu.enabled);
        assert_eq!(eu.remaining, 5.0);
    }

    #[test]
    fn credits_no_data_omits_extra_usage() {
        let limits = CodexRateLimits {
            primary: Some(win(20.0, 300, 0)),
            ..Default::default()
        };
        let q = build_codex_quota(&limits, base());
        // extra is present (modelsDetailed), but extra_usage is None
        let ce = codex_extra(&q);
        assert!(ce.extra_usage.is_none());
    }

    #[test]
    fn credits_false_and_balance_zero_omits_extra_usage() {
        let limits = CodexRateLimits {
            primary: Some(win(20.0, 300, 0)),
            credits: Some(CodexCredits {
                has_credits: false,
                unlimited: false,
                balance: "0".into(),
            }),
            ..Default::default()
        };
        let q = build_codex_quota(&limits, base());
        let ce = codex_extra(&q);
        assert!(ce.extra_usage.is_none());
    }

    // -----------------------------------------------------------------------
    // Window classification
    // -----------------------------------------------------------------------

    #[test]
    fn window_300_classifies_as_five_hour() {
        let limits = CodexRateLimits {
            primary: Some(win(10.0, 300, future_unix())),
            ..Default::default()
        };
        let q = build_codex_quota(&limits, base());
        let md = codex_extra(&q).models_detailed.as_ref().unwrap();
        let model = md.values().next().unwrap();
        assert!(model.five_hour.is_some());
        assert_eq!(model.five_hour.as_ref().unwrap().remaining, 90.0);
    }

    #[test]
    fn window_10080_classifies_as_seven_day() {
        let limits = CodexRateLimits {
            secondary: Some(win(25.0, 10080, future_unix())),
            ..Default::default()
        };
        let q = build_codex_quota(&limits, base());
        let md = codex_extra(&q).models_detailed.as_ref().unwrap();
        let model = md.values().next().unwrap();
        assert!(model.seven_day.is_some());
        assert_eq!(model.seven_day.as_ref().unwrap().remaining, 75.0);
    }

    #[test]
    fn tolerates_five_hour_within_90min() {
        // 210 = 300 - 90 (boundary), 390 = 300 + 90 (boundary)
        let mut buckets = IndexMap::new();
        buckets.insert(
            "b1".to_string(),
            CodexLimitBucket {
                limit_id: "b1".into(),
                limit_name: None,
                primary: Some(win(10.0, 210, future_unix())),
                secondary: None,
            },
        );
        buckets.insert(
            "b2".to_string(),
            CodexLimitBucket {
                limit_id: "b2".into(),
                limit_name: None,
                primary: Some(win(20.0, 390, future_unix())),
                secondary: None,
            },
        );
        let limits = CodexRateLimits {
            buckets: Some(buckets),
            ..Default::default()
        };
        let q = build_codex_quota(&limits, base());
        let md = codex_extra(&q).models_detailed.as_ref().unwrap();
        for windows in md.values() {
            assert!(
                windows.five_hour.is_some(),
                "expected fiveHour for boundary minutes"
            );
        }
    }

    #[test]
    fn tolerates_seven_day_within_1440min() {
        // 8640 = 10080 - 1440, 11520 = 10080 + 1440
        let mut buckets = IndexMap::new();
        buckets.insert(
            "b1".to_string(),
            CodexLimitBucket {
                limit_id: "b1".into(),
                limit_name: None,
                primary: None,
                secondary: Some(win(30.0, 8640, future_unix())),
            },
        );
        buckets.insert(
            "b2".to_string(),
            CodexLimitBucket {
                limit_id: "b2".into(),
                limit_name: None,
                primary: None,
                secondary: Some(win(40.0, 11520, future_unix())),
            },
        );
        let limits = CodexRateLimits {
            buckets: Some(buckets),
            ..Default::default()
        };
        let q = build_codex_quota(&limits, base());
        let md = codex_extra(&q).models_detailed.as_ref().unwrap();
        for windows in md.values() {
            assert!(
                windows.seven_day.is_some(),
                "expected sevenDay for boundary minutes"
            );
        }
    }

    #[test]
    fn unrecognized_window_stays_other_no_fallback() {
        // 60 min = "other" via classify_window; SEM fallback, fica só
        // em `other` (nunca forçado em fiveHour/sevenDay) — é o fix do
        // bug de duplicação do Codex (auditoria 2026-07-21).
        let mut buckets = IndexMap::new();
        buckets.insert(
            "b1".to_string(),
            CodexLimitBucket {
                limit_id: "b1".into(),
                limit_name: None,
                primary: Some(win(10.0, 60, future_unix())),
                secondary: Some(win(20.0, 60, future_unix())),
            },
        );
        let limits = CodexRateLimits {
            buckets: Some(buckets),
            ..Default::default()
        };
        let q = build_codex_quota(&limits, base());
        let md = codex_extra(&q).models_detailed.as_ref().unwrap();
        let model = md.values().next().unwrap();
        assert!(model.five_hour.is_none(), "60min não deve ir pra fiveHour");
        assert!(model.seven_day.is_none(), "60min não deve ir pra sevenDay");
        let other = model.other.as_ref().expect("other deve ter as 2 janelas");
        assert_eq!(other.len(), 2, "primary+secondary de 60min, ambas em other");
        for w in other {
            assert_eq!(
                w.window_kind,
                Some(crate::formatters::shared::WindowKind::Other)
            );
        }
    }

    #[test]
    fn both_windows_weekly_primary_wins_seven_day_slot() {
        // Payload real que gerava o bug: primary e secondary do MESMO
        // bucket com window_minutes=10080 (7 dias) — o caso citado na
        // auditoria 2026-07-21. O loop de build_model_windows itera
        // [primary, secondary] nessa ordem, então primary preenche o
        // slot seven_day primeiro; secondary chega com o slot já
        // ocupado e cai em `other` (sem duplicação, sem fallback).
        let mut buckets = IndexMap::new();
        buckets.insert(
            "b1".to_string(),
            CodexLimitBucket {
                limit_id: "b1".into(),
                limit_name: None,
                primary: Some(win(10.0, 10080, future_unix())),
                secondary: Some(win(20.0, 10080, future_unix())),
            },
        );
        let limits = CodexRateLimits {
            buckets: Some(buckets),
            ..Default::default()
        };
        let q = build_codex_quota(&limits, base());
        let md = codex_extra(&q).models_detailed.as_ref().unwrap();
        let model = md.values().next().unwrap();

        assert!(
            model.five_hour.is_none(),
            "nada deve ser forçado no slot fiveHour"
        );

        let seven_day = model
            .seven_day
            .as_ref()
            .expect("seven_day deve conter a primary (primeira a preencher o slot)");
        assert_eq!(
            seven_day.remaining, 90.0,
            "seven_day deve ser a primary (used=10.0)"
        );
        assert_eq!(
            seven_day.window_kind,
            Some(crate::formatters::shared::WindowKind::SevenDay)
        );

        let other = model
            .other
            .as_ref()
            .expect("other deve conter a secondary, sem slot livre");
        assert_eq!(other.len(), 1, "só a secondary sobra pra other");
        assert_eq!(
            other[0].remaining, 80.0,
            "other deve ser a secondary (used=20.0)"
        );
        assert_eq!(
            other[0].window_kind,
            Some(crate::formatters::shared::WindowKind::SevenDay)
        );
    }

    // -----------------------------------------------------------------------
    // Multiple buckets
    // -----------------------------------------------------------------------

    #[test]
    fn multiple_buckets_create_models_detailed_entries() {
        let mut buckets = IndexMap::new();
        buckets.insert(
            "codex-mini".to_string(),
            CodexLimitBucket {
                limit_id: "codex-mini".into(),
                limit_name: Some("Codex Mini".into()),
                primary: Some(win(30.0, 300, future_unix())),
                secondary: Some(win(15.0, 10080, future_unix())),
            },
        );
        buckets.insert(
            "codex-standard".to_string(),
            CodexLimitBucket {
                limit_id: "codex-standard".into(),
                limit_name: Some("Codex Standard".into()),
                primary: Some(win(60.0, 300, future_unix())),
                secondary: Some(win(45.0, 10080, future_unix())),
            },
        );
        let limits = CodexRateLimits {
            buckets: Some(buckets),
            ..Default::default()
        };
        let q = build_codex_quota(&limits, base());
        assert!(q.available);
        let md = codex_extra(&q).models_detailed.as_ref().unwrap();
        assert_eq!(md.len(), 2);
        assert!(md.contains_key("Codex Mini"));
        assert!(md.contains_key("Codex Standard"));
        assert_eq!(md["Codex Mini"].five_hour.as_ref().unwrap().remaining, 70.0);
        assert_eq!(md["Codex Mini"].seven_day.as_ref().unwrap().remaining, 85.0);
        assert_eq!(
            md["Codex Standard"].five_hour.as_ref().unwrap().remaining,
            40.0
        );
        assert_eq!(
            md["Codex Standard"].seven_day.as_ref().unwrap().remaining,
            55.0
        );
    }

    #[test]
    fn limit_id_used_when_limit_name_null() {
        let mut buckets = IndexMap::new();
        buckets.insert(
            "my_custom_limit".to_string(),
            CodexLimitBucket {
                limit_id: "my_custom_limit".into(),
                limit_name: None,
                primary: Some(win(10.0, 300, future_unix())),
                secondary: None,
            },
        );
        let limits = CodexRateLimits {
            buckets: Some(buckets),
            ..Default::default()
        };
        let q = build_codex_quota(&limits, base());
        let md = codex_extra(&q).models_detailed.as_ref().unwrap();
        assert_eq!(md.len(), 1);
        assert!(md.contains_key("My Custom Limit"));
    }

    #[test]
    fn flatten_picks_five_hour_first() {
        let mut buckets = IndexMap::new();
        buckets.insert(
            "codex".to_string(),
            CodexLimitBucket {
                limit_id: "codex".into(),
                limit_name: Some("Codex".into()),
                primary: Some(win(25.0, 300, future_unix())),
                secondary: Some(win(50.0, 10080, future_unix())),
            },
        );
        let limits = CodexRateLimits {
            buckets: Some(buckets),
            ..Default::default()
        };
        let q = build_codex_quota(&limits, base());
        assert!(q.models.is_some());
        let models = q.models.as_ref().unwrap();
        assert_eq!(models["Codex"].remaining, 75.0); // fiveHour preferred
    }

    #[test]
    fn dedup_bucket_names_with_suffix() {
        let mut buckets = IndexMap::new();
        buckets.insert(
            "a".to_string(),
            CodexLimitBucket {
                limit_id: "a".into(),
                limit_name: Some("gpt".into()),
                primary: Some(win(20.0, 300, 0)),
                secondary: None,
            },
        );
        buckets.insert(
            "b".to_string(),
            CodexLimitBucket {
                limit_id: "b".into(),
                limit_name: Some("gpt".into()),
                primary: Some(win(30.0, 300, 0)),
                secondary: None,
            },
        );
        let limits = CodexRateLimits {
            buckets: Some(buckets),
            ..Default::default()
        };
        let q = build_codex_quota(&limits, base());
        let md = codex_extra(&q).models_detailed.as_ref().unwrap();
        assert!(md.contains_key("Gpt"));
        assert!(md.contains_key("Gpt (2)"));
    }

    #[test]
    fn dedup_with_codex_label_name_collision() {
        let mut buckets = IndexMap::new();
        buckets.insert(
            "a".to_string(),
            CodexLimitBucket {
                limit_id: "a".into(),
                limit_name: Some("Codex".into()),
                primary: Some(win(10.0, 300, 0)),
                secondary: None,
            },
        );
        buckets.insert(
            "b".to_string(),
            CodexLimitBucket {
                limit_id: "b".into(),
                limit_name: Some("Codex".into()),
                primary: Some(win(20.0, 300, 0)),
                secondary: None,
            },
        );
        let limits = CodexRateLimits {
            buckets: Some(buckets),
            ..Default::default()
        };
        let q = build_codex_quota(&limits, base());
        let md = codex_extra(&q).models_detailed.as_ref().unwrap();
        assert_eq!(md.len(), 2);
        assert!(md.contains_key("Codex"));
        assert!(md.contains_key("Codex (2)"));
    }

    // -----------------------------------------------------------------------
    // Legacy fallback (no buckets, only primary/secondary)
    // -----------------------------------------------------------------------

    #[test]
    fn legacy_single_codex_entry() {
        let limits = CodexRateLimits {
            primary: Some(win(35.0, 300, future_unix())),
            secondary: Some(win(55.0, 10080, future_unix())),
            ..Default::default()
        };
        let q = build_codex_quota(&limits, base());
        let md = codex_extra(&q).models_detailed.as_ref().unwrap();
        assert_eq!(md.len(), 1);
        assert!(md.contains_key("Codex"));
        assert_eq!(md["Codex"].five_hour.as_ref().unwrap().remaining, 65.0);
        assert_eq!(md["Codex"].seven_day.as_ref().unwrap().remaining, 45.0);
    }

    // -----------------------------------------------------------------------
    // Primary/secondary selection
    // -----------------------------------------------------------------------

    #[test]
    fn explicit_primary_secondary_preferred_over_buckets() {
        let mut buckets = IndexMap::new();
        buckets.insert(
            "codex".to_string(),
            CodexLimitBucket {
                limit_id: "codex".into(),
                limit_name: None,
                primary: Some(win(99.0, 300, future_unix())),
                secondary: Some(win(99.0, 10080, future_unix())),
            },
        );
        let limits = CodexRateLimits {
            primary: Some(win(30.0, 300, future_unix())),
            secondary: Some(win(50.0, 10080, future_unix())),
            buckets: Some(buckets),
            ..Default::default()
        };
        let q = build_codex_quota(&limits, base());
        assert_eq!(q.primary.as_ref().unwrap().remaining, 70.0);
        assert_eq!(q.secondary.as_ref().unwrap().remaining, 50.0);
    }

    #[test]
    fn falls_back_to_bucket_five_hour_seven_day() {
        let mut buckets = IndexMap::new();
        buckets.insert(
            "codex".to_string(),
            CodexLimitBucket {
                limit_id: "codex".into(),
                limit_name: None,
                primary: Some(win(40.0, 300, future_unix())),
                secondary: Some(win(60.0, 10080, future_unix())),
            },
        );
        let limits = CodexRateLimits {
            buckets: Some(buckets),
            ..Default::default()
        };
        let q = build_codex_quota(&limits, base());
        // pickPrimary → first model's fiveHour
        assert_eq!(q.primary.as_ref().unwrap().remaining, 60.0);
        // pickSecondary → first model's sevenDay
        assert_eq!(q.secondary.as_ref().unwrap().remaining, 40.0);
    }

    // -----------------------------------------------------------------------
    // Edge: empty buckets / skipped
    // -----------------------------------------------------------------------

    #[test]
    fn empty_plan_type_is_omitted() {
        let limits = CodexRateLimits {
            primary: Some(win(10.0, 300, 0)),
            plan_type: Some("".into()),
            ..Default::default()
        };
        let q = build_codex_quota(&limits, base());
        assert!(
            q.plan_type.is_none(),
            "plan_type vazio deve ser omitido (casa o TS)"
        );
        assert!(q.plan.is_none());
    }

    #[test]
    fn skips_bucket_with_no_primary_or_secondary() {
        let mut buckets = IndexMap::new();
        buckets.insert(
            "empty".to_string(),
            CodexLimitBucket {
                limit_id: "empty".into(),
                limit_name: None,
                primary: None,
                secondary: None,
            },
        );
        buckets.insert(
            "valid".to_string(),
            CodexLimitBucket {
                limit_id: "valid".into(),
                limit_name: Some("Valid Bucket".into()),
                primary: Some(win(20.0, 300, future_unix())),
                secondary: None,
            },
        );
        let limits = CodexRateLimits {
            primary: Some(win(10.0, 300, future_unix())),
            buckets: Some(buckets),
            ..Default::default()
        };
        let q = build_codex_quota(&limits, base());
        let md = codex_extra(&q).models_detailed.as_ref().unwrap();
        assert!(md.contains_key("Valid Bucket"));
        assert!(!md.contains_key("empty") && !md.contains_key("Empty"));
    }

    // -----------------------------------------------------------------------
    // normalize_appserver_rate_limits — Task 2
    // -----------------------------------------------------------------------

    fn app_win(
        used_percent: f64,
        window_duration_mins: Option<i64>,
        resets_at: Option<i64>,
    ) -> CodexAppServerWindow {
        CodexAppServerWindow {
            used_percent,
            window_duration_mins,
            resets_at,
        }
    }

    #[test]
    fn normalize_root_rate_limits_simple() {
        // rateLimits simples (usedPercent 30, windowDurationMins 300)
        // → CodexRateLimits com primary{used_percent 30, window 300}
        let raw = CodexAppServerRateLimitsReadResult {
            rate_limits: Some(CodexAppServerLimitBucket {
                limit_id: None,
                limit_name: None,
                primary: Some(app_win(30.0, Some(300), None)),
                secondary: None,
                plan_type: None,
            }),
            rate_limits_by_limit_id: None,
            credits: None,
            plan_type: None,
        };
        let result = normalize_appserver_rate_limits(&raw, None).expect("deve retornar Some");
        let primary = result.primary.expect("primary deve existir");
        assert_eq!(primary.used_percent, 30.0);
        assert_eq!(primary.window_minutes, 300);
        assert_eq!(primary.resets_at, 0);
    }

    #[test]
    fn normalize_rate_limits_by_limit_id_single_bucket() {
        // rateLimitsByLimitId com 1 bucket
        let mut by_id = IndexMap::new();
        by_id.insert(
            "codex-mini".to_string(),
            CodexAppServerLimitBucket {
                limit_id: Some("codex-mini".into()),
                limit_name: Some("Codex Mini".into()),
                primary: Some(app_win(50.0, Some(300), Some(1893456000))),
                secondary: Some(app_win(20.0, Some(10080), Some(1893456000))),
                plan_type: None,
            },
        );
        let raw = CodexAppServerRateLimitsReadResult {
            rate_limits: None,
            rate_limits_by_limit_id: Some(by_id),
            credits: None,
            plan_type: None,
        };
        let result = normalize_appserver_rate_limits(&raw, None).expect("deve retornar Some");
        let buckets = result.buckets.expect("buckets deve existir");
        assert_eq!(buckets.len(), 1);
        let bucket = &buckets["codex-mini"];
        assert_eq!(bucket.limit_id, "codex-mini");
        assert_eq!(bucket.limit_name.as_deref(), Some("Codex Mini"));
        let p = bucket.primary.as_ref().expect("primary do bucket");
        assert_eq!(p.used_percent, 50.0);
        assert_eq!(p.window_minutes, 300);
    }

    #[test]
    fn normalize_credits_camelcase_to_snake() {
        // credits camelCase → snake_case; balance None → "0"
        let raw = CodexAppServerRateLimitsReadResult {
            rate_limits: Some(CodexAppServerLimitBucket {
                limit_id: None,
                limit_name: None,
                primary: Some(app_win(10.0, Some(300), None)),
                secondary: None,
                plan_type: None,
            }),
            rate_limits_by_limit_id: None,
            credits: Some(CodexAppServerCredits {
                has_credits: true,
                unlimited: false,
                balance: Some("42.5".into()),
            }),
            plan_type: None,
        };
        let result = normalize_appserver_rate_limits(&raw, None).expect("deve retornar Some");
        let credits = result.credits.expect("credits deve existir");
        assert!(credits.has_credits);
        assert!(!credits.unlimited);
        assert_eq!(credits.balance, "42.5");
    }

    #[test]
    fn normalize_credits_balance_none_defaults_to_zero_string() {
        let raw = CodexAppServerRateLimitsReadResult {
            rate_limits: Some(CodexAppServerLimitBucket {
                limit_id: None,
                limit_name: None,
                primary: Some(app_win(0.0, Some(300), None)),
                secondary: None,
                plan_type: None,
            }),
            rate_limits_by_limit_id: None,
            credits: Some(CodexAppServerCredits {
                has_credits: false,
                unlimited: false,
                balance: None,
            }),
            plan_type: None,
        };
        let result = normalize_appserver_rate_limits(&raw, None).expect("deve retornar Some");
        let credits = result.credits.expect("credits deve existir");
        assert_eq!(credits.balance, "0");
    }

    #[test]
    fn normalize_plan_type_priority_account_over_raw_over_root() {
        // account_plan_type > raw.planType > root.planType
        let raw = CodexAppServerRateLimitsReadResult {
            rate_limits: Some(CodexAppServerLimitBucket {
                limit_id: None,
                limit_name: None,
                primary: Some(app_win(10.0, Some(300), None)),
                secondary: None,
                plan_type: Some("root-plan".into()),
            }),
            rate_limits_by_limit_id: None,
            credits: None,
            plan_type: Some("raw-plan".into()),
        };

        // account_plan_type vence
        let r1 = normalize_appserver_rate_limits(&raw, Some("account-plan")).expect("Some");
        assert_eq!(r1.plan_type.as_deref(), Some("account-plan"));

        // sem account → raw.planType
        let r2 = normalize_appserver_rate_limits(&raw, None).expect("Some");
        assert_eq!(r2.plan_type.as_deref(), Some("raw-plan"));

        // sem account, sem raw.planType → root.planType
        let raw_no_raw_plan = CodexAppServerRateLimitsReadResult {
            rate_limits: Some(CodexAppServerLimitBucket {
                limit_id: None,
                limit_name: None,
                primary: Some(app_win(10.0, Some(300), None)),
                secondary: None,
                plan_type: Some("root-plan".into()),
            }),
            rate_limits_by_limit_id: None,
            credits: None,
            plan_type: None,
        };
        let r3 = normalize_appserver_rate_limits(&raw_no_raw_plan, None).expect("Some");
        assert_eq!(r3.plan_type.as_deref(), Some("root-plan"));
    }

    #[test]
    fn normalize_returns_none_when_everything_empty() {
        let raw = CodexAppServerRateLimitsReadResult::default();
        assert!(normalize_appserver_rate_limits(&raw, None).is_none());
    }

    #[test]
    fn normalize_root_inserted_only_if_key_absent() {
        // root com limitId "codex" e by_id já tem "codex" → root NÃO sobrescreve
        let mut by_id = IndexMap::new();
        by_id.insert(
            "codex".to_string(),
            CodexAppServerLimitBucket {
                limit_id: Some("codex".into()),
                limit_name: None,
                primary: Some(app_win(70.0, Some(300), None)),
                secondary: None,
                plan_type: None,
            },
        );
        let raw = CodexAppServerRateLimitsReadResult {
            rate_limits: Some(CodexAppServerLimitBucket {
                limit_id: Some("codex".into()),
                limit_name: None,
                primary: Some(app_win(10.0, Some(300), None)),
                secondary: None,
                plan_type: None,
            }),
            rate_limits_by_limit_id: Some(by_id),
            credits: None,
            plan_type: None,
        };
        let result = normalize_appserver_rate_limits(&raw, None).expect("Some");
        let buckets = result.buckets.expect("buckets");
        assert_eq!(buckets.len(), 1);
        // O bucket do by_id (70%) vence, root (10%) não sobrescreve
        assert_eq!(
            buckets["codex"].primary.as_ref().unwrap().used_percent,
            70.0
        );
    }

    #[test]
    fn normalize_fallback_id_codex_when_root_limit_id_absent() {
        // root sem limitId → fallback "codex"
        let raw = CodexAppServerRateLimitsReadResult {
            rate_limits: Some(CodexAppServerLimitBucket {
                limit_id: None,
                limit_name: None,
                primary: Some(app_win(55.0, Some(300), None)),
                secondary: None,
                plan_type: None,
            }),
            rate_limits_by_limit_id: None,
            credits: None,
            plan_type: None,
        };
        let result = normalize_appserver_rate_limits(&raw, None).expect("Some");
        let buckets = result.buckets.expect("buckets");
        assert!(buckets.contains_key("codex"));
    }

    // -----------------------------------------------------------------------
    // session-log fallback — Task 3
    // -----------------------------------------------------------------------

    fn make_session_dir(
        base: &std::path::Path,
        now_ms: u64,
        offset: UtcOffset,
    ) -> std::path::PathBuf {
        use time::OffsetDateTime;
        let dt = OffsetDateTime::from_unix_timestamp_nanos((now_ms as i128) * 1_000_000)
            .unwrap()
            .to_offset(offset);
        base.join(format!("{:04}", dt.year()))
            .join(format!("{:02}", dt.month() as u8))
            .join(format!("{:02}", dt.day()))
    }

    const TOKEN_COUNT_LINE: &str = r#"{"payload":{"type":"token_count","rate_limits":{"primary":{"used_percent":40,"window_minutes":300,"resets_at":0}}}}"#;

    #[test]
    fn find_and_extract_session_log_basic() {
        let tmp = tempfile::tempdir().unwrap();
        let now_ms: u64 = 1_750_000_000_000; // um timestamp fixo qualquer
        let offset = UtcOffset::UTC;
        let dir = make_session_dir(tmp.path(), now_ms, offset);
        std::fs::create_dir_all(&dir).unwrap();
        let file = dir.join("session.jsonl");
        std::fs::write(&file, TOKEN_COUNT_LINE).unwrap();

        let found = find_latest_session_file(tmp.path(), now_ms, offset);
        assert_eq!(found.as_deref(), Some(file.as_path()));

        let rl = extract_rate_limits(found.as_ref().unwrap()).unwrap();
        assert_eq!(rl.primary.as_ref().unwrap().used_percent, 40.0);
        assert_eq!(rl.primary.as_ref().unwrap().window_minutes, 300);
    }

    #[test]
    fn find_session_multiple_files_picks_newest_mtime() {
        let tmp = tempfile::tempdir().unwrap();
        let now_ms: u64 = 1_750_000_000_000;
        let offset = UtcOffset::UTC;
        let dir = make_session_dir(tmp.path(), now_ms, offset);
        std::fs::create_dir_all(&dir).unwrap();

        let old_file = dir.join("old.jsonl");
        let new_file = dir.join("new.jsonl");
        std::fs::write(&old_file, TOKEN_COUNT_LINE).unwrap();
        std::fs::write(&new_file, TOKEN_COUNT_LINE).unwrap();

        // set old_file mtime to past
        let past = std::time::SystemTime::UNIX_EPOCH + std::time::Duration::from_secs(1_000_000);
        filetime::set_file_mtime(&old_file, filetime::FileTime::from_system_time(past)).unwrap();
        let future = std::time::SystemTime::UNIX_EPOCH + std::time::Duration::from_secs(2_000_000);
        filetime::set_file_mtime(&new_file, filetime::FileTime::from_system_time(future)).unwrap();

        let found = find_latest_session_file(tmp.path(), now_ms, offset).unwrap();
        assert_eq!(found, new_file);
    }

    #[test]
    fn extract_rate_limits_scan_reverse_skips_non_token_count() {
        let tmp = tempfile::tempdir().unwrap();
        let now_ms: u64 = 1_750_000_000_000;
        let offset = UtcOffset::UTC;
        let dir = make_session_dir(tmp.path(), now_ms, offset);
        std::fs::create_dir_all(&dir).unwrap();

        let content = [
            r#"{"payload":{"type":"other_event"}}"#,
            TOKEN_COUNT_LINE,
            r#"{"payload":{"type":"something_else"}}"#,
        ]
        .join("\n");
        let file = dir.join("session.jsonl");
        std::fs::write(&file, &content).unwrap();

        let rl = extract_rate_limits(&file).unwrap();
        assert_eq!(rl.primary.as_ref().unwrap().used_percent, 40.0);
    }

    #[test]
    fn find_session_falls_back_to_yesterday() {
        let tmp = tempfile::tempdir().unwrap();
        // Usar now_ms tal que hoje não tem dir, mas ontem tem
        let now_ms: u64 = 1_750_000_000_000;
        let offset = UtcOffset::UTC;

        // Cria dir de ontem
        use time::{Duration as TimeDuration, OffsetDateTime};
        let now = OffsetDateTime::from_unix_timestamp_nanos((now_ms as i128) * 1_000_000)
            .unwrap()
            .to_offset(offset);
        let yesterday = now - TimeDuration::days(1);
        let yesterday_dir = tmp
            .path()
            .join(format!("{:04}", yesterday.year()))
            .join(format!("{:02}", yesterday.month() as u8))
            .join(format!("{:02}", yesterday.day()));
        std::fs::create_dir_all(&yesterday_dir).unwrap();
        let file = yesterday_dir.join("session.jsonl");
        std::fs::write(&file, TOKEN_COUNT_LINE).unwrap();

        // Não cria dir de hoje → deve cair no fallback
        let found = find_latest_session_file(tmp.path(), now_ms, offset);
        assert_eq!(found.as_deref(), Some(file.as_path()));
    }

    #[test]
    fn find_session_returns_none_when_no_dirs() {
        let tmp = tempfile::tempdir().unwrap();
        let now_ms: u64 = 1_750_000_000_000;
        let found = find_latest_session_file(tmp.path(), now_ms, UtcOffset::UTC);
        assert!(found.is_none());
    }

    // -----------------------------------------------------------------------
    // run_appserver_protocol — Task 4
    // -----------------------------------------------------------------------

    #[tokio::test]
    async fn appserver_happy_path() {
        use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
        let (client, server) = tokio::io::duplex(8192);
        let (cr, cw) = tokio::io::split(client);
        tokio::spawn(async move {
            let (sr, mut sw) = tokio::io::split(server);
            let mut lines = BufReader::new(sr).lines();
            while let Ok(Some(line)) = lines.next_line().await {
                let v: serde_json::Value = match serde_json::from_str(&line) {
                    Ok(v) => v,
                    Err(_) => continue,
                };
                match v.get("id").and_then(|i| i.as_i64()) {
                    Some(0) => {
                        let _ = sw
                            .write_all(b"{\"id\":0,\"result\":{\"capabilities\":{}}}\n")
                            .await;
                    }
                    Some(1) => {
                        let _ = sw
                            .write_all(
                                b"{\"id\":1,\"result\":{\"account\":{\"planType\":\"pro\"}}}\n",
                            )
                            .await;
                    }
                    Some(2) => {
                        let _ = sw
                            .write_all(b"{\"id\":2,\"result\":{\"rateLimits\":{\"limitId\":\"codex-default\",\"primary\":{\"usedPercent\":30,\"windowDurationMins\":300,\"resetsAt\":1700000000}}}}\n")
                            .await;
                    }
                    _ => {}
                }
            }
        });
        let out = run_appserver_protocol(cr, cw, "test", std::time::Duration::from_secs(4)).await;
        let limits = out.expect("should resolve");
        assert_eq!(limits.primary.as_ref().unwrap().used_percent, 30.0);
        assert_eq!(limits.plan_type.as_deref(), Some("pro"));
    }

    #[tokio::test]
    async fn appserver_timeout_returns_none() {
        let (client, _server) = tokio::io::duplex(8192);
        let (cr, cw) = tokio::io::split(client);
        // _server nunca responde
        let out =
            run_appserver_protocol(cr, cw, "test", std::time::Duration::from_millis(100)).await;
        assert!(out.is_none());
    }

    #[tokio::test]
    async fn appserver_eof_returns_none() {
        // Cria um duplex e dropa o server imediatamente → client lê EOF
        let (client, server) = tokio::io::duplex(8192);
        let (cr, cw) = tokio::io::split(client);
        drop(server);
        let out = run_appserver_protocol(cr, cw, "test", std::time::Duration::from_secs(4)).await;
        assert!(out.is_none());
    }

    #[tokio::test]
    async fn appserver_rate_limits_error_returns_none_without_waiting_hard_timeout() {
        // Reproduz o caso real: token de auth expirado/revogado → app-server
        // responde `account/rateLimits/read` (id=2) com um erro JSON-RPC em vez
        // de `result`. Antes do fix, isso ficava sem tratamento e o loop
        // esperava o hard timeout inteiro (aqui, 4s) antes de retornar None.
        // O fix deve retornar None assim que o erro chega, bem antes do timeout.
        use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
        let (client, server) = tokio::io::duplex(8192);
        let (cr, cw) = tokio::io::split(client);
        tokio::spawn(async move {
            let (sr, mut sw) = tokio::io::split(server);
            let mut lines = BufReader::new(sr).lines();
            while let Ok(Some(line)) = lines.next_line().await {
                let v: serde_json::Value = match serde_json::from_str(&line) {
                    Ok(v) => v,
                    Err(_) => continue,
                };
                match v.get("id").and_then(|i| i.as_i64()) {
                    Some(0) => {
                        let _ = sw
                            .write_all(b"{\"id\":0,\"result\":{\"capabilities\":{}}}\n")
                            .await;
                    }
                    Some(1) => {
                        let _ = sw
                            .write_all(
                                b"{\"id\":1,\"result\":{\"account\":{\"planType\":\"plus\"}}}\n",
                            )
                            .await;
                    }
                    Some(2) => {
                        let _ = sw
                            .write_all(
                                b"{\"id\":2,\"error\":{\"code\":-32603,\"message\":\"failed to fetch codex rate limits: GET https://chatgpt.com/backend-api/wham/usage failed: 401 Unauthorized; token_expired\"}}\n",
                            )
                            .await;
                    }
                    _ => {}
                }
            }
        });
        let start = std::time::Instant::now();
        let out = run_appserver_protocol(cr, cw, "test", std::time::Duration::from_secs(4)).await;
        let elapsed = start.elapsed();
        assert!(out.is_none());
        assert!(
            elapsed < std::time::Duration::from_secs(2),
            "esperou {elapsed:?}; deveria retornar assim que o erro de id=2 chega, não aguardar o hard timeout de 4s"
        );
    }

    #[tokio::test]
    async fn appserver_grace_resolves_without_account() {
        // Responde id0 (capabilities) e id2 (rateLimits) mas NÃO id1 (account).
        // Após grace 200ms deve resolver com Some (plan_type None).
        use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
        let (client, server) = tokio::io::duplex(8192);
        let (cr, cw) = tokio::io::split(client);
        tokio::spawn(async move {
            let (sr, mut sw) = tokio::io::split(server);
            let mut lines = BufReader::new(sr).lines();
            while let Ok(Some(line)) = lines.next_line().await {
                let v: serde_json::Value = match serde_json::from_str(&line) {
                    Ok(v) => v,
                    Err(_) => continue,
                };
                match v.get("id").and_then(|i| i.as_i64()) {
                    Some(0) => {
                        let _ = sw
                            .write_all(b"{\"id\":0,\"result\":{\"capabilities\":{}}}\n")
                            .await;
                    }
                    // id1 (account/read) intencionalmente ignorado
                    Some(2) => {
                        let _ = sw
                            .write_all(b"{\"id\":2,\"result\":{\"rateLimits\":{\"limitId\":\"codex-default\",\"primary\":{\"usedPercent\":30,\"windowDurationMins\":300,\"resetsAt\":1700000000}}}}\n")
                            .await;
                    }
                    _ => {}
                }
            }
        });
        // timeout de 2s; grace de 200ms deve disparar antes
        let out = run_appserver_protocol(cr, cw, "test", std::time::Duration::from_secs(2)).await;
        let limits = out.expect("grace deve resolver");
        assert_eq!(limits.primary.as_ref().unwrap().used_percent, 30.0);
        // plan_type None porque account não foi recebido
        assert!(limits.plan_type.is_none());
    }

    // -----------------------------------------------------------------------
    // CodexProvider — Task 5
    // -----------------------------------------------------------------------

    #[tokio::test]
    async fn codex_provider_is_available_when_auth_exists() {
        use crate::providers::test_support::{ctx_for, settings};
        let tmp = tempfile::tempdir().unwrap();
        let settings = settings();
        let client = reqwest::Client::new();
        let ctx = ctx_for(tmp.path(), &settings, &client, 0);
        // codex_auth does not exist yet → false
        assert!(!QuotaSource::is_available(&CodexProvider, &ctx).await);
        // create the file → true
        std::fs::write(&ctx.paths.codex_auth, b"{}").unwrap();
        assert!(QuotaSource::is_available(&CodexProvider, &ctx).await);
    }

    #[test]
    fn codex_provider_to_user_facing_error_codex_variant() {
        let e = ProviderError::Codex(CodexError::NoSessionData);
        assert_eq!(
            CodexProvider.to_user_facing_error(&e),
            "No session data found"
        );
    }

    #[test]
    fn codex_provider_to_user_facing_error_non_codex_variant() {
        use crate::providers::error::AmpError;
        let e = ProviderError::Amp(AmpError::Generic);
        assert_eq!(
            CodexProvider.to_user_facing_error(&e),
            "Failed to fetch Codex usage"
        );
    }

    #[test]
    fn appserver_credits_tolerates_missing_bool_fields() {
        let c: CodexAppServerCredits = serde_json::from_str(r#"{"balance":"5"}"#).unwrap();
        assert!(!c.has_credits);
        assert!(!c.unlimited);
        assert_eq!(c.balance.as_deref(), Some("5"));
    }
}
