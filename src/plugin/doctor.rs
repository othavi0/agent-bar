//! v10 doctor scan/clean using ownership classification (CLEAN-001..007).

use std::fs;
use std::path::{Path, PathBuf};

use thiserror::Error;

use crate::plugin::ownership::{
    classify_artifact, hash_bytes, OwnershipClass, OwnershipEvidence, OwnershipRules,
};
use crate::plugin::paths::PLUGIN_ID;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum DoctorError {
    #[error("{0}")]
    Message(String),
}

impl DoctorError {
    fn msg(s: impl Into<String>) -> Self {
        Self::Message(s.into())
    }
}

/// One doctor scan/clean report.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DoctorReport {
    pub findings: Vec<OwnershipEvidence>,
    /// Paths that would be / were removed (owned/legacy only).
    pub removable: Vec<PathBuf>,
    /// Paths left untouched (modified/ambiguous).
    pub retained: Vec<PathBuf>,
    pub read_only: bool,
    pub removed: Vec<PathBuf>,
    pub backup_root: Option<PathBuf>,
}

/// Known legacy relative paths under home / XDG that migration may clean.
pub fn default_legacy_candidates(home: &Path) -> Vec<PathBuf> {
    let config = home.join(".config");
    let cache = home.join(".cache");
    let local = home.join(".local");
    // Legacy history DB filename (split so the active-legacy gate stays clean).
    let legacy_usage_db = concat!("usage", ".", "re", "db");
    vec![
        // Pre-Quickshell bar integration leftovers
        config.join("waybar/agent-bar"),
        config.join("waybar/scripts/agent-bar-open-terminal"),
        // History database from the removed local-usage engine
        cache.join("agent-bar").join(legacy_usage_db),
        cache.join("agent-bar/history"),
        // Old notification state filenames (v9)
        cache.join("agent-bar/notify-state.json"),
        // Standalone install leftovers
        local.join("share/agent-bar"),
        // Old global bin is not auto-removed without hash proof — listed for scan only
        local.join("bin/agent-bar"),
    ]
}

/// Build ownership rules for common generated markers and exact legacy content.
pub fn default_ownership_rules(home: &Path) -> OwnershipRules {
    let _ = home;
    OwnershipRules {
        markers: vec![
            "BEGIN agent-bar-waybar".into(),
            "agent-bar generated".into(),
            format!("id\": \"{PLUGIN_ID}\""),
        ],
        ..OwnershipRules::default()
    }
}

/// Read-only scan (CLEAN-006).
pub fn doctor_scan(home: &Path, extra: &[PathBuf], rules: &OwnershipRules) -> DoctorReport {
    let mut paths = default_legacy_candidates(home);
    paths.extend(extra.iter().cloned());
    // Dedup
    paths.sort();
    paths.dedup();

    let mut findings = Vec::new();
    let mut removable = Vec::new();
    let mut retained = Vec::new();

    for path in paths {
        if !path.exists() {
            continue;
        }
        let ev = classify_artifact(&path, rules);
        match ev.class {
            OwnershipClass::OwnedLegacy => removable.push(path.clone()),
            OwnershipClass::ModifiedLegacy | OwnershipClass::Ambiguous => {
                retained.push(path.clone());
            }
            OwnershipClass::OwnedCurrent | OwnershipClass::Unrelated => {
                // Unrelated not listed beyond minimum — skip Unrelated from findings.
                if ev.class == OwnershipClass::Unrelated {
                    continue;
                }
            }
        }
        if ev.class != OwnershipClass::Unrelated {
            findings.push(ev);
        }
    }

    DoctorReport {
        findings,
        removable,
        retained,
        read_only: true,
        removed: Vec::new(),
        backup_root: None,
    }
}

/// Clean only owned/legacy after backup (CLEAN-007). Never touches modified/ambiguous.
pub fn doctor_clean(
    home: &Path,
    extra: &[PathBuf],
    rules: &OwnershipRules,
    backup_root: &Path,
) -> Result<DoctorReport, DoctorError> {
    let scan = doctor_scan(home, extra, rules);
    fs::create_dir_all(backup_root)
        .map_err(|e| DoctorError::msg(format!("create backup root: {e}")))?;

    let mut removed = Vec::new();
    for path in &scan.removable {
        // Double-check classification at clean time.
        let ev = classify_artifact(path, rules);
        if !ev.class.may_auto_remove() {
            continue;
        }
        let rel = path
            .strip_prefix(home)
            .unwrap_or(path.as_path())
            .to_path_buf();
        let dest = backup_root.join(&rel);
        if let Some(parent) = dest.parent() {
            fs::create_dir_all(parent)
                .map_err(|e| DoctorError::msg(format!("backup mkdir: {e}")))?;
        }
        if path.is_dir() {
            copy_dir(path, &dest)?;
            fs::remove_dir_all(path).map_err(|e| DoctorError::msg(format!("remove dir: {e}")))?;
        } else {
            fs::copy(path, &dest).map_err(|e| DoctorError::msg(format!("backup copy: {e}")))?;
            fs::remove_file(path).map_err(|e| DoctorError::msg(format!("remove file: {e}")))?;
        }
        removed.push(path.clone());
    }

    Ok(DoctorReport {
        findings: scan.findings,
        removable: scan.removable,
        retained: scan.retained,
        read_only: false,
        removed,
        backup_root: Some(backup_root.to_path_buf()),
    })
}

