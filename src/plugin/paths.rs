//! Plugin path layout and path-safety helpers (MIG-002A, BUNDLE-007A).

use std::fs;
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
    /// `$HOME/.config/omarchy/plugins` or an injected test plugins dir.
    pub plugins_dir: PathBuf,
    /// `plugins_dir/agent-bar.usage` — production install root.
    pub plugin_root: PathBuf,
    pub xdg_state: PathBuf,
    pub backups_dir: PathBuf,
    pub transactions_dir: PathBuf,
    pub reports_dir: PathBuf,
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

    /// Test / `setup plugins-dir` layout with an injected plugins parent.
    pub fn with_plugins_dir(
        home: impl Into<PathBuf>,
        plugins_dir: impl Into<PathBuf>,
        xdg_state: impl Into<PathBuf>,
    ) -> Self {
        Self::from_parts(home.into(), plugins_dir.into(), xdg_state.into())
    }

    fn from_parts(home: PathBuf, plugins_dir: PathBuf, xdg_state: PathBuf) -> Self {
        let agent_state = xdg_state.join("agent-bar");
        Self {
            home,
            plugin_root: plugins_dir.join(PLUGIN_ID),
            plugins_dir,
            backups_dir: agent_state.join("backups"),
            transactions_dir: agent_state.join("transactions"),
            reports_dir: agent_state.join("reports"),
            maintenance_lock: agent_state.join("maintenance.lock"),
            xdg_state: agent_state,
        }
    }

    /// Destination-local stage sibling (hidden; ignored by Quattro discovery).
    pub fn stage_dir(&self, txid: &str) -> Result<PathBuf, PathError> {
        validate_txid(txid)?;
        Ok(self.plugins_dir.join(format!(".{PLUGIN_ID}.stage-{txid}")))
    }

    /// Destination-local quarantine sibling after exchange.
    pub fn quarantine_dir(&self, txid: &str) -> Result<PathBuf, PathError> {
        validate_txid(txid)?;
        Ok(self
            .plugins_dir
            .join(format!(".{PLUGIN_ID}.quarantine-{txid}")))
    }

    /// Journal path under XDG state (never inside the replaced plugin dir).
    pub fn journal_path(&self, txid: &str) -> Result<PathBuf, PathError> {
        validate_txid(txid)?;
        Ok(self.transactions_dir.join(format!("{txid}.journal.json")))
    }

    /// Durable backup root for one operation (outside the target).
    pub fn backup_root(&self, stamp: &str) -> PathBuf {
        self.backups_dir.join(stamp)
    }

    /// Settings-file quarantine sibling (MIG-002A).
    pub fn settings_quarantine(settings_path: &Path, txid: &str) -> Result<PathBuf, PathError> {
        validate_txid(txid)?;
        let parent = settings_path
            .parent()
            .ok_or_else(|| PathError::msg("settings path has no parent"))?;
        let name = settings_path
            .file_name()
            .and_then(|s| s.to_str())
            .ok_or_else(|| PathError::msg("settings path has no file name"))?;
        Ok(parent.join(format!(".{name}.agent-bar-quarantine-{txid}")))
    }

    /// Cache-root quarantine sibling (MIG-002A):
    /// `<cache-parent>/.agent-bar-cache-quarantine-<txid>/`.
    pub fn cache_quarantine(cache_root: &Path, txid: &str) -> Result<PathBuf, PathError> {
        validate_txid(txid)?;
        let parent = cache_root
            .parent()
            .ok_or_else(|| PathError::msg("cache root has no parent"))?;
        Ok(parent.join(format!(".agent-bar-cache-quarantine-{txid}")))
    }

    /// Backups-dir quarantine sibling (MIG-002A):
    /// `<backup-parent>/.agent-bar-backups-quarantine-<txid>/`.
    pub fn backups_quarantine(backups_dir: &Path, txid: &str) -> Result<PathBuf, PathError> {
        validate_txid(txid)?;
        let parent = backups_dir
            .parent()
            .ok_or_else(|| PathError::msg("backups dir has no parent"))?;
        Ok(parent.join(format!(".agent-bar-backups-quarantine-{txid}")))
    }
}

/// Transaction IDs are exactly 32 lowercase hex characters.
pub fn validate_txid(txid: &str) -> Result<(), PathError> {
    if txid.len() != 32 {
        return Err(PathError::msg(format!(
            "transaction id must be 32 lowercase hex chars, got length {}",
            txid.len()
        )));
    }
    if !txid.chars().all(|c| matches!(c, '0'..='9' | 'a'..='f')) {
        return Err(PathError::msg(
            "transaction id must be 32 lowercase hex characters",
        ));
    }
    Ok(())
}

/// Generate a random-looking 32-hex txid from a clock/nonce seed (tests inject).
pub fn txid_from_bytes(bytes: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    let digest = Sha256::digest(bytes);
    digest.iter().take(16).map(|b| format!("{b:02x}")).collect()
}

