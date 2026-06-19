//! Notificações de quota (notify-send). Núcleo PURO (`plan_notifications`) +
//! IO best-effort (`check_and_notify`). Port fiel de `src/notify.ts`.
//! Os GATES de quando notificar (TTY, settings, comando) ficam no CALLER (Plano 5).

use std::collections::{BTreeMap, HashMap, HashSet};

use serde::{Deserialize, Serialize};

use crate::providers::extras::get_claude_extra;
use crate::providers::types::{AllQuotas, ProviderQuota, QuotaWindow};

/// Thresholds sobre %USADO de uma janela (contrato — não alterar).
pub const LOW_USED: f64 = 90.0;
pub const CRITICAL_USED: f64 = 95.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NotifyLevel {
    Ok,
    Low,
    Critical,
}

impl NotifyLevel {
    fn rank(self) -> u8 {
        match self {
            NotifyLevel::Ok => 0,
            NotifyLevel::Low => 1,
            NotifyLevel::Critical => 2,
        }
    }

    fn as_str(self) -> &'static str {
        match self {
            NotifyLevel::Ok => "ok",
            NotifyLevel::Low => "low",
            NotifyLevel::Critical => "critical",
        }
    }
}

pub fn level_for(used: f64) -> NotifyLevel {
    if used >= CRITICAL_USED {
        NotifyLevel::Critical
    } else if used >= LOW_USED {
        NotifyLevel::Low
    } else {
        NotifyLevel::Ok
    }
}

/// Estado persistido por-provider: maior nível já notificado por label de janela.
/// `windows` guarda strings cruas de nível (sanitizadas na leitura — stale → ok).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ProviderNotifyState {
    #[serde(default)]
    pub windows: BTreeMap<String, String>,
}

#[derive(Debug, Clone)]
pub struct NotifyFire {
    pub provider: String,
    pub display_name: String,
    pub label: String,
    pub used: f64,
    pub level: NotifyLevel, // low | critical
}

pub struct NotifyPlan {
    pub fires: Vec<NotifyFire>,
    pub next_states: HashMap<String, ProviderNotifyState>,
    pub changed: HashSet<String>,
}

fn used_of(w: &QuotaWindow) -> f64 {
    w.used.unwrap_or(100.0 - w.remaining)
}

/// Janelas distintas de um provider, com key de dedup, label e %usado. Ordem:
/// models → weeklyModels (Claude) → primary → secondary; dedup por
/// `(round(used), resetsAt)` (1º visto vence — o label amigável do model ganha
/// do alias primary/secondary).
fn windows_of(p: &ProviderQuota) -> Vec<(String, String, f64)> {
    let mut raw: Vec<(String, String, &QuotaWindow)> = Vec::new();
    if let Some(models) = p.models.as_ref() {
        for (name, w) in models {
            raw.push((format!("m:{name}"), name.clone(), w));
        }
    }
    if let Some(weekly) = get_claude_extra(p).and_then(|e| e.weekly_models.as_ref()) {
        for (name, w) in weekly {
            raw.push((format!("w:{name}"), format!("{name} (weekly)"), w));
        }
    }
    if let Some(pr) = p.primary.as_ref() {
        raw.push(("primary".to_string(), "primary".to_string(), pr));
    }
    if let Some(sec) = p.secondary.as_ref() {
        raw.push(("secondary".to_string(), "secondary".to_string(), sec));
    }

    let mut seen: HashSet<String> = HashSet::new();
    let mut out: Vec<(String, String, f64)> = Vec::new();
    for (key, label, w) in raw {
        let used = used_of(w);
        let sig = format!("{}|{}", used.round(), w.resets_at.as_deref().unwrap_or(""));
        if seen.contains(&sig) {
            continue;
        }
        seen.insert(sig);
        out.push((key, label, used));
    }
    out
}

