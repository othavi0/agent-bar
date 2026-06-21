//! Envelope JSON versionado (sem Pango) para outras barras (Quickshell/Eww).
//! Bump em mudança incompatível (remover/renomear/retipar campo estável); adicionar
//! campo opcional NÃO exige bump.

use serde::Serialize;

use crate::providers::types::{AllQuotas, ProviderQuota};

pub const SCHEMA_VERSION: u32 = 1;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct JsonOutput<'a> {
    schema_version: u32,
    fetched_at: &'a str,
    providers: &'a [ProviderQuota],
}

/// Serializa o envelope versionado. `ProviderQuota` já tem a forma de saída correta.
pub fn to_json_string(quotas: &AllQuotas) -> Result<String, serde_json::Error> {
    let out = JsonOutput {
        schema_version: SCHEMA_VERSION,
        fetched_at: &quotas.fetched_at,
        providers: &quotas.providers,
    };
    serde_json::to_string(&out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::providers::types::{AllQuotas, ProviderQuota, QuotaWindow};

    fn sample() -> AllQuotas {
        AllQuotas {
            fetched_at: "2026-06-19T14:00:00Z".into(),
            providers: vec![ProviderQuota {
                provider: "claude".into(),
                display_name: "Claude".into(),
                available: true,
                account: None,
                plan: Some("Pro".into()),
                plan_type: None,
                primary: Some(QuotaWindow {
                    remaining: 60.0,
                    resets_at: None,
                    window_minutes: None,
                    used: None,
                }),
                secondary: None,
                models: None,
                extra: None,
                error: None,
            }],
        }
    }

    #[test]
    fn envelope_has_schema_version_and_fetched_at() {
        let s = to_json_string(&sample()).unwrap();
        let v: serde_json::Value = serde_json::from_str(&s).unwrap();
        assert_eq!(v["schemaVersion"], 1);
        assert_eq!(v["fetchedAt"], "2026-06-19T14:00:00Z");
        assert_eq!(v["providers"][0]["provider"], "claude");
        assert_eq!(v["providers"][0]["plan"], "Pro");
    }

    #[test]
    fn omits_absent_provider_fields() {
        let s = to_json_string(&sample()).unwrap();
        let v: serde_json::Value = serde_json::from_str(&s).unwrap();
        let p = &v["providers"][0];
        for absent in [
            "account",
            "planType",
            "secondary",
            "models",
            "extra",
            "error",
        ] {
            assert!(p.get(absent).is_none(), "{absent} deve ser omitido");
        }
    }

    #[test]
    fn never_contains_pango_markup() {
        let s = to_json_string(&sample()).unwrap();
        assert!(
            !s.contains("<span"),
            "envelope JSON nunca pode conter Pango"
        );
    }

    #[test]
    fn schema_version_is_one() {
        assert_eq!(SCHEMA_VERSION, 1);
    }
}