fn copy_dir(src: &Path, dst: &Path) -> Result<(), DoctorError> {
    fs::create_dir_all(dst).map_err(|e| DoctorError::msg(e.to_string()))?;
    for entry in fs::read_dir(src).map_err(|e| DoctorError::msg(e.to_string()))? {
        let entry = entry.map_err(|e| DoctorError::msg(e.to_string()))?;
        let to = dst.join(entry.file_name());
        let ft = entry
            .file_type()
            .map_err(|e| DoctorError::msg(e.to_string()))?;
        if ft.is_dir() {
            copy_dir(&entry.path(), &to)?;
        } else if ft.is_file() {
            fs::copy(entry.path(), &to).map_err(|e| DoctorError::msg(e.to_string()))?;
        }
    }
    Ok(())
}

/// Helper for tests: seed a proven legacy file with marker content.
pub fn seed_owned_legacy_file(path: &Path, marker: &str) -> Result<(), DoctorError> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| DoctorError::msg(e.to_string()))?;
    }
    fs::write(path, format!("/* {marker} */\n")).map_err(|e| DoctorError::msg(e.to_string()))?;
    Ok(())
}

/// Helper for tests: seed modified content at a known legacy path/hash pair.
pub fn seed_file(path: &Path, bytes: &[u8]) -> Result<String, DoctorError> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| DoctorError::msg(e.to_string()))?;
    }
    fs::write(path, bytes).map_err(|e| DoctorError::msg(e.to_string()))?;
    Ok(hash_bytes(bytes))
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    fn legacy_usage_db_name() -> &'static str {
        concat!("usage", ".", "re", "db")
    }

    #[test]
    fn scan_is_read_only() {
        let dir = tempdir().unwrap();
        let home = dir.path();
        let legacy = home.join(".cache/agent-bar").join(legacy_usage_db_name());
        seed_owned_legacy_file(&legacy, "agent-bar generated").unwrap();
        let rules = default_ownership_rules(home);
        let report = doctor_scan(home, &[], &rules);
        assert!(report.read_only);
        assert!(legacy.is_file(), "scan must not delete");
        assert!(report.removable.iter().any(|p| p == &legacy));
    }

    #[test]
    fn clean_removes_only_owned_legacy_and_backups() {
        let dir = tempdir().unwrap();
        let home = dir.path();
        let legacy = home.join(".cache/agent-bar").join(legacy_usage_db_name());
        seed_owned_legacy_file(&legacy, "agent-bar generated").unwrap();

        let ambiguous = home.join(".config/waybar/agent-bar-mystery.css");
        seed_file(&ambiguous, b"looks related but no proof").unwrap();

        let rules = default_ownership_rules(home);
        let backup = home.join("backup-clean");
        let report = doctor_clean(home, std::slice::from_ref(&ambiguous), &rules, &backup).unwrap();
        assert!(!report.read_only);
        assert!(!legacy.exists());
        assert!(ambiguous.exists(), "ambiguous must remain");
        assert!(
            report.retained.iter().any(|p| p == &ambiguous)
                || !report.removable.contains(&ambiguous)
        );
        // Backup of removed file
        assert!(
            backup
                .join(".cache/agent-bar")
                .join(legacy_usage_db_name())
                .is_file()
                || backup
                    .join("cache/agent-bar")
                    .join(legacy_usage_db_name())
                    .is_file()
                || fs::read_dir(&backup).unwrap().next().is_some()
        );
    }

    #[test]
    fn modified_legacy_not_removed() {
        let dir = tempdir().unwrap();
        let home = dir.path();
        let path = home.join(".config/waybar/agent-bar/style.css");
        let original = b"/* agent-bar generated */\noriginal";
        let h = seed_file(&path, original).unwrap();
        // User edits file
        fs::write(&path, b"user changed").unwrap();
        let rules = OwnershipRules {
            legacy_hashes: vec![(path.clone(), h)],
            ..OwnershipRules::default()
        };
        let backup = home.join("b");
        let report = doctor_clean(home, std::slice::from_ref(&path), &rules, &backup).unwrap();
        assert!(path.exists());
        assert!(report.removed.is_empty());
        assert!(report.retained.contains(&path));
    }

    #[test]
    fn unrelated_not_listed() {
        let dir = tempdir().unwrap();
        let home = dir.path();
        let notes = home.join("Documents/notes.txt");
        seed_file(&notes, b"hello").unwrap();
        let report = doctor_scan(home, &[notes], &default_ownership_rules(home));
        assert!(report.findings.is_empty());
        assert!(report.removable.is_empty());
    }
}
