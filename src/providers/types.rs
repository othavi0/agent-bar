//! Modelo de quota normalizado, agnóstico de provider. SERIALIZE-ONLY: o cache
//! guarda a resposta crua do provider, não este tipo — então `ProviderQuota`
//! nunca é desserializado, o que evita a ambiguidade de enums untagged.

use std::collections::BTreeMap;

use indexmap::IndexMap;
use serde::Serialize;

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct QuotaWindow {
    pub remaining: f64,
    /// Sempre presente no JSON (pode ser null).
    pub resets_at: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub window_minutes: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub used: Option<f64>,
    /// Severidade vinda da API (`limits[].severity` do Claude). `None` =
    /// calcular localmente por threshold. Omitida do JSON quando ausente
    /// (mantém golden/waybar_contract intactos).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub severity: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ModelWindows {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub five_hour: Option<QuotaWindow>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub seven_day: Option<QuotaWindow>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub other: Option<Vec<QuotaWindow>>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExtraUsage {
    pub enabled: bool,
    pub remaining: f64,
    pub limit: f64,
    pub used: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ClaudeQuotaExtra {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub weekly_models: Option<IndexMap<String, QuotaWindow>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub extra_usage: Option<ExtraUsage>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct CodexQuotaExtra {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub models_detailed: Option<BTreeMap<String, ModelWindows>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub extra_usage: Option<ExtraUsage>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct AmpQuotaExtra {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub meta: Option<BTreeMap<String, String>>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct GrokQuotaExtra {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sessions_today: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub turns_today: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub context_tokens_used: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub context_window_tokens: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub recent_model: Option<String>,
}

/// Untagged: serializa apenas o conteúdo do struct interno (sem chave de variante),
/// reproduzindo a forma de `extra` do TS.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(untagged)]
pub enum ProviderExtra {
    Claude(ClaudeQuotaExtra),
    Codex(CodexQuotaExtra),
    Amp(AmpQuotaExtra),
    Grok(GrokQuotaExtra),
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderQuota {
    pub provider: String,
    pub display_name: String,
    pub available: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub account: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub plan: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub plan_type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub primary: Option<QuotaWindow>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub secondary: Option<QuotaWindow>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub models: Option<IndexMap<String, QuotaWindow>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub extra: Option<ProviderExtra>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AllQuotas {
    pub providers: Vec<ProviderQuota>,
    pub fetched_at: String,
}

#[cfg(test)]
mod tests {
    use super::*;
    use indexmap::IndexMap;

    fn window(remaining: f64) -> QuotaWindow {
        QuotaWindow {
            remaining,
            resets_at: Some("2026-06-19T14:00:00Z".into()),
            window_minutes: None,
            used: None,
            severity: None,
        }
    }

    #[test]
    fn quota_window_omits_none_optionals() {
        let j = serde_json::to_value(window(75.0)).unwrap();
        assert_eq!(j["remaining"], 75.0);
        assert_eq!(j["resetsAt"], "2026-06-19T14:00:00Z");
        assert!(
            j.get("windowMinutes").is_none(),
            "windowMinutes deve ser omitido quando None"
        );
        assert!(j.get("used").is_none(), "used deve ser omitido quando None");
    }

    #[test]
    fn quota_window_keeps_null_resets_at() {
        let w = QuotaWindow {
            remaining: 100.0,
            resets_at: None,
            window_minutes: Some(300),
            used: None,
            severity: None,
        };
        let j = serde_json::to_value(w).unwrap();
        assert!(
            j.as_object().unwrap().contains_key("resetsAt"),
            "resetsAt sempre presente (pode ser null)"
        );
        assert_eq!(j["resetsAt"], serde_json::Value::Null);
        assert_eq!(j["windowMinutes"], 300);
    }

    #[test]
    fn provider_quota_omits_absent_fields() {
        let q = ProviderQuota {
            provider: "claude".into(),
            display_name: "Claude".into(),
            available: true,
            account: None,
            plan: None,
            plan_type: None,
            primary: Some(window(60.0)),
            secondary: None,
            models: None,
            extra: None,
            error: None,
        };
        let j = serde_json::to_value(q).unwrap();
        assert_eq!(j["provider"], "claude");
        assert_eq!(j["displayName"], "Claude");
        assert_eq!(j["available"], true);
        assert!(j["primary"].is_object());
        for absent in [
            "account",
            "plan",
            "planType",
            "secondary",
            "models",
            "extra",
            "error",
        ] {
            assert!(
                j.get(absent).is_none(),
                "{absent} deve ser omitido quando None"
            );
        }
    }

    #[test]
    fn provider_extra_serializes_untagged() {
        let mut weekly = IndexMap::new();
        weekly.insert("Opus".to_string(), window(50.0));
        let extra = ProviderExtra::Claude(ClaudeQuotaExtra {
            weekly_models: Some(weekly),
            extra_usage: None,
        });
        let j = serde_json::to_value(extra).unwrap();
        // untagged: emite os campos do struct interno diretamente (sem chave de variante)
        assert!(j["weeklyModels"]["Opus"].is_object());
        assert!(j.get("extraUsage").is_none());
    }

    #[test]
    fn all_quotas_field_names() {
        let aq = AllQuotas {
            providers: vec![],
            fetched_at: "2026-06-19T14:00:00Z".into(),
        };
        let j = serde_json::to_value(aq).unwrap();
        assert_eq!(j["fetchedAt"], "2026-06-19T14:00:00Z");
        assert!(j["providers"].is_array());
    }
}
