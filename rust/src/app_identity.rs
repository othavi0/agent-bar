//! Constantes de identidade. Use estas em vez de strings hardcoded.

pub const APP_NAME: &str = "agent-bar";
pub const WAYBAR_NAMESPACE: &str = "agent-bar";
pub const WAYBAR_MODULE_PREFIX: &str = "custom/agent-bar-";
pub const WAYBAR_SELECTOR_PREFIX: &str = "#custom-agent-bar-";
pub const TERMINAL_HELPER_NAME: &str = "agent-bar-open-terminal";
pub const BACKUP_SUFFIX: &str = ".agent-bar-backup";
pub const APP_HIDDEN_CLASS: &str = "agent-bar-hidden";
pub const AMP_INSTALL_COMMAND: &str = "curl -fsSL https://ampcode.com/install.sh | bash";
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
