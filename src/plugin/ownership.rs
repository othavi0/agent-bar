//! Ownership classification and before-hash evidence (CLEAN-001..005).

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use sha2::{Digest, Sha256};

/// Five ownership classes from the migration contract.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OwnershipClass {
    OwnedCurrent,
    OwnedLegacy,
    ModifiedLegacy,
    Ambiguous,
    Unrelated,
}

impl OwnershipClass {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::OwnedCurrent => "owned/current",
            Self::OwnedLegacy => "owned/legacy",
            Self::ModifiedLegacy => "modified legacy",
            Self::Ambiguous => "ambiguous",
            Self::Unrelated => "unrelated",
        }
    }

    /// Automatic cleanup may remove only owned/legacy (CLEAN-001).
    pub fn may_auto_remove(self) -> bool {
        matches!(self, Self::OwnedLegacy)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FileKind {
    File,
    Dir,
    Symlink,
    Other,
}

/// Captured evidence for one path in a plan/report.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OwnershipEvidence {
    pub class: OwnershipClass,
    pub path: PathBuf,
    pub reason: String,
    pub before_hash: Option<String>,
    pub size: Option<u64>,
    pub mode: Option<u32>,
    pub file_type: FileKind,
}

/// Known markers used to prove ownership (CLEAN-002). Location alone is never enough.
#[derive(Debug, Clone, Default)]
pub struct OwnershipRules {
    /// Exact path → expected sha256 of current owned content.
    pub current_hashes: Vec<(PathBuf, String)>,
    /// Exact path → expected sha256 of known legacy content.
    pub legacy_hashes: Vec<(PathBuf, String)>,
    /// Exact generated marker strings found inside a file.
    pub markers: Vec<String>,
    /// Paths recorded in an install/migration manifest as owned.
    pub manifest_paths: Vec<PathBuf>,
}

pub fn hash_bytes(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    digest.iter().map(|b| format!("{b:02x}")).collect()
}

pub fn hash_path(path: &Path) -> io::Result<String> {
    let bytes = fs::read(path)?;
    Ok(hash_bytes(&bytes))
}

fn file_kind(meta: &fs::Metadata) -> FileKind {
    let ft = meta.file_type();
    if ft.is_symlink() {
        FileKind::Symlink
    } else if ft.is_dir() {
        FileKind::Dir
    } else if ft.is_file() {
        FileKind::File
    } else {
        FileKind::Other
    }
}

fn mode_of(meta: &fs::Metadata) -> Option<u32> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        Some(meta.permissions().mode())
    }
    #[cfg(not(unix))]
    {
        let _ = meta;
        None
    }
}

