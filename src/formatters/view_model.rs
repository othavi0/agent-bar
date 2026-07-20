//! View model do Codex resolvido a partir de settings já carregadas (puro). A
//! variante que carrega settings frescas vive na superfície (Plano 3c).

use crate::providers::types::ProviderQuota;
use crate::settings::{Settings, WindowPolicy};

use super::codex_helpers::{apply_codex_model_filter, codex_models_from_quota, CodexModelEntry};

/// Dados que o builder do Codex precisa: models filtrados + window policy.
#[derive(Debug, Clone, PartialEq)]
pub struct CodexViewModel {
    pub models: Vec<CodexModelEntry>,
    pub policy: WindowPolicy,
}

/// Deriva o view model a partir de settings já carregadas.
pub fn resolve_codex_view_model_from(settings: &Settings, p: &ProviderQuota) -> CodexViewModel {
    let policy = settings
        .window_policy
        .get(&p.provider)
        .copied()
        .unwrap_or(WindowPolicy::Both);
    let allowed = settings.models.get(&p.provider).map(|v| v.as_slice());
    let models = apply_codex_model_filter(codex_models_from_quota(p), allowed);
    CodexViewModel { models, policy }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Paths;
    use crate::providers::types::{ProviderQuota, QuotaWindow};
    use crate::settings::{load, WindowPolicy};
    use indexmap::IndexMap;
    use std::path::PathBuf;
    use tempfile::tempdir;

    fn paths_in(dir: &std::path::Path) -> Paths {
        Paths {
            cache_dir: dir.join("cache"),
            config_dir: dir.join("config"),
            claude_credentials: PathBuf::new(),
            codex_auth: PathBuf::new(),
            codex_sessions: PathBuf::new(),
            amp_settings: PathBuf::new(),
            amp_threads: PathBuf::new(),
            grok_home: PathBuf::new(),
            grok_auth: PathBuf::new(),
        }
    }

    fn codex_quota() -> ProviderQuota {
        let mut m = IndexMap::new();
        m.insert(
            "gpt-5".to_string(),
            QuotaWindow {
                remaining: 80.0,
                resets_at: None,
                window_minutes: Some(300),
                used: None,
                severity: None,
            },
        );
        ProviderQuota {
            provider: "codex".into(),
            display_name: "Codex".into(),
            available: true,
            account: None,
            plan: None,
            plan_type: None,
            primary: None,
            secondary: None,
            models: Some(m),
            extra: None,
            error: None,
            stale_reason: None,
        }
    }

    #[test]
    fn resolves_policy_and_models() {
        let dir = tempdir().unwrap();
        let settings = load(&paths_in(dir.path())); // defaults: window_policy[codex]=Both
        let vm = resolve_codex_view_model_from(&settings, &codex_quota());
        assert_eq!(vm.policy, WindowPolicy::Both);
        assert_eq!(vm.models.len(), 1);
        assert_eq!(vm.models[0].name, "gpt-5");
    }
}