/// True when a plugins-dir child is a hidden stage/quarantine sibling, not a plugin ID.
pub fn is_hidden_plugin_sibling(name: &str) -> bool {
    name.starts_with(&format!(".{PLUGIN_ID}.stage-"))
        || name.starts_with(&format!(".{PLUGIN_ID}.quarantine-"))
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

/// Fail when `path` exists and is a symlink (canonical install roots must be real dirs).
pub fn ensure_not_symlink(path: &Path) -> Result<(), PathError> {
    match fs::symlink_metadata(path) {
        Ok(meta) if meta.file_type().is_symlink() => Err(PathError::msg(format!(
            "symlink rejected at {}",
            path.display()
        ))),
        Ok(_) => Ok(()),
        Err(err) if err.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(err) => Err(err.into()),
    }
}

/// Compare device IDs so stage/exchange stay same-filesystem (MIG-002).
pub fn same_filesystem(a: &Path, b: &Path) -> Result<bool, PathError> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        let ma = fs::metadata(a).or_else(|_| {
            a.parent()
                .map(fs::metadata)
                .transpose()?
                .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "no parent"))
        })?;
        let mb = fs::metadata(b).or_else(|_| {
            b.parent()
                .map(fs::metadata)
                .transpose()?
                .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "no parent"))
        })?;
        Ok(ma.dev() == mb.dev())
    }
    #[cfg(not(unix))]
    {
        let _ = (a, b);
        Ok(true)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::symlink;
    use tempfile::tempdir;

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
    fn injected_plugins_dir_for_tests() {
        let p = PluginPaths::with_plugins_dir("/tmp/home", "/tmp/plugins", "/tmp/state");
        assert_eq!(p.plugin_root, PathBuf::from("/tmp/plugins/agent-bar.usage"));
        assert_eq!(
            p.transactions_dir,
            PathBuf::from("/tmp/state/agent-bar/transactions")
        );
    }

    #[test]
    fn txid_must_be_32_lowercase_hex() {
        assert!(validate_txid("0123456789abcdef0123456789abcdef").is_ok());
        assert!(validate_txid("0123456789ABCDEF0123456789abcdef").is_err());
        assert!(validate_txid("short").is_err());
        assert!(validate_txid("gggggggggggggggggggggggggggggggg").is_err());
    }

    #[test]
    fn stage_and_quarantine_are_hidden_siblings() {
        let p = PluginPaths::with_plugins_dir("/h", "/plugins", "/state");
        let tx = "0123456789abcdef0123456789abcdef";
        let stage = p.stage_dir(tx).unwrap();
        let q = p.quarantine_dir(tx).unwrap();
        assert_eq!(
            stage.file_name().unwrap().to_str().unwrap(),
            ".agent-bar.usage.stage-0123456789abcdef0123456789abcdef"
        );
        assert!(is_hidden_plugin_sibling(
            stage.file_name().unwrap().to_str().unwrap()
        ));
        assert!(is_hidden_plugin_sibling(
            q.file_name().unwrap().to_str().unwrap()
        ));
        assert!(!is_hidden_plugin_sibling(PLUGIN_ID));
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
    fn symlink_plugin_root_rejected() {
        let dir = tempdir().unwrap();
        let real = dir.path().join("real");
        fs::create_dir_all(&real).unwrap();
        let link = dir.path().join("link");
        symlink(&real, &link).unwrap();
        assert!(ensure_not_symlink(&link).is_err());
        assert!(ensure_not_symlink(&real).is_ok());
        assert!(ensure_not_symlink(&dir.path().join("missing")).is_ok());
    }

    #[test]
    fn same_filesystem_true_for_siblings() {
        let dir = tempdir().unwrap();
        let a = dir.path().join("a");
        let b = dir.path().join("b");
        fs::write(&a, b"1").unwrap();
        fs::write(&b, b"2").unwrap();
        assert!(same_filesystem(&a, &b).unwrap());
    }

    #[test]
    fn settings_quarantine_is_hidden_sibling() {
        let settings = PathBuf::from("/home/u/.config/agent-bar/settings.json");
        let q = PluginPaths::settings_quarantine(&settings, "0123456789abcdef0123456789abcdef")
            .unwrap();
        assert_eq!(
            q,
            PathBuf::from(
                "/home/u/.config/agent-bar/.settings.json.agent-bar-quarantine-0123456789abcdef0123456789abcdef"
            )
        );
    }

    #[test]
    fn cache_and_backups_quarantine_are_destination_local() {
        let tx = "0123456789abcdef0123456789abcdef";
        let cache = PathBuf::from("/home/u/.cache/agent-bar");
        let q = PluginPaths::cache_quarantine(&cache, tx).unwrap();
        assert_eq!(
            q,
            PathBuf::from(
                "/home/u/.cache/.agent-bar-cache-quarantine-0123456789abcdef0123456789abcdef"
            )
        );
        let backups = PathBuf::from("/home/u/.local/state/agent-bar/backups");
        let bq = PluginPaths::backups_quarantine(&backups, tx).unwrap();
        assert_eq!(
            bq,
            PathBuf::from(
                "/home/u/.local/state/agent-bar/.agent-bar-backups-quarantine-0123456789abcdef0123456789abcdef"
            )
        );
    }

    #[test]
    fn backups_never_inside_plugin_root() {
        let p = PluginPaths::production("/home/u", None);
        assert!(!p.backups_dir.starts_with(&p.plugin_root));
        assert!(!p.transactions_dir.starts_with(&p.plugin_root));
    }
}