/// Classify one artifact. Filename resemblance alone never yields owned/* (CLEAN-003).
pub fn classify_artifact(path: &Path, rules: &OwnershipRules) -> OwnershipEvidence {
    let meta = match fs::symlink_metadata(path) {
        Ok(m) => m,
        Err(_) => {
            return OwnershipEvidence {
                class: OwnershipClass::Unrelated,
                path: path.to_path_buf(),
                reason: "path does not exist".into(),
                before_hash: None,
                size: None,
                mode: None,
                file_type: FileKind::Other,
            };
        }
    };

    let file_type = file_kind(&meta);
    let size = Some(meta.len());
    let mode = mode_of(&meta);

    // Symlinks are never auto-owned without following policy: treat as ambiguous.
    if file_type == FileKind::Symlink {
        return OwnershipEvidence {
            class: OwnershipClass::Ambiguous,
            path: path.to_path_buf(),
            reason: "symlink requires manual review".into(),
            before_hash: None,
            size,
            mode,
            file_type,
        };
    }

    let hash = if file_type == FileKind::File {
        hash_path(path).ok()
    } else {
        None
    };

    if let Some(ref h) = hash {
        for (p, expected) in &rules.current_hashes {
            if p == path && expected == h {
                return OwnershipEvidence {
                    class: OwnershipClass::OwnedCurrent,
                    path: path.to_path_buf(),
                    reason: "matching current content hash".into(),
                    before_hash: hash,
                    size,
                    mode,
                    file_type,
                };
            }
        }
        for (p, expected) in &rules.legacy_hashes {
            if p == path && expected == h {
                return OwnershipEvidence {
                    class: OwnershipClass::OwnedLegacy,
                    path: path.to_path_buf(),
                    reason: "matching known legacy content hash".into(),
                    before_hash: hash,
                    size,
                    mode,
                    file_type,
                };
            }
        }
        // Same path as known legacy/current but different hash → modified legacy.
        for (p, _) in rules
            .legacy_hashes
            .iter()
            .chain(rules.current_hashes.iter())
        {
            if p == path {
                return OwnershipEvidence {
                    class: OwnershipClass::ModifiedLegacy,
                    path: path.to_path_buf(),
                    reason: "known path with non-matching content hash".into(),
                    before_hash: hash,
                    size,
                    mode,
                    file_type,
                };
            }
        }
    }

    if rules.manifest_paths.iter().any(|p| p == path) {
        return OwnershipEvidence {
            class: OwnershipClass::OwnedLegacy,
            path: path.to_path_buf(),
            reason: "recorded in install/migration manifest".into(),
            before_hash: hash,
            size,
            mode,
            file_type,
        };
    }

    if file_type == FileKind::File {
        if let Ok(text) = fs::read_to_string(path) {
            for marker in &rules.markers {
                if text.contains(marker) {
                    return OwnershipEvidence {
                        class: OwnershipClass::OwnedLegacy,
                        path: path.to_path_buf(),
                        reason: format!("contains generated marker {marker:?}"),
                        before_hash: hash,
                        size,
                        mode,
                        file_type,
                    };
                }
            }
        }
    }

    // Path looks agent-bar-ish but no proof → ambiguous (report, do not remove).
    let name = path
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    let path_s = path.to_string_lossy().to_ascii_lowercase();
    if name.contains("agent-bar") || path_s.contains("agent-bar") {
        return OwnershipEvidence {
            class: OwnershipClass::Ambiguous,
            path: path.to_path_buf(),
            reason: "name resemblance without proof".into(),
            before_hash: hash,
            size,
            mode,
            file_type,
        };
    }

    OwnershipEvidence {
        class: OwnershipClass::Unrelated,
        path: path.to_path_buf(),
        reason: "outside agent-bar ownership proofs".into(),
        before_hash: hash,
        size,
        mode,
        file_type,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn five_classes_and_auto_remove_only_legacy() {
        assert!(OwnershipClass::OwnedLegacy.may_auto_remove());
        assert!(!OwnershipClass::OwnedCurrent.may_auto_remove());
        assert!(!OwnershipClass::ModifiedLegacy.may_auto_remove());
        assert!(!OwnershipClass::Ambiguous.may_auto_remove());
        assert!(!OwnershipClass::Unrelated.may_auto_remove());
        assert_eq!(OwnershipClass::OwnedCurrent.as_str(), "owned/current");
        assert_eq!(OwnershipClass::OwnedLegacy.as_str(), "owned/legacy");
        assert_eq!(OwnershipClass::ModifiedLegacy.as_str(), "modified legacy");
        assert_eq!(OwnershipClass::Ambiguous.as_str(), "ambiguous");
        assert_eq!(OwnershipClass::Unrelated.as_str(), "unrelated");
    }

    #[test]
    fn hash_matches_current_and_legacy() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("module.css");
        fs::write(&path, b"/* agent-bar generated */\n").unwrap();
        let h = hash_path(&path).unwrap();
        let rules = OwnershipRules {
            current_hashes: vec![(path.clone(), h.clone())],
            ..OwnershipRules::default()
        };
        let ev = classify_artifact(&path, &rules);
        assert_eq!(ev.class, OwnershipClass::OwnedCurrent);
        assert_eq!(ev.before_hash.as_deref(), Some(h.as_str()));

        let rules = OwnershipRules {
            legacy_hashes: vec![(path.clone(), hash_bytes(b"/* agent-bar generated */\n"))],
            ..OwnershipRules::default()
        };
        assert_eq!(
            classify_artifact(&path, &rules).class,
            OwnershipClass::OwnedLegacy
        );
    }

    #[test]
    fn modified_legacy_when_hash_diverges() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("module.css");
        fs::write(&path, b"user edited").unwrap();
        let rules = OwnershipRules {
            legacy_hashes: vec![(path.clone(), hash_bytes(b"original"))],
            ..OwnershipRules::default()
        };
        assert_eq!(
            classify_artifact(&path, &rules).class,
            OwnershipClass::ModifiedLegacy
        );
    }

    #[test]
    fn name_resemblance_without_proof_is_ambiguous() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("agent-bar-mystery.txt");
        fs::write(&path, b"no marker").unwrap();
        let rules = OwnershipRules::default();
        assert_eq!(
            classify_artifact(&path, &rules).class,
            OwnershipClass::Ambiguous
        );
    }

    #[test]
    fn marker_proves_owned_legacy() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("style.css");
        fs::write(&path, b"/* BEGIN agent-bar-waybar */\n.foo{}\n").unwrap();
        let rules = OwnershipRules {
            markers: vec!["BEGIN agent-bar-waybar".into()],
            ..OwnershipRules::default()
        };
        assert_eq!(
            classify_artifact(&path, &rules).class,
            OwnershipClass::OwnedLegacy
        );
    }

    #[test]
    fn manifest_path_proves_owned_legacy() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("helper.sh");
        fs::write(&path, b"#!/bin/sh\n").unwrap();
        let rules = OwnershipRules {
            manifest_paths: vec![path.clone()],
            ..OwnershipRules::default()
        };
        assert_eq!(
            classify_artifact(&path, &rules).class,
            OwnershipClass::OwnedLegacy
        );
    }

    #[test]
    fn unrelated_plain_file() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("notes.txt");
        fs::write(&path, b"hello").unwrap();
        assert_eq!(
            classify_artifact(&path, &OwnershipRules::default()).class,
            OwnershipClass::Unrelated
        );
    }
}
