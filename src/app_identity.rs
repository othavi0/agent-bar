//! Product identity constants. Prefer these over hardcoded strings.

pub const APP_NAME: &str = "agent-bar";
pub const TERMINAL_HELPER_NAME: &str = "agent-bar-open-terminal";
/// Omarchy Quattro plugin ID (bar-widget). The `omarchy.*` prefix is reserved.
pub const OMARCHY_PLUGIN_ID: &str = "agent-bar.usage";
/// Omarchy Quickshell root — detection signal only.
pub const OMARCHY_SHELL_DIR: &str = "/usr/share/omarchy/shell";

pub const VERSION: &str = env!("CARGO_PKG_VERSION");

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn omarchy_plugin_id_is_namespaced_and_not_reserved() {
        assert_eq!(OMARCHY_PLUGIN_ID, "agent-bar.usage");
        assert!(!OMARCHY_PLUGIN_ID.starts_with("omarchy."));
        assert!(OMARCHY_PLUGIN_ID.contains('.'));
        assert!(OMARCHY_SHELL_DIR.starts_with('/'));
    }

    /// Release identity: package version, manifest, helper, and archive
    /// name must all match (BUNDLE-006 / docs/dev/releasing.md).
    #[test]
    fn package_version_is_release_identity() {
        assert_eq!(
            VERSION, "10.3.0",
            "Cargo.toml package version must match the release identity"
        );
    }
}
