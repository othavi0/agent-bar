//! Amp provider. Estende `QuotaSource` (04a): spawn de `amp usage` + parse regex.
//! Port fiel de `src/providers/amp.ts`. NÃO há `amp usage --json` → regex no texto.

use std::collections::BTreeMap;
use std::sync::OnceLock;

use indexmap::IndexMap;
use regex::Regex;

use super::iso_from_ms;
use super::types::{AmpQuotaExtra, ProviderExtra, ProviderQuota, QuotaWindow};

fn re(cell: &'static OnceLock<Option<Regex>>, pattern: &str) -> Option<&'static Regex> {
    cell.get_or_init(|| Regex::new(pattern).ok()).as_ref()
}

macro_rules! lazy_re {
    ($name:ident, $pat:expr) => {{
        static CELL: OnceLock<Option<Regex>> = OnceLock::new();
        re(&CELL, $pat)
    }};
}

/// `$` + número estilo `Number.toString()` (Display do f64 = shortest round-trip).
fn dollars(n: f64) -> String {
    format!("${n}")
}

/// Parse do stdout de `amp usage` para `ProviderQuota`. `now_ms` é o relógio
/// atual (o `fullAt` recalcula a cada chamada, inclusive em cache hit).
pub fn parse_usage(stdout: &str, base: ProviderQuota, now_ms: u64) -> ProviderQuota {
    let cap1 = |re: Option<&Regex>, i: usize| -> Option<String> {
        re.and_then(|r| r.captures(stdout))
            .and_then(|c| c.get(i).map(|m| m.as_str().to_string()))
    };

    let account = cap1(lazy_re!(RE_SIGNED, r"Signed in as (\S+)"), 1);

    let free_re = lazy_re!(RE_FREE, r"Amp Free:\s*\$([0-9.]+)/\$([0-9.]+)\s*remaining");
    let free_caps = free_re.and_then(|r| r.captures(stdout));

    let replenish = cap1(lazy_re!(RE_REPLENISH, r"replenishes \+\$([0-9.]+)/hour"), 1);
    let bonus_re = lazy_re!(RE_BONUS, r"\+(\d+)%\s*bonus\s*for\s*(\d+)\s*more\s*days");
    let bonus_caps = bonus_re.and_then(|r| r.captures(stdout));
    let bonus_pct = bonus_caps
        .as_ref()
        .and_then(|c| c.get(1))
        .map(|m| m.as_str().to_string());
    let bonus_days = bonus_caps
        .as_ref()
        .and_then(|c| c.get(2))
        .map(|m| m.as_str().to_string());

    let credits = cap1(
        lazy_re!(RE_CREDITS, r"Individual credits:\s*\$([0-9.]+)\s*remaining"),
        1,
    );

    let mut models: IndexMap<String, QuotaWindow> = IndexMap::new();
    let mut meta: BTreeMap<String, String> = BTreeMap::new();
    let mut primary: Option<QuotaWindow> = None;

    if let Some(fc) = free_caps {
        let remaining: f64 = fc
            .get(1)
            .and_then(|m| m.as_str().parse().ok())
            .unwrap_or(0.0);
        let total: f64 = fc
            .get(2)
            .and_then(|m| m.as_str().parse().ok())
            .unwrap_or(0.0);
        let pct = if total > 0.0 {
            (remaining / total * 100.0).round()
        } else {
            0.0
        };

        // fullAt: só com replenish e não-cheio.
        let full_at: Option<String> = if let Some(rep) = replenish.as_deref() {
            if remaining < total {
                let rate: f64 = rep.parse().unwrap_or(0.0);
                let eff = match bonus_pct.as_deref() {
                    Some(b) => rate * (1.0 + b.parse::<f64>().unwrap_or(0.0) / 100.0),
                    None => rate,
                };
                let hours = (total - remaining) / eff;
                if eff > 0.0 && hours.is_finite() {
                    Some(iso_from_ms((now_ms as f64 + hours * 3_600_000.0) as u64))
                } else {
                    None
                }
            } else {
                None
            }
        } else {
            None
        };

        let window = QuotaWindow {
            remaining: pct,
            resets_at: full_at.clone(),
            window_minutes: None,
            used: None,
        };
        primary = Some(window.clone());
        models.insert("Free Tier".to_string(), window);
        meta.insert("freeRemaining".to_string(), dollars(remaining));
        meta.insert("freeTotal".to_string(), dollars(total));
        if let Some(rep) = replenish.as_deref() {
            meta.insert("replenishRate".to_string(), format!("+${rep}/hr"));
        }
        if let (Some(p), Some(d)) = (bonus_pct.as_deref(), bonus_days.as_deref()) {
            meta.insert("bonus".to_string(), format!("+{p}% ({d}d)"));
        }
    }

    if let Some(bal_str) = credits.as_deref() {
        let balance: f64 = bal_str.parse().unwrap_or(0.0);
        models.insert(
            "Credits".to_string(),
            QuotaWindow {
                remaining: if balance > 0.0 { 100.0 } else { 0.0 },
                resets_at: None,
                window_minutes: None,
                used: None,
            },
        );
        meta.insert("creditsBalance".to_string(), dollars(balance));
    }

    let extra = if meta.is_empty() {
        None
    } else {
        Some(ProviderExtra::Amp(AmpQuotaExtra { meta: Some(meta) }))
    };

    ProviderQuota {
        provider: "amp".to_string(),
        available: true,
        account,
        primary,
        models: Some(models),
        extra,
        ..base
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const NOW: u64 = 1_700_000_000_000; // relógio fixo

    fn base() -> ProviderQuota {
        ProviderQuota {
            provider: "amp".into(),
            display_name: "Amp".into(),
            available: false,
            account: None,
            plan: None,
            plan_type: None,
            primary: None,
            secondary: None,
            models: None,
            extra: None,
            error: None,
        }
    }

    fn meta_of(q: &ProviderQuota) -> &BTreeMap<String, String> {
        match q.extra.as_ref() {
            Some(ProviderExtra::Amp(a)) => a.meta.as_ref().expect("meta"),
            _ => panic!("expected Amp extra"),
        }
    }

    const FULL: &str = "Signed in as user@email.com\nAmp Free: $3.50/$5.00 remaining\nreplenishes +$0.25/hour\n+20% bonus for 5 more days\nIndividual credits: $10.00 remaining";

    #[test]
    fn parses_full_output() {
        let q = parse_usage(FULL, base(), NOW);
        assert!(q.available);
        assert_eq!(q.account.as_deref(), Some("user@email.com"));
        assert_eq!(q.primary.as_ref().unwrap().remaining, 70.0); // 3.5/5 = 70%
        let models = q.models.as_ref().unwrap();
        assert_eq!(models["Free Tier"].remaining, 70.0);
        assert_eq!(models["Credits"].remaining, 100.0);
        // ordem de inserção: Free Tier antes de Credits (IndexMap)
        let keys: Vec<&str> = models.keys().map(String::as_str).collect();
        assert_eq!(keys, vec!["Free Tier", "Credits"]);
        let m = meta_of(&q);
        assert_eq!(m.get("freeRemaining").map(String::as_str), Some("$3.5"));
        assert_eq!(m.get("freeTotal").map(String::as_str), Some("$5"));
        assert_eq!(
            m.get("replenishRate").map(String::as_str),
            Some("+$0.25/hr")
        );
        assert_eq!(m.get("bonus").map(String::as_str), Some("+20% (5d)"));
        assert_eq!(m.get("creditsBalance").map(String::as_str), Some("$10"));
        // fullAt presente e no futuro
        let resets = q.primary.as_ref().unwrap().resets_at.as_deref().unwrap();
        assert!(resets.ends_with('Z'));
    }

    #[test]
    fn eta_with_bonus_is_about_5h() {
        // eff = 0.25 * 1.20 = 0.30; hours = 1.5/0.30 = 5.0
        let q = parse_usage(FULL, base(), NOW);
        let resets = q.primary.as_ref().unwrap().resets_at.as_deref().unwrap();
        // 5h após NOW
        let expected = iso_from_ms(NOW + 5 * 3_600_000);
        assert_eq!(resets, expected);
    }

    #[test]
    fn no_bonus_eta_is_6h_and_meta_omits_bonus() {
        let out =
            "Signed in as user@email.com\nAmp Free: $3.50/$5.00 remaining\nreplenishes +$0.25/hour";
        let q = parse_usage(out, base(), NOW);
        let m = meta_of(&q);
        assert!(m.get("bonus").is_none());
        assert_eq!(
            m.get("replenishRate").map(String::as_str),
            Some("+$0.25/hr")
        );
        // hours = 1.5/0.25 = 6.0
        let resets = q.primary.as_ref().unwrap().resets_at.as_deref().unwrap();
        assert_eq!(resets, iso_from_ms(NOW + 6 * 3_600_000));
        // sem credits
        assert!(q.models.as_ref().unwrap().get("Credits").is_none());
    }

    #[test]
    fn no_replenish_means_null_resets_and_no_meta_rate() {
        let out = "Signed in as user@email.com\nAmp Free: $3.50/$5.00 remaining";
        let q = parse_usage(out, base(), NOW);
        assert!(q.primary.as_ref().unwrap().resets_at.is_none());
        let m = meta_of(&q);
        assert!(m.get("replenishRate").is_none());
        assert!(m.get("bonus").is_none());
    }

    #[test]
    fn full_quota_has_null_resets() {
        let out =
            "Signed in as user@email.com\nAmp Free: $5.00/$5.00 remaining\nreplenishes +$0.25/hour";
        let q = parse_usage(out, base(), NOW);
        assert_eq!(q.primary.as_ref().unwrap().remaining, 100.0);
        assert!(q.primary.as_ref().unwrap().resets_at.is_none());
    }

    #[test]
    fn zero_replenish_stays_available_null_resets() {
        let out =
            "Signed in as user@email.com\nAmp Free: $3.50/$5.00 remaining\nreplenishes +$0/hour";
        let q = parse_usage(out, base(), NOW);
        assert!(q.available);
        assert_eq!(q.primary.as_ref().unwrap().remaining, 70.0);
        assert!(q.primary.as_ref().unwrap().resets_at.is_none());
    }

    #[test]
    fn zero_credits_balance_means_remaining_zero() {
        let out = "Signed in as user@email.com\nAmp Free: $3.50/$5.00 remaining\nreplenishes +$0.25/hour\nIndividual credits: $0.00 remaining";
        let q = parse_usage(out, base(), NOW);
        assert_eq!(q.models.as_ref().unwrap()["Credits"].remaining, 0.0);
        assert_eq!(
            meta_of(&q).get("creditsBalance").map(String::as_str),
            Some("$0")
        );
    }
}
