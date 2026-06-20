//! Extração de gastos $ do Amp a partir do `meta` já parseado pelo provider.
//! Reutiliza as chaves `freeRemaining`/`freeTotal` que `providers::amp` insere
//! no `BTreeMap<String,String>` (formato "$X.XX"). NÃO reexecuta a CLI.

use std::collections::BTreeMap;

/// Gastos em dólar do Amp Free Tier, derivados do `meta` do provider.
#[derive(Debug, Clone, PartialEq)]
pub struct AmpDollars {
    /// Quanto já foi gasto (= total − remaining). `None` se um dos dois ausente.
    pub spent: Option<f64>,
    /// Quanto ainda resta (`freeRemaining`). `None` se ausente/malformado.
    pub remaining: Option<f64>,
    /// Crédito total do ciclo (`freeTotal`). `None` se ausente/malformado.
    pub total: Option<f64>,
}

/// Parseia `"$X.XX"` → `f64`. Strip do `$` e `parse`; falha → `None`.
fn parse_dollar(s: &str) -> Option<f64> {
    s.strip_prefix('$')
        .and_then(|rest| rest.parse::<f64>().ok())
}

/// Extrai `AmpDollars` do mapa `meta` produzido por `providers::amp::parse_usage`.
/// As chaves esperadas são `"freeRemaining"` e `"freeTotal"` (formato `"$X.XX"`).
/// Nunca re-executa a CLI; só interpreta o que já veio parseado.
pub fn amp_dollar_usage(meta: &BTreeMap<String, String>) -> AmpDollars {
    let remaining = meta.get("freeRemaining").and_then(|s| parse_dollar(s));
    let total = meta.get("freeTotal").and_then(|s| parse_dollar(s));
    let spent = remaining.zip(total).map(|(r, t)| t - r);
    AmpDollars {
        spent,
        remaining,
        total,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn meta(pairs: &[(&str, &str)]) -> BTreeMap<String, String> {
        pairs
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect()
    }

    #[test]
    fn amp_dollar_usage_parses_free_tier() {
        let m = meta(&[("freeRemaining", "$3.5"), ("freeTotal", "$5")]);
        let d = amp_dollar_usage(&m);
        assert_eq!(d.remaining, Some(3.5));
        assert_eq!(d.total, Some(5.0));
        // spent = 5 - 3.5 = 1.5
        let spent = d.spent.unwrap();
        assert!((spent - 1.5).abs() < 1e-9, "got {spent}");
    }

    #[test]
    fn amp_dollar_usage_missing_keys_returns_none() {
        let m = meta(&[]);
        let d = amp_dollar_usage(&m);
        assert!(d.remaining.is_none());
        assert!(d.total.is_none());
        assert!(d.spent.is_none());
    }

    #[test]
    fn amp_dollar_usage_only_remaining_no_spent() {
        let m = meta(&[("freeRemaining", "$3.5")]);
        let d = amp_dollar_usage(&m);
        assert_eq!(d.remaining, Some(3.5));
        assert!(d.total.is_none());
        assert!(d.spent.is_none());
    }

    #[test]
    fn amp_dollar_usage_malformed_value_returns_none() {
        let m = meta(&[("freeRemaining", "not-a-number"), ("freeTotal", "$5")]);
        let d = amp_dollar_usage(&m);
        assert!(d.remaining.is_none());
        assert_eq!(d.total, Some(5.0));
        assert!(d.spent.is_none());
    }
}