/// Decisão pura: dado os quotas e o estado anterior, retorna o que disparar e o
/// próximo estado. Dispara SÓ quando uma janela ESCALA; re-arma (sem disparar)
/// na recuperação.
pub fn plan_notifications(
    quotas: &AllQuotas,
    prev_states: &HashMap<String, ProviderNotifyState>,
) -> NotifyPlan {
    let mut fires: Vec<NotifyFire> = Vec::new();
    let mut next_states: HashMap<String, ProviderNotifyState> = HashMap::new();
    let mut changed: HashSet<String> = HashSet::new();

    for p in &quotas.providers {
        if !p.available {
            continue;
        }
        let empty = ProviderNotifyState::default();
        let prev = prev_states.get(&p.provider).unwrap_or(&empty);
        let mut next: BTreeMap<String, String> = BTreeMap::new();

        for (key, label, used) in windows_of(p) {
            let current = level_for(used);
            // Sanitiza: valor stale/hand-edited que não é nível conhecido → ok.
            let previous = match prev.windows.get(&key).map(String::as_str) {
                Some("low") => NotifyLevel::Low,
                Some("critical") => NotifyLevel::Critical,
                _ => NotifyLevel::Ok,
            };

            if current.rank() > previous.rank() {
                fires.push(NotifyFire {
                    provider: p.provider.clone(),
                    display_name: p.display_name.clone(),
                    label,
                    used,
                    level: current,
                });
                next.insert(key, current.as_str().to_string());
                changed.insert(p.provider.clone());
            } else if current != previous {
                next.insert(key, current.as_str().to_string());
                changed.insert(p.provider.clone());
            } else if previous != NotifyLevel::Ok {
                next.insert(key, previous.as_str().to_string());
            }
        }

        next_states.insert(p.provider.clone(), ProviderNotifyState { windows: next });
    }

    NotifyPlan {
        fires,
        next_states,
        changed,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::providers::types::{ClaudeQuotaExtra, ProviderExtra, QuotaWindow};
    use indexmap::IndexMap;

    fn win(remaining: f64, resets: Option<&str>) -> QuotaWindow {
        QuotaWindow {
            remaining,
            resets_at: resets.map(str::to_string),
            window_minutes: None,
            used: None,
        }
    }

    fn wrap(providers: Vec<ProviderQuota>) -> AllQuotas {
        AllQuotas {
            providers,
            fetched_at: "2026-06-17T00:00:00.000Z".into(),
        }
    }

    fn claude(primary_remaining: f64) -> ProviderQuota {
        ProviderQuota {
            provider: "claude".into(),
            display_name: "Claude".into(),
            available: true,
            account: None,
            plan: None,
            plan_type: None,
            primary: Some(win(primary_remaining, None)),
            secondary: None,
            models: None,
            extra: None,
            error: None,
        }
    }

    #[test]
    fn level_for_classifies() {
        assert_eq!(level_for(0.0), NotifyLevel::Ok);
        assert_eq!(level_for(89.0), NotifyLevel::Ok);
        assert_eq!(level_for(90.0), NotifyLevel::Low);
        assert_eq!(level_for(94.0), NotifyLevel::Low);
        assert_eq!(level_for(95.0), NotifyLevel::Critical);
        assert_eq!(level_for(232.0), NotifyLevel::Critical);
    }

    #[test]
    fn fires_low_on_first_cross_90() {
        let plan = plan_notifications(&wrap(vec![claude(8.0)]), &HashMap::new()); // 92% used
        assert_eq!(plan.fires.len(), 1);
        assert_eq!(plan.fires[0].provider, "claude");
        assert_eq!(plan.fires[0].level, NotifyLevel::Low);
        assert_eq!(plan.fires[0].label, "primary");
        assert_eq!(
            plan.next_states["claude"]
                .windows
                .get("primary")
                .map(String::as_str),
            Some("low")
        );
        assert!(plan.changed.contains("claude"));
    }

    #[test]
    fn no_refire_at_same_level() {
        let mut prev = HashMap::new();
        prev.insert(
            "claude".to_string(),
            ProviderNotifyState {
                windows: BTreeMap::from([("primary".to_string(), "low".to_string())]),
            },
        );
        let plan = plan_notifications(&wrap(vec![claude(8.0)]), &prev);
        assert_eq!(plan.fires.len(), 0);
        assert!(!plan.changed.contains("claude"));
    }

    #[test]
    fn escalates_low_to_critical() {
        let mut prev = HashMap::new();
        prev.insert(
            "claude".to_string(),
            ProviderNotifyState {
                windows: BTreeMap::from([("primary".to_string(), "low".to_string())]),
            },
        );
        let plan = plan_notifications(&wrap(vec![claude(3.0)]), &prev); // 97% used
        assert_eq!(plan.fires.len(), 1);
        assert_eq!(plan.fires[0].level, NotifyLevel::Critical);
        assert_eq!(
            plan.next_states["claude"]
                .windows
                .get("primary")
                .map(String::as_str),
            Some("critical")
        );
    }

    #[test]
    fn rearms_on_recovery_without_firing() {
        let mut prev = HashMap::new();
        prev.insert(
            "claude".to_string(),
            ProviderNotifyState {
                windows: BTreeMap::from([("primary".to_string(), "low".to_string())]),
            },
        );
        let plan = plan_notifications(&wrap(vec![claude(80.0)]), &prev); // 20% used → ok
        assert_eq!(plan.fires.len(), 0);
        assert_eq!(
            plan.next_states["claude"]
                .windows
                .get("primary")
                .map(String::as_str),
            Some("ok")
        );
        assert!(plan.changed.contains("claude"));
    }

    #[test]
    fn fires_for_any_window_secondary() {
        let mut c = claude(50.0);
        c.secondary = Some(win(4.0, None)); // 96% used → critical
        let plan = plan_notifications(&wrap(vec![c]), &HashMap::new());
        assert_eq!(plan.fires.len(), 1);
        assert_eq!(plan.fires[0].label, "secondary");
        assert_eq!(plan.fires[0].level, NotifyLevel::Critical);
    }

    #[test]
    fn fires_for_model_window() {
        let mut c = claude(50.0);
        let mut models = IndexMap::new();
        models.insert("Sonnet".to_string(), win(9.0, None)); // 91% → low
        c.models = Some(models);
        let plan = plan_notifications(&wrap(vec![c]), &HashMap::new());
        assert_eq!(plan.fires.len(), 1);
        assert_eq!(plan.fires[0].label, "Sonnet");
        assert_eq!(plan.fires[0].level, NotifyLevel::Low);
    }

    #[test]
    fn honors_provider_used_over_100() {
        let mut c = claude(50.0);
        c.primary = Some(QuotaWindow {
            remaining: 0.0,
            resets_at: None,
            window_minutes: None,
            used: Some(232.0),
        });
        let plan = plan_notifications(&wrap(vec![c]), &HashMap::new());
        assert_eq!(plan.fires[0].label, "primary");
        assert_eq!(plan.fires[0].level, NotifyLevel::Critical);
        assert_eq!(plan.fires[0].used, 232.0);
    }

    #[test]
    fn skips_unavailable() {
        let p = ProviderQuota {
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
            error: Some("x".into()),
        };
        let plan = plan_notifications(&wrap(vec![p]), &HashMap::new());
        assert_eq!(plan.fires.len(), 0);
        assert!(!plan.next_states.contains_key("amp"));
    }

    #[test]
    fn dedups_primary_aliasing_model() {
        let mut models = IndexMap::new();
        models.insert(
            "Free Tier".to_string(),
            win(8.0, Some("2026-06-17T20:00:00Z")),
        );
        models.insert("Credits".to_string(), win(100.0, None));
        let amp = ProviderQuota {
            provider: "amp".into(),
            display_name: "Amp".into(),
            available: true,
            account: None,
            plan: None,
            plan_type: None,
            primary: Some(win(8.0, Some("2026-06-17T20:00:00Z"))),
            secondary: None,
            models: Some(models),
            extra: None,
            error: None,
        };
        let plan = plan_notifications(&wrap(vec![amp]), &HashMap::new());
        assert_eq!(plan.fires.len(), 1);
        assert_eq!(plan.fires[0].label, "Free Tier");
        assert_eq!(plan.fires[0].level, NotifyLevel::Low);
    }

    #[test]
    fn fires_for_claude_weekly_models() {
        let mut weekly = IndexMap::new();
        weekly.insert("Opus".to_string(), win(3.0, Some("2026-06-19T00:00:00Z"))); // 97% → critical
        let c = ProviderQuota {
            provider: "claude".into(),
            display_name: "Claude".into(),
            available: true,
            account: None,
            plan: None,
            plan_type: None,
            primary: Some(win(50.0, None)),
            secondary: None,
            models: None,
            extra: Some(ProviderExtra::Claude(ClaudeQuotaExtra {
                weekly_models: Some(weekly),
                extra_usage: None,
            })),
            error: None,
        };
        let plan = plan_notifications(&wrap(vec![c]), &HashMap::new());
        assert!(plan
            .fires
            .iter()
            .any(|f| f.label == "Opus (weekly)" && f.level == NotifyLevel::Critical));
    }
}
