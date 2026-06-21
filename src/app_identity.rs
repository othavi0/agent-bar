//! Constantes de identidade. Use estas em vez de strings hardcoded.

pub const APP_NAME: &str = "agent-bar";
pub const WAYBAR_NAMESPACE: &str = "agent-bar";
pub const WAYBAR_MODULE_PREFIX: &str = "custom/agent-bar-";
pub const WAYBAR_SELECTOR_PREFIX: &str = "#custom-agent-bar-";
pub const TERMINAL_HELPER_NAME: &str = "agent-bar-open-terminal";
pub const BACKUP_SUFFIX: &str = ".agent-bar-backup";
pub const APP_HIDDEN_CLASS: &str = "agent-bar-hidden";
pub const AMP_INSTALL_COMMAND: &str = "curl -fsSL https://ampcode.com/install.sh | bash";
/// Classe CSS base do Waybar (= APP_NAME). Fonte: TS `APP_BASE_CLASS = APP_NAME`.
pub const APP_BASE_CLASS: &str = APP_NAME;

pub const VERSION: &str = env!("CARGO_PKG_VERSION");

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn app_base_class_is_app_name() {
        assert_eq!(APP_BASE_CLASS, "agent-bar");
        assert_eq!(APP_BASE_CLASS, APP_NAME);
    }
}
