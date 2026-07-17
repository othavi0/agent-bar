//! Getters para o payload `extra` específico de cada provider. O enum
//! `ProviderExtra` já discrimina, mas mantemos o gate por `provider` (string)
//! para reproduzir exatamente o comportamento do TS (`q.provider === '...'`).

use super::types::{
    AmpQuotaExtra, ClaudeQuotaExtra, CodexQuotaExtra, GrokQuotaExtra, ProviderExtra, ProviderQuota,
};

/// Payload Claude-específico, ou None para outros providers.
pub fn get_claude_extra(q: &ProviderQuota) -> Option<&ClaudeQuotaExtra> {
    match (q.provider == "claude", &q.extra) {
        (true, Some(ProviderExtra::Claude(e))) => Some(e),
        _ => None,
    }
}

/// Payload Codex-específico, ou None para outros providers.
pub fn get_codex_extra(q: &ProviderQuota) -> Option<&CodexQuotaExtra> {
    match (q.provider == "codex", &q.extra) {
        (true, Some(ProviderExtra::Codex(e))) => Some(e),
        _ => None,
    }
}

/// Payload Amp-específico, ou None para outros providers.
pub fn get_amp_extra(q: &ProviderQuota) -> Option<&AmpQuotaExtra> {
    match (q.provider == "amp", &q.extra) {
        (true, Some(ProviderExtra::Amp(e))) => Some(e),
        _ => None,
    }
}

/// Payload Grok-específico, ou None para outros providers.
pub fn get_grok_extra(q: &ProviderQuota) -> Option<&GrokQuotaExtra> {
    match (q.provider == "grok", &q.extra) {
        (true, Some(ProviderExtra::Grok(e))) => Some(e),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::providers::types::{
        ClaudeQuotaExtra, CodexQuotaExtra, GrokQuotaExtra, ProviderExtra, ProviderQuota,
    };

    fn base(provider: &str, extra: Option<ProviderExtra>) -> ProviderQuota {
        ProviderQuota {
            provider: provider.into(),
            display_name: "X".into(),
            available: true,
            account: None,
            plan: None,
            plan_type: None,
            primary: None,
            secondary: None,
            models: None,
            extra,
            error: None,
        }
    }

    #[test]
    fn claude_getter_returns_payload() {
        let q = base(
            "claude",
            Some(ProviderExtra::Claude(ClaudeQuotaExtra::default())),
        );
        assert!(get_claude_extra(&q).is_some());
        assert!(get_codex_extra(&q).is_none());
        assert!(get_amp_extra(&q).is_none());
        assert!(get_grok_extra(&q).is_none());
    }

    #[test]
    fn getter_gated_by_provider_string() {
        // provider diz "codex" mas o payload é Claude → todos None (fidelidade ao gate do TS).
        let q = base(
            "codex",
            Some(ProviderExtra::Claude(ClaudeQuotaExtra::default())),
        );
        assert!(get_claude_extra(&q).is_none());
        assert!(get_codex_extra(&q).is_none());
    }

    #[test]
    fn codex_getter_returns_payload() {
        let q = base(
            "codex",
            Some(ProviderExtra::Codex(CodexQuotaExtra::default())),
        );
        assert!(get_codex_extra(&q).is_some());
    }

    #[test]
    fn none_extra_returns_none() {
        let q = base("amp", None);
        assert!(get_amp_extra(&q).is_none());
    }

    #[test]
    fn grok_getter_returns_payload() {
        let q = base(
            "grok",
            Some(ProviderExtra::Grok(GrokQuotaExtra::default())),
        );
        assert!(get_grok_extra(&q).is_some());
        assert!(get_claude_extra(&q).is_none());
        assert!(get_codex_extra(&q).is_none());
        assert!(get_amp_extra(&q).is_none());
    }

    #[test]
    fn grok_getter_gated_by_provider_string() {
        // provider diz "amp" mas o payload é Grok → None.
        let q = base(
            "amp",
            Some(ProviderExtra::Grok(GrokQuotaExtra::default())),
        );
        assert!(get_grok_extra(&q).is_none());
        assert!(get_amp_extra(&q).is_none());
    }
}
