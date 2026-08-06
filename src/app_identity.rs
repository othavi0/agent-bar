//! Product identity constants. Prefer these over hardcoded strings.

pub const APP_NAME: &str = "agent-bar";
pub const TERMINAL_HELPER_NAME: &str = "agent-bar-open-terminal";
/// Omarchy Quattro plugin ID (bar-widget). The `omarchy.*` prefix is reserved.
pub const OMARCHY_PLUGIN_ID: &str = "othavi0.agent-bar";
/// Omarchy Quickshell root — detection signal only.
pub const OMARCHY_SHELL_DIR: &str = "/usr/share/omarchy/shell";

// Release identity (version == manifest == helper == tag) is enforced by
// scripts/check-version inside the auto-release workflow; a literal pin here
// would fail every automated bump.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn omarchy_plugin_id_is_namespaced_and_not_reserved() {
        assert_eq!(OMARCHY_PLUGIN_ID, "othavi0.agent-bar");
        assert!(!OMARCHY_PLUGIN_ID.starts_with("omarchy."));
        assert!(OMARCHY_PLUGIN_ID.contains('.'));
        assert!(OMARCHY_SHELL_DIR.starts_with('/'));
    }
}
