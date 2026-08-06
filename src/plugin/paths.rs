//! Plugin path layout and path-safety helpers (MIG-002A, BUNDLE-007A).

use std::io;
use std::path::{Component, Path, PathBuf};

use thiserror::Error;

/// Canonical product plugin ID and directory name.
pub const PLUGIN_ID: &str = "agent-bar.usage";

#[derive(Debug, Error)]
pub enum PathError {
    #[error("{0}")]
    Message(String),
    #[error(transparent)]
    Io(#[from] io::Error),
}

impl PathError {
    pub fn msg(s: impl Into<String>) -> Self {
        Self::Message(s.into())
    }
}

/// Resolved layout for plugin install and maintenance transactions.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PluginPaths {
    pub home: PathBuf,
    /// `$HOME/.config/omarchy/plugins`.
    pub plugins_dir: PathBuf,
    /// `plugins_dir/agent-bar.usage` — production install root.
    pub plugin_root: PathBuf,
    pub xdg_state: PathBuf,
    pub backups_dir: PathBuf,
    pub maintenance_lock: PathBuf,
}

impl PluginPaths {
    /// Production layout: literal `$HOME/.config/omarchy/plugins` and XDG state.
    pub fn production(home: impl Into<PathBuf>, xdg_state: Option<PathBuf>) -> Self {
        let home = home.into();
        let plugins_dir = home.join(".config/omarchy/plugins");
        let state = xdg_state.unwrap_or_else(|| home.join(".local/state"));
        Self::from_parts(home, plugins_dir, state)
    }

    fn from_parts(home: PathBuf, plugins_dir: PathBuf, xdg_state: PathBuf) -> Self {
        let agent_state = xdg_state.join("agent-bar");
        Self {
            home,
            plugin_root: plugins_dir.join(PLUGIN_ID),
            plugins_dir,
            backups_dir: agent_state.join("backups"),
            maintenance_lock: agent_state.join("maintenance.lock"),
            xdg_state: agent_state,
        }
    }

    /// Durable backup root for one operation (outside the target).
    pub fn backup_root(&self, stamp: &str) -> PathBuf {
        self.backups_dir.join(stamp)
    }
}

/// Generate a random-looking 32-hex txid from a clock/nonce seed (tests inject).
pub fn txid_from_bytes(bytes: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    let digest = Sha256::digest(bytes);
    digest.iter().take(16).map(|b| format!("{b:02x}")).collect()
}

/// Reject absolute paths, `..`, empty components, and NUL (archive/inventory safety).
pub fn validate_archive_entry_path(rel: &str) -> Result<(), PathError> {
    if rel.is_empty() {
        return Err(PathError::msg("empty archive path"));
    }
    if rel.contains('\0') {
        return Err(PathError::msg("archive path contains NUL"));
    }
    if rel.starts_with('/') || rel.starts_with('\\') {
        return Err(PathError::msg(format!(
            "absolute archive path rejected: {rel}"
        )));
    }
    // Windows drive / UNC style
    if rel.len() >= 2 && rel.as_bytes()[1] == b':' {
        return Err(PathError::msg(format!(
            "absolute archive path rejected: {rel}"
        )));
    }
    let path = Path::new(rel);
    for comp in path.components() {
        match comp {
            Component::Normal(s) => {
                if s.is_empty() {
                    return Err(PathError::msg("empty path component"));
                }
            }
            Component::CurDir => {}
            Component::ParentDir => {
                return Err(PathError::msg(format!(
                    "parent directory component rejected: {rel}"
                )));
            }
            Component::RootDir | Component::Prefix(_) => {
                return Err(PathError::msg(format!(
                    "absolute archive path rejected: {rel}"
                )));
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn production_uses_literal_omarchy_plugins() {
        let p = PluginPaths::production("/home/alice", None);
        assert_eq!(
            p.plugins_dir,
            PathBuf::from("/home/alice/.config/omarchy/plugins")
        );
        assert_eq!(
            p.plugin_root,
            PathBuf::from("/home/alice/.config/omarchy/plugins/agent-bar.usage")
        );
        assert_eq!(
            p.maintenance_lock,
            PathBuf::from("/home/alice/.local/state/agent-bar/maintenance.lock")
        );
    }

    #[test]
    fn archive_paths_reject_traversal_and_absolute() {
        assert!(validate_archive_entry_path("manifest.json").is_ok());
        assert!(validate_archive_entry_path("bin/agent-bar").is_ok());
        assert!(validate_archive_entry_path("../etc/passwd").is_err());
        assert!(validate_archive_entry_path("/etc/passwd").is_err());
        assert!(validate_archive_entry_path("foo/../../bar").is_err());
        assert!(validate_archive_entry_path("").is_err());
        assert!(validate_archive_entry_path("C:\\windows").is_err());
    }

    #[test]
    fn backups_never_inside_plugin_root() {
        let p = PluginPaths::production("/home/u", None);
        assert!(!p.backups_dir.starts_with(&p.plugin_root));
    }
}
