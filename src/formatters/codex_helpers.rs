//! Derivação dos models do Codex a partir do ProviderQuota: agrupa janelas por
//! model (modelsDetailed + p.models classificados + fallback primary/secondary)
//! e ordena por severidade (menor remaining primeiro), desempate por nome.

use std::collections::BTreeMap;

use crate::formatters::shared::{classify_window, WindowKind};
use crate::providers::extras::get_codex_extra;
use crate::providers::types::{ModelWindows, ProviderQuota, QuotaWindow};

#[derive(Debug, Clone, PartialEq)]
pub struct CodexModelEntry {
    pub name: String,
    pub windows: ModelWindows,
    pub severity: f64,
}

/// Coloca `window` no bucket certo de `mw` seguindo a regra do TS:
/// fiveHour/sevenDay só se ainda vazio; senão cai em `other`.
fn place_window(mw: &mut ModelWindows, window: &QuotaWindow) {
    match classify_window(window.window_minutes) {
        WindowKind::FiveHour if mw.five_hour.is_none() => mw.five_hour = Some(window.clone()),
        WindowKind::SevenDay if mw.seven_day.is_none() => mw.seven_day = Some(window.clone()),
        _ => mw.other.get_or_insert_with(Vec::new).push(window.clone()),
    }
}

pub fn codex_models_from_quota(p: &ProviderQuota) -> Vec<CodexModelEntry> {
    let mut models: BTreeMap<String, ModelWindows> = BTreeMap::new();

    if let Some(detailed) = get_codex_extra(p).and_then(|e| e.models_detailed.as_ref()) {
        for (name, windows) in detailed {
            models.insert(name.clone(), windows.clone());
        }
    }

    if let Some(pm) = p.models.as_ref() {
        for (name, window) in pm {
            let entry = models.entry(name.clone()).or_default();
            place_window(entry, window);
        }
    }

    if models.is_empty() && (p.primary.is_some() || p.secondary.is_some()) {
        let mut fallback = ModelWindows::default();
        for window in [p.primary.as_ref(), p.secondary.as_ref()]
            .into_iter()
            .flatten()
        {
            place_window(&mut fallback, window);
        }
        models.insert("Codex".to_string(), fallback);
    }

    let mut entries: Vec<CodexModelEntry> = models
        .into_iter()
        .map(|(name, windows)| {
            let mut values: Vec<f64> = Vec::new();
            if let Some(w) = &windows.five_hour {
                values.push(w.remaining);
            }
            if let Some(w) = &windows.seven_day {
                values.push(w.remaining);
            }
            if let Some(others) = &windows.other {
                values.extend(others.iter().map(|w| w.remaining));
            }
            let severity = if values.is_empty() {
                101.0
            } else {
                values.iter().copied().fold(f64::INFINITY, f64::min)
            };
            CodexModelEntry {
                name,
                windows,
                severity,
            }
        })
        .collect();

    entries.sort_by(|a, b| {
        a.severity
            .partial_cmp(&b.severity)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| a.name.cmp(&b.name))
    });
    entries
}

/// Filtra para os models permitidos (lista de settings). Lista ausente/vazia → passthrough.
pub fn apply_codex_model_filter(
    models: Vec<CodexModelEntry>,
    allowed: Option<&[String]>,
) -> Vec<CodexModelEntry> {
    match allowed {
        Some(a) if !a.is_empty() => models
            .into_iter()
            .filter(|m| a.iter().any(|x| x == &m.name))
            .collect(),
        _ => models,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::providers::types::{CodexQuotaExtra, ProviderExtra, ProviderQuota, QuotaWindow};
    use indexmap::IndexMap;
    use std::collections::BTreeMap;

    fn win(remaining: f64, minutes: Option<i64>) -> QuotaWindow {
        QuotaWindow {
            remaining,
            resets_at: Some("2026-06-19T14:00:00Z".into()),
            window_minutes: minutes,
            used: None,
        }
    }

    fn quota_with_models(models: IndexMap<String, QuotaWindow>) -> ProviderQuota {
        ProviderQuota {
            provider: "codex".into(),
            display_name: "Codex".into(),
            available: true,
            account: None,
            plan: None,
            plan_type: None,
            primary: None,
            secondary: None,
            models: Some(models),
            extra: None,
            error: None,
        }
    }

    #[test]
    fn classifies_and_sorts_by_severity() {
        let mut m = IndexMap::new();
        m.insert("gpt-5".to_string(), win(80.0, Some(300))); // fiveHour, sev 80
        m.insert("o3".to_string(), win(20.0, Some(10080))); // sevenDay, sev 20
        let entries = codex_models_from_quota(&quota_with_models(m));
        assert_eq!(entries.len(), 2);
        // menor remaining (mais severo) primeiro
        assert_eq!(entries[0].name, "o3");
        assert_eq!(
            entries[0].windows.seven_day.as_ref().unwrap().remaining,
            20.0
        );
        assert_eq!(entries[1].name, "gpt-5");
        assert_eq!(
            entries[1].windows.five_hour.as_ref().unwrap().remaining,
            80.0
        );
    }

    #[test]
    fn fallback_to_codex_from_primary_secondary() {
        let mut q = quota_with_models(IndexMap::new());
        q.models = None;
        q.primary = Some(win(60.0, Some(300)));
        q.secondary = Some(win(50.0, Some(10080)));
        let entries = codex_models_from_quota(&q);
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].name, "Codex");
        assert_eq!(
            entries[0].windows.five_hour.as_ref().unwrap().remaining,
            60.0
        );
        assert_eq!(
            entries[0].windows.seven_day.as_ref().unwrap().remaining,
            50.0
        );
    }

    #[test]
    fn second_five_hour_goes_to_other() {
        // modelsDetailed dá fiveHour; p.models tenta outro fiveHour → vai p/ other
        let detailed = {
            let mut md = BTreeMap::new();
            let mw = crate::providers::types::ModelWindows {
                five_hour: Some(win(90.0, Some(300))),
                ..Default::default()
            };
            md.insert("gpt-5".to_string(), mw);
            md
        };
        let mut q = quota_with_models({
            let mut m = IndexMap::new();
            m.insert("gpt-5".to_string(), win(40.0, Some(300)));
            m
        });
        q.extra = Some(ProviderExtra::Codex(CodexQuotaExtra {
            models_detailed: Some(detailed),
            extra_usage: None,
        }));
        let entries = codex_models_from_quota(&q);
        assert_eq!(entries.len(), 1);
        let w = &entries[0].windows;
        assert_eq!(w.five_hour.as_ref().unwrap().remaining, 90.0);
        assert_eq!(w.other.as_ref().unwrap()[0].remaining, 40.0);
        // severity = min(90, 40) = 40
        assert_eq!(entries[0].severity, 40.0);
    }

    #[test]
    fn filter_keeps_only_allowed() {
        let mut m = IndexMap::new();
        m.insert("gpt-5".to_string(), win(80.0, Some(300)));
        m.insert("o3".to_string(), win(20.0, Some(300)));
        let all = codex_models_from_quota(&quota_with_models(m));
        let allowed = vec!["gpt-5".to_string()];
        let filtered = apply_codex_model_filter(all, Some(&allowed));
        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].name, "gpt-5");
    }

    #[test]
    fn empty_filter_is_passthrough() {
        let mut m = IndexMap::new();
        m.insert("gpt-5".to_string(), win(80.0, Some(300)));
        let all = codex_models_from_quota(&quota_with_models(m));
        let n = all.len();
        assert_eq!(apply_codex_model_filter(all, Some(&[])).len(), n);
    }
}
