//! Plugin release stamping, receipt (`bundle.json`), and tree validation.
//!
//! The plugin QML/JS/manifest tree lives at the repo root (monorepo
//! migration). `BundleBuilder::stamp` does not assemble a separate tree; it
//! stamps release artifacts (private helper, marketplace preview image,
//! `bundle.json`) directly into that root and validates the shipped scope.

use std::collections::{BTreeMap, BTreeSet};
use std::ffi::OsStr;
use std::fs::{self, File, OpenOptions};
use std::io::{self, Write};
use std::path::{Component, Path};
use std::process::Command;

use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::plugin::ownership::hash_bytes;
use crate::plugin::paths::{validate_archive_entry_path, PLUGIN_ID};

/// Official first-class Rust target for the product bundle.
pub const OFFICIAL_TARGET: &str = "x86_64-unknown-linux-gnu";
/// Quattro / Omarchy contract version embedded in receipts and metadata.
pub const OMARCHY_CONTRACT: u32 = 1;
/// Minimum Quickshell version required by the Omarchy contract.
pub const MINIMUM_QUICKSHELL_VERSION: &str = "0.3.0";
/// Bundle receipt / metadata schema version.
pub const BUNDLE_SCHEMA_VERSION: u32 = 1;

/// Shipped top-level files at the repo root -- the complete inventory scope
/// for `stamp`, `collect_inventory`, and `reject_special_files`. The repo
/// root also holds `src/`, `docs/`, `target/`, and other non-shipped trees
/// that are not the bundle's business.
pub const SHIPPED_ROOT_FILES: &[&str] = &[
    "BarWidget.qml",
    "CoreMaintenance.js",
    "CoreScroll.js",
    "CoreService.js",
    "CoreSettings.js",
    "CoreView.js",
    "LICENSE",
    "MaintenanceView.qml",
    "Popup.qml",
    "ProviderRail.qml",
    "ProviderView.qml",
    "README.md",
    "Service.qml",
    "SettingsView.qml",
    "bin/agent-bar",
    "manifest.json",
    "preview.png",
    "scripts/agent-bar-open-terminal",
];
/// Shipped directories, walked in full. See [`SHIPPED_ROOT_FILES`].
pub const SHIPPED_DIRS: &[&str] = &["components", "icons"];

const EXEC_MODE: &str = "0755";

#[derive(Debug, Error)]
pub enum BundleError {
    #[error("{0}")]
    Message(String),
    #[error(transparent)]
    Io(#[from] io::Error),
    #[error(transparent)]
    Json(#[from] serde_json::Error),
}

impl BundleError {
    pub fn msg(s: impl Into<String>) -> Self {
        Self::Message(s.into())
    }
}

/// One inventory entry in `bundle.json`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct BundleFileEntry {
    pub path: String,
    pub sha256: String,
    pub size: u64,
    pub mode: String,
}

/// Exact `bundle.json` receipt (BUNDLE-012 / schema v1).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct BundleReceipt {
    pub schema_version: u32,
    pub plugin_id: String,
    pub version: String,
    pub target: String,
    pub omarchy_contract: u32,
    pub minimum_quickshell_version: String,
    pub source_commit: String,
    pub files: Vec<BundleFileEntry>,
}

impl BundleReceipt {
    pub fn to_pretty_json(&self) -> Result<String, BundleError> {
        let mut json = serde_json::to_string_pretty(self)?;
        json.push('\n');
        Ok(json)
    }

    pub fn parse_json(bytes: &[u8]) -> Result<Self, BundleError> {
        let receipt: Self = serde_json::from_slice(bytes)?;
        receipt.validate_shape()?;
        Ok(receipt)
    }

    /// Structural checks independent of filesystem inventory.
    pub fn validate_shape(&self) -> Result<(), BundleError> {
        if self.schema_version != BUNDLE_SCHEMA_VERSION {
            return Err(BundleError::msg(format!(
                "bundle schemaVersion must be {BUNDLE_SCHEMA_VERSION}, got {}",
                self.schema_version
            )));
        }
        if self.plugin_id != PLUGIN_ID {
            return Err(BundleError::msg(format!(
                "pluginId must be {PLUGIN_ID}, got {}",
                self.plugin_id
            )));
        }
        if self.target != OFFICIAL_TARGET {
            return Err(BundleError::msg(format!(
                "unsupported target '{}'; only {OFFICIAL_TARGET}",
                self.target
            )));
        }
        if self.omarchy_contract != OMARCHY_CONTRACT {
            return Err(BundleError::msg(format!(
                "omarchyContract must be {OMARCHY_CONTRACT}, got {}",
                self.omarchy_contract
            )));
        }
        if self.minimum_quickshell_version != MINIMUM_QUICKSHELL_VERSION {
            return Err(BundleError::msg(format!(
                "minimumQuickshellVersion must be {MINIMUM_QUICKSHELL_VERSION}, got {}",
                self.minimum_quickshell_version
            )));
        }
        validate_source_commit(&self.source_commit)?;
        validate_semver_strict(&self.version)?;

        let mut seen = BTreeSet::new();
        let mut prev: Option<&str> = None;
        for entry in &self.files {
            validate_receipt_path(&entry.path)?;
            if entry.path == "bundle.json" {
                return Err(BundleError::msg(
                    "bundle.json must not appear in its own files inventory",
                ));
            }
            if !seen.insert(entry.path.clone()) {
                return Err(BundleError::msg(format!(
                    "duplicate receipt path: {}",
                    entry.path
                )));
            }
            if let Some(p) = prev {
                if entry.path.as_bytes() < p.as_bytes() {
                    return Err(BundleError::msg(
                        "receipt files must be sorted by raw UTF-8 path bytes",
                    ));
                }
            }
            prev = Some(entry.path.as_str());
            validate_sha256_hex(&entry.sha256)?;
            validate_mode_string(&entry.mode)?;
        }
        Ok(())
    }
}

/// Builds a complete `othavi0.agent-bar/` tree and writes `bundle.json`.
#[derive(Debug, Clone)]
pub struct BundleBuilder {
    pub version: String,
    pub target: String,
    pub source_commit: String,
    pub omarchy_contract: u32,
    pub minimum_quickshell_version: String,
}

impl BundleBuilder {
    pub fn new(
        version: impl Into<String>,
        source_commit: impl Into<String>,
    ) -> Result<Self, BundleError> {
        let version = version.into();
        let source_commit = source_commit.into();
        validate_semver_strict(&version)?;
        validate_source_commit(&source_commit)?;
        Ok(Self {
            version,
            target: OFFICIAL_TARGET.to_string(),
            source_commit,
            omarchy_contract: OMARCHY_CONTRACT,
            minimum_quickshell_version: MINIMUM_QUICKSHELL_VERSION.to_string(),
        })
    }

    /// Stamp release artifacts directly into the repo root and write
    /// `bundle.json`.
    ///
    /// The plugin QML/JS/manifest tree already lives at `repo_root` (no
    /// separate assembly step or version substitution). This copies
    /// `helper_bin` to `<repo_root>/bin/agent-bar` (mode 0755) and
    /// `<repo_root>/docs/media/demo.png` to `<repo_root>/preview.png`
    /// (mode 0644), builds the receipt from the shipped scope only
    /// ([`SHIPPED_ROOT_FILES`] / [`SHIPPED_DIRS`]), writes
    /// `<repo_root>/bundle.json`, and validates.
    pub fn stamp(&self, repo_root: &Path, helper_bin: &Path) -> Result<BundleReceipt, BundleError> {
        if !repo_root.is_dir() {
            return Err(BundleError::msg(format!(
                "repo root is not a directory: {}",
                repo_root.display()
            )));
        }
        if !helper_bin.is_file() {
            return Err(BundleError::msg(format!(
                "helper binary not found: {}",
                helper_bin.display()
            )));
        }
        let terminal = repo_root.join("scripts/agent-bar-open-terminal");
        if !terminal.is_file() {
            return Err(BundleError::msg(format!(
                "terminal helper not found: {}",
                terminal.display()
            )));
        }
        let readme = repo_root.join("README.md");
        if !readme.is_file() {
            return Err(BundleError::msg(format!(
                "README not found: {}",
                readme.display()
            )));
        }
        let license = repo_root.join("LICENSE");
        if !license.is_file() {
            return Err(BundleError::msg(format!(
                "LICENSE not found: {}",
                license.display()
            )));
        }
        let preview_src = repo_root.join("docs/media/demo.png");
        if !preview_src.is_file() {
            return Err(BundleError::msg(format!(
                "preview image not found: {}",
                preview_src.display()
            )));
        }

        // Private helper.
        let bin_dir = repo_root.join("bin");
        fs::create_dir_all(&bin_dir)?;
        let dest_helper = bin_dir.join("agent-bar");
        fs::copy(helper_bin, &dest_helper)?;
        set_unix_mode(&dest_helper, 0o755)?;

        // Marketplace preview image, stamped fresh every run.
        fs::copy(&preview_src, repo_root.join("preview.png"))?;
        set_unix_mode(&repo_root.join("preview.png"), 0o644)?;

        // Deterministic non-exec modes for ordinary shipped files.
        normalize_bundle_modes(repo_root)?;

        let receipt = BundleValidator::build_receipt(self, repo_root)?;
        let receipt_path = repo_root.join("bundle.json");
        let json = receipt.to_pretty_json()?;
        write_bytes_atomic(&receipt_path, json.as_bytes(), 0o644)?;

        // Final validation including the written receipt.
        BundleValidator::validate_tree(repo_root)?;
        Ok(receipt)
    }
}

/// Validates staged plugin trees and receipts against the closed contract.
pub struct BundleValidator;

impl BundleValidator {
    /// Walk the staged tree (excluding `bundle.json`) and produce a receipt.
    pub fn build_receipt(
        builder: &BundleBuilder,
        plugin_root: &Path,
    ) -> Result<BundleReceipt, BundleError> {
        let inventory = collect_inventory(plugin_root)?;
        let mut files: Vec<BundleFileEntry> = inventory
            .into_iter()
            .filter(|(rel, _, _, _)| rel != "bundle.json")
            .map(|(path, sha256, size, mode)| BundleFileEntry {
                path,
                sha256,
                size,
                mode,
            })
            .collect();
        files.sort_by(|a, b| a.path.as_bytes().cmp(b.path.as_bytes()));

        let receipt = BundleReceipt {
            schema_version: BUNDLE_SCHEMA_VERSION,
            plugin_id: PLUGIN_ID.to_string(),
            version: builder.version.clone(),
            target: builder.target.clone(),
            omarchy_contract: builder.omarchy_contract,
            minimum_quickshell_version: builder.minimum_quickshell_version.clone(),
            source_commit: builder.source_commit.clone(),
            files,
        };
        receipt.validate_shape()?;
        validate_manifest_matches(&plugin_root.join("manifest.json"), &builder.version)?;
        Ok(receipt)
    }

    /// Full validation: receipt shape, inventory equality, modes, links, versions.
    pub fn validate_tree(plugin_root: &Path) -> Result<BundleReceipt, BundleError> {
        if !plugin_root.is_dir() {
            return Err(BundleError::msg(format!(
                "plugin root is not a directory: {}",
                plugin_root.display()
            )));
        }
        // No symlinks / special files anywhere in the tree.
        reject_special_files(plugin_root)?;

        let receipt_path = plugin_root.join("bundle.json");
        let bytes = fs::read(&receipt_path).map_err(|e| {
            BundleError::msg(format!(
                "missing bundle.json at {}: {e}",
                receipt_path.display()
            ))
        })?;
        let receipt = BundleReceipt::parse_json(&bytes)?;

        let live = collect_inventory(plugin_root)?;
        let mut live_map: BTreeMap<String, (String, u64, String)> = BTreeMap::new();
        for (path, sha, size, mode) in live {
            if path == "bundle.json" {
                continue;
            }
            live_map.insert(path, (sha, size, mode));
        }

        let mut receipt_map: BTreeMap<String, &BundleFileEntry> = BTreeMap::new();
        for entry in &receipt.files {
            receipt_map.insert(entry.path.clone(), entry);
        }

        for path in live_map.keys() {
            if !receipt_map.contains_key(path) {
                return Err(BundleError::msg(format!(
                    "extra file not in receipt: {path}"
                )));
            }
        }
        for path in receipt_map.keys() {
            if !live_map.contains_key(path) {
                return Err(BundleError::msg(format!(
                    "receipt path missing on disk: {path}"
                )));
            }
        }

        for (path, entry) in &receipt_map {
            let (sha, size, mode) = live_map
                .get(path)
                .ok_or_else(|| BundleError::msg(format!("missing live entry for {path}")))?;
            if entry.sha256 != *sha {
                return Err(BundleError::msg(format!("checksum mismatch for {path}")));
            }
            if entry.size != *size {
                return Err(BundleError::msg(format!("size mismatch for {path}")));
            }
            if entry.mode != *mode {
                return Err(BundleError::msg(format!(
                    "mode mismatch for {path}: receipt {} live {mode}",
                    entry.mode
                )));
            }
        }

        validate_manifest_matches(&plugin_root.join("manifest.json"), &receipt.version)?;
        // Helper version must match manifest/receipt exactly (BUNDLE-006 / BUNDLE-027).
        let helper = plugin_root.join("bin/agent-bar");
        if !helper.is_file() {
            return Err(BundleError::msg("bin/agent-bar is missing"));
        }
        let mode = mode_string_of(&helper)?;
        if mode != EXEC_MODE {
            return Err(BundleError::msg(format!(
                "bin/agent-bar mode must be {EXEC_MODE}, got {mode}"
            )));
        }
        let helper_version = read_helper_version(&helper)?;
        if helper_version != receipt.version {
            return Err(BundleError::msg(format!(
                "helper version {helper_version} does not match receipt version {}",
                receipt.version
            )));
        }
        let term = plugin_root.join("scripts/agent-bar-open-terminal");
        if term.is_file() {
            let tm = mode_string_of(&term)?;
            if tm != EXEC_MODE {
                return Err(BundleError::msg(format!(
                    "scripts/agent-bar-open-terminal mode must be {EXEC_MODE}, got {tm}"
                )));
            }
        }

        // Required top-level files.
        for required in [
            "manifest.json",
            "Service.qml",
            "BarWidget.qml",
            "bin/agent-bar",
            "scripts/agent-bar-open-terminal",
            "README.md",
            "LICENSE",
            "preview.png",
        ] {
            if !plugin_root.join(required).is_file() {
                return Err(BundleError::msg(format!(
                    "required bundle file missing: {required}"
                )));
            }
        }

        Ok(receipt)
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

pub fn validate_source_commit(s: &str) -> Result<(), BundleError> {
    if s.len() != 40 {
        return Err(BundleError::msg(
            "sourceCommit must be exactly 40 lowercase hex characters",
        ));
    }
    if !s.chars().all(|c| matches!(c, '0'..='9' | 'a'..='f')) {
        return Err(BundleError::msg(
            "sourceCommit must be 40 lowercase hex characters",
        ));
    }
    Ok(())
}

pub fn validate_semver_strict(v: &str) -> Result<(), BundleError> {
    let parsed = semver::Version::parse(v)
        .map_err(|e| BundleError::msg(format!("invalid semantic version '{v}': {e}")))?;
    // Disallow leading zeros / weird forms by round-trip.
    if parsed.to_string() != v {
        return Err(BundleError::msg(format!(
            "version must be strict semver canonical form, got '{v}'"
        )));
    }
    Ok(())
}

pub fn validate_sha256_hex(s: &str) -> Result<(), BundleError> {
    if s.len() != 64 {
        return Err(BundleError::msg(
            "sha256 must be 64 lowercase hex characters",
        ));
    }
    if !s.chars().all(|c| matches!(c, '0'..='9' | 'a'..='f')) {
        return Err(BundleError::msg(
            "sha256 must be 64 lowercase hex characters",
        ));
    }
    Ok(())
}

fn validate_mode_string(mode: &str) -> Result<(), BundleError> {
    if mode.len() != 4 || !mode.chars().all(|c| matches!(c, '0'..='7')) {
        return Err(BundleError::msg(format!(
            "mode must be four-digit octal, got '{mode}'"
        )));
    }
    Ok(())
}

fn validate_receipt_path(rel: &str) -> Result<(), BundleError> {
    if rel.is_empty() {
        return Err(BundleError::msg("empty receipt path"));
    }
    if rel.contains('\\') {
        return Err(BundleError::msg(format!("backslash in path: {rel}")));
    }
    validate_archive_entry_path(rel).map_err(|e| BundleError::msg(e.to_string()))?;
    let path = Path::new(rel);
    for comp in path.components() {
        match comp {
            Component::Normal(s) => {
                let s = s.to_string_lossy();
                if s == "." || s == ".." || s.is_empty() {
                    return Err(BundleError::msg(format!("illegal path component in {rel}")));
                }
            }
            Component::CurDir | Component::ParentDir => {
                return Err(BundleError::msg(format!("dot component in path: {rel}")));
            }
            _ => {
                return Err(BundleError::msg(format!("illegal path: {rel}")));
            }
        }
    }
    Ok(())
}

fn validate_manifest_matches(manifest_path: &Path, version: &str) -> Result<(), BundleError> {
    let bytes = fs::read(manifest_path)?;
    let value: serde_json::Value = serde_json::from_slice(&bytes)?;
    let id = value
        .get("id")
        .and_then(|v| v.as_str())
        .ok_or_else(|| BundleError::msg("manifest missing id"))?;
    if id != PLUGIN_ID {
        return Err(BundleError::msg(format!(
            "manifest id must be {PLUGIN_ID}, got {id}"
        )));
    }
    let mv = value
        .get("version")
        .and_then(|v| v.as_str())
        .ok_or_else(|| BundleError::msg("manifest missing version"))?;
    if mv != version {
        return Err(BundleError::msg(format!(
            "manifest version {mv} does not equal receipt version {version}"
        )));
    }
    let schema = value
        .get("schemaVersion")
        .and_then(|v| v.as_u64())
        .ok_or_else(|| BundleError::msg("manifest missing schemaVersion"))?;
    if schema != 1 {
        return Err(BundleError::msg("manifest schemaVersion must be 1"));
    }
    Ok(())
}

/// Run `bin/agent-bar version` and return the trimmed first line (BUNDLE-006).
fn read_helper_version(helper: &Path) -> Result<String, BundleError> {
    let output = Command::new(helper)
        .arg("version")
        .output()
        .map_err(|e| BundleError::msg(format!("failed to execute helper version: {e}")))?;
    if !output.status.success() {
        return Err(BundleError::msg(format!(
            "helper version exited {}: {}",
            output.status.code().unwrap_or(-1),
            String::from_utf8_lossy(&output.stderr).trim()
        )));
    }
    let stdout = String::from_utf8_lossy(&output.stdout);
    let line = stdout.lines().next().unwrap_or("").trim().to_string();
    if line.is_empty() {
        return Err(BundleError::msg(
            "helper version produced empty stdout; expected exact semver",
        ));
    }
    Ok(line)
}

/// Walk a shipped directory (`components/` or `icons/`) recursively,
/// invoking `visit` for each regular file found. Rejects symlinks and other
/// special files anywhere in the walk.
fn walk_shipped_dir(
    root: &Path,
    dir_name: &str,
    mut visit: impl FnMut(&Path, &str) -> Result<(), BundleError>,
) -> Result<(), BundleError> {
    let dir = root.join(dir_name);
    if !dir.is_dir() {
        return Err(BundleError::msg(format!(
            "missing shipped directory: {dir_name}"
        )));
    }
    let mut stack = vec![dir];
    while let Some(d) = stack.pop() {
        for entry in fs::read_dir(&d)? {
            let entry = entry?;
            let path = entry.path();
            let meta = fs::symlink_metadata(&path)?;
            let ft = meta.file_type();
            if ft.is_symlink() {
                return Err(BundleError::msg(format!(
                    "symlink rejected in bundle: {}",
                    path.display()
                )));
            }
            if ft.is_dir() {
                stack.push(path);
            } else if ft.is_file() {
                let rel = path
                    .strip_prefix(root)
                    .map_err(|e| BundleError::msg(e.to_string()))?;
                let rel_s = rel
                    .components()
                    .map(|c| c.as_os_str().to_string_lossy())
                    .collect::<Vec<_>>()
                    .join("/");
                visit(&path, &rel_s)?;
            } else {
                return Err(BundleError::msg(format!(
                    "special file rejected in bundle: {}",
                    path.display()
                )));
            }
        }
    }
    Ok(())
}

fn normalize_bundle_modes(root: &Path) -> Result<(), BundleError> {
    for rel in SHIPPED_ROOT_FILES {
        let path = root.join(rel);
        // Presence check first, so an absent shipped file is refused by name
        // instead of surfacing as a bare OS error from the chmod below.
        fs::symlink_metadata(&path)
            .map_err(|e| BundleError::msg(format!("missing shipped file {rel}: {e}")))?;
        if *rel == "bin/agent-bar" || *rel == "scripts/agent-bar-open-terminal" {
            set_unix_mode(&path, 0o755)?;
        } else {
            set_unix_mode(&path, 0o644)?;
        }
    }
    for dir_name in SHIPPED_DIRS {
        walk_shipped_dir(root, dir_name, |path, _rel| set_unix_mode(path, 0o644))?;
    }
    Ok(())
}

/// True for a real directory named `.git` sitting directly under `root`.
///
/// Unlike omarchy-plugin-validate's `find "$PLUGIN_DIR" -name .git -prune`,
/// which skips a `.git` directory anywhere in the tree, this tolerance is
/// deliberately root-only: installed plugins are git checkouts, so a `.git`
/// at the tree root is expected and never part of the plugin contract, but a
/// `.git` nested deeper is unexpected and still trips the ordinary
/// extra-file-not-in-receipt check below. A `.git` that is itself a symlink
/// at the root gets no pass either -- the symlink check always runs first.
fn is_root_git_dir(root: &Path, dir: &Path, name: &OsStr, is_dir: bool) -> bool {
    is_dir && dir == root && name == OsStr::new(".git")
}

/// Collect (relative path with `/`, sha256, size, mode) for every shipped
/// regular file: [`SHIPPED_ROOT_FILES`] plus a full walk of [`SHIPPED_DIRS`].
/// The repo root also holds `src/`, `docs/`, `target/`, and other non-shipped
/// trees; those are outside this inventory's scope entirely, not merely
/// tolerated.
fn collect_inventory(root: &Path) -> Result<Vec<(String, String, u64, String)>, BundleError> {
    let mut out = Vec::new();
    for rel in SHIPPED_ROOT_FILES {
        let path = root.join(rel);
        let meta = fs::symlink_metadata(&path)
            .map_err(|e| BundleError::msg(format!("missing shipped file {rel}: {e}")))?;
        let ft = meta.file_type();
        if ft.is_symlink() {
            return Err(BundleError::msg(format!(
                "symlink rejected in bundle: {}",
                path.display()
            )));
        }
        if !ft.is_file() {
            return Err(BundleError::msg(format!(
                "special file rejected in bundle: {}",
                path.display()
            )));
        }
        validate_receipt_path(rel)?;
        let bytes = fs::read(&path)?;
        let sha = hash_bytes(&bytes);
        let mode = mode_string_of(&path)?;
        out.push((rel.to_string(), sha, bytes.len() as u64, mode));
    }
    for dir_name in SHIPPED_DIRS {
        walk_shipped_dir(root, dir_name, |path, rel_s| {
            validate_receipt_path(rel_s)?;
            let bytes = fs::read(path)?;
            let sha = hash_bytes(&bytes);
            let mode = mode_string_of(path)?;
            out.push((rel_s.to_string(), sha, bytes.len() as u64, mode));
            Ok(())
        })?;
    }
    out.sort_by(|a, b| a.0.as_bytes().cmp(b.0.as_bytes()));
    Ok(out)
}

/// Reject symlinks and other special files within the shipped scope, plus
/// the repo root's own immediate entries (excluding a root `.git`). This
/// deliberately does not descend into non-shipped subdirectories like
/// `src/` or `docs/` -- a symlink there is the shell's own full-tree
/// validator's job at install time, not this bundle's.
fn reject_special_files(root: &Path) -> Result<(), BundleError> {
    for entry in fs::read_dir(root)? {
        let entry = entry?;
        let path = entry.path();
        let meta = fs::symlink_metadata(&path)?;
        let ft = meta.file_type();
        if is_root_git_dir(root, root, &entry.file_name(), ft.is_dir()) {
            continue;
        }
        if ft.is_symlink() {
            return Err(BundleError::msg(format!(
                "symlink rejected: {}",
                path.display()
            )));
        }
        if !ft.is_dir() && !ft.is_file() {
            return Err(BundleError::msg(format!(
                "special file rejected: {}",
                path.display()
            )));
        }
    }

    for rel in SHIPPED_ROOT_FILES {
        let path = root.join(rel);
        let meta = fs::symlink_metadata(&path)?;
        let ft = meta.file_type();
        if ft.is_symlink() {
            return Err(BundleError::msg(format!(
                "symlink rejected: {}",
                path.display()
            )));
        }
        if !ft.is_file() {
            return Err(BundleError::msg(format!(
                "special file rejected: {}",
                path.display()
            )));
        }
    }

    for dir_name in SHIPPED_DIRS {
        walk_shipped_dir(root, dir_name, |_path, _rel| Ok(()))?;
    }
    Ok(())
}

fn mode_string_of(path: &Path) -> Result<String, BundleError> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let meta = fs::metadata(path)?;
        let mode = meta.permissions().mode() & 0o7777;
        Ok(format!("{mode:04o}"))
    }
    #[cfg(not(unix))]
    {
        let _ = path;
        Ok("0644".to_string())
    }
}

fn set_unix_mode(path: &Path, mode: u32) -> Result<(), BundleError> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let perms = fs::Permissions::from_mode(mode);
        fs::set_permissions(path, perms)?;
    }
    #[cfg(not(unix))]
    {
        let _ = (path, mode);
    }
    Ok(())
}

fn write_bytes_atomic(path: &Path, bytes: &[u8], mode: u32) -> Result<(), BundleError> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let tmp = path.with_extension("tmp");
    {
        let mut opts = OpenOptions::new();
        opts.write(true).create(true).truncate(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            opts.mode(mode);
        }
        let mut f = opts.open(&tmp)?;
        f.write_all(bytes)?;
        f.sync_all()?;
    }
    set_unix_mode(&tmp, mode)?;
    fs::rename(&tmp, path)?;
    if let Some(parent) = path.parent() {
        if let Ok(dir) = File::open(parent) {
            let _ = dir.sync_all();
        }
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::PermissionsExt;
    use tempfile::tempdir;

    const ZERO_COMMIT: &str = "0000000000000000000000000000000000000000";

    fn sample_receipt() -> BundleReceipt {
        BundleReceipt {
            schema_version: 1,
            plugin_id: PLUGIN_ID.to_string(),
            version: "10.0.0".to_string(),
            target: OFFICIAL_TARGET.to_string(),
            omarchy_contract: 1,
            minimum_quickshell_version: MINIMUM_QUICKSHELL_VERSION.to_string(),
            source_commit: "0123456789abcdef0123456789abcdef01234567".to_string(),
            files: vec![BundleFileEntry {
                path: "BarWidget.qml".to_string(),
                sha256: "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef"
                    .to_string(),
                size: 1234,
                mode: "0644".to_string(),
            }],
        }
    }

    #[test]
    fn receipt_literal_shape_round_trip() {
        let r = sample_receipt();
        let json = r.to_pretty_json().expect("json");
        let parsed = BundleReceipt::parse_json(json.as_bytes()).expect("parse");
        assert_eq!(parsed, r);
        assert!(json.contains("\"schemaVersion\": 1"));
        assert!(json.contains("\"pluginId\": \"othavi0.agent-bar\""));
        assert!(json.contains("\"omarchyContract\": 1"));
        assert!(json.contains("\"minimumQuickshellVersion\": \"0.3.0\""));
    }

    #[test]
    fn receipt_rejects_unknown_fields() {
        let mut v = serde_json::to_value(sample_receipt()).unwrap();
        v.as_object_mut()
            .unwrap()
            .insert("extra".into(), serde_json::json!(true));
        let bytes = serde_json::to_vec(&v).unwrap();
        assert!(BundleReceipt::parse_json(&bytes).is_err());
    }

    #[test]
    fn receipt_rejects_wrong_target_and_commit() {
        let mut r = sample_receipt();
        r.target = "aarch64-unknown-linux-gnu".into();
        assert!(r.validate_shape().is_err());
        r = sample_receipt();
        r.source_commit = "ABC".into();
        assert!(r.validate_shape().is_err());
        r = sample_receipt();
        r.source_commit = "0123456789ABCDEF0123456789abcdef01234567".into();
        assert!(r.validate_shape().is_err());
    }

    #[test]
    fn receipt_rejects_duplicate_and_unsorted_paths() {
        let mut r = sample_receipt();
        r.files.push(BundleFileEntry {
            path: "BarWidget.qml".into(),
            sha256: r.files[0].sha256.clone(),
            size: 1,
            mode: "0644".into(),
        });
        assert!(r.validate_shape().is_err());

        r = sample_receipt();
        r.files = vec![
            BundleFileEntry {
                path: "z.qml".into(),
                sha256: "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef".into(),
                size: 1,
                mode: "0644".into(),
            },
            BundleFileEntry {
                path: "a.qml".into(),
                sha256: "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef".into(),
                size: 1,
                mode: "0644".into(),
            },
        ];
        assert!(r.validate_shape().is_err());
    }

    #[test]
    fn receipt_rejects_traversal_paths() {
        let mut r = sample_receipt();
        r.files[0].path = "../evil".into();
        assert!(r.validate_shape().is_err());
        assert!(validate_receipt_path("..").is_err());
        assert!(validate_receipt_path("").is_err());
        assert!(validate_receipt_path("bundle.json").is_ok()); // path ok; shape forbids in files
        assert!(validate_receipt_path("../evil").is_err());
    }

    /// A fully-stamped shipped root: every `SHIPPED_ROOT_FILES` entry plus
    /// non-empty `SHIPPED_DIRS`, ready for `BundleValidator` calls directly
    /// (as opposed to `write_stamp_source_root`, which is missing the two
    /// artifacts `stamp` itself creates).
    fn write_minimal_plugin(root: &Path, version: &str) {
        fs::create_dir_all(root.join("bin")).unwrap();
        fs::create_dir_all(root.join("scripts")).unwrap();
        fs::create_dir_all(root.join("icons")).unwrap();
        fs::create_dir_all(root.join("components")).unwrap();
        fs::write(
            root.join("manifest.json"),
            format!(
                r#"{{
  "schemaVersion": 1,
  "id": "othavi0.agent-bar",
  "name": "Agent Bar",
  "version": "{version}",
  "author": "othavi0",
  "license": "MIT",
  "description": "LLM quota monitor for Claude, Codex, Amp, and Grok.",
  "kinds": ["service", "bar-widget"],
  "entryPoints": {{
    "service": "Service.qml",
    "barWidget": "BarWidget.qml"
  }},
  "barWidget": {{
    "displayName": "Agent Bar",
    "description": "Shows normalized provider quota and reset information.",
    "category": "AI",
    "aliases": ["agent-bar"],
    "allowMultiple": false,
    "defaults": {{}},
    "schema": []
  }}
}}
"#
            ),
        )
        .unwrap();
        fs::set_permissions(
            root.join("manifest.json"),
            fs::Permissions::from_mode(0o644),
        )
        .unwrap();
        for name in [
            "Service.qml",
            "BarWidget.qml",
            "CoreMaintenance.js",
            "CoreScroll.js",
            "CoreService.js",
            "CoreSettings.js",
            "CoreView.js",
            "MaintenanceView.qml",
            "Popup.qml",
            "ProviderRail.qml",
            "ProviderView.qml",
            "SettingsView.qml",
        ] {
            fs::write(root.join(name), format!("// {name}\n")).unwrap();
            fs::set_permissions(root.join(name), fs::Permissions::from_mode(0o644)).unwrap();
        }
        // Fake helper that answers `version` with the staged receipt version.
        fs::write(
            root.join("bin/agent-bar"),
            format!(
                "#!/bin/sh\nif [ \"$1\" = version ] || [ \"$1\" = --version ]; then echo {version}; fi\n"
            ),
        )
        .unwrap();
        fs::set_permissions(
            root.join("bin/agent-bar"),
            fs::Permissions::from_mode(0o755),
        )
        .unwrap();
        fs::write(
            root.join("scripts/agent-bar-open-terminal"),
            b"#!/bin/bash\n",
        )
        .unwrap();
        fs::set_permissions(
            root.join("scripts/agent-bar-open-terminal"),
            fs::Permissions::from_mode(0o755),
        )
        .unwrap();
        fs::write(root.join("icons/claude.png"), b"png").unwrap();
        fs::set_permissions(
            root.join("icons/claude.png"),
            fs::Permissions::from_mode(0o644),
        )
        .unwrap();
        fs::write(root.join("components/Sample.qml"), b"// sample\n").unwrap();
        fs::set_permissions(
            root.join("components/Sample.qml"),
            fs::Permissions::from_mode(0o644),
        )
        .unwrap();
        for (name, bytes) in [
            ("README.md", &b"# Agent Bar\n"[..]),
            ("LICENSE", &b"MIT\n"[..]),
            ("preview.png", &b"png"[..]),
        ] {
            fs::write(root.join(name), bytes).unwrap();
            fs::set_permissions(root.join(name), fs::Permissions::from_mode(0o644)).unwrap();
        }
    }

    /// A stamp-input root: everything `stamp` expects to already exist,
    /// minus the two artifacts it creates itself (`bin/agent-bar`,
    /// `preview.png`), plus `docs/media/demo.png` as the preview source.
    fn write_stamp_source_root(root: &Path, version: &str) {
        write_minimal_plugin(root, version);
        fs::remove_file(root.join("bin/agent-bar")).unwrap();
        fs::remove_dir(root.join("bin")).unwrap();
        fs::remove_file(root.join("preview.png")).unwrap();
        fs::create_dir_all(root.join("docs/media")).unwrap();
        fs::write(root.join("docs/media/demo.png"), b"not a real png\n").unwrap();
    }

    #[test]
    fn build_receipt_and_validate_tree() {
        let dir = tempdir().unwrap();
        let root = dir.path().join("othavi0.agent-bar");
        write_minimal_plugin(&root, "10.0.0");
        let builder = BundleBuilder::new("10.0.0", ZERO_COMMIT).unwrap();
        let receipt = BundleValidator::build_receipt(&builder, &root).unwrap();
        let json = receipt.to_pretty_json().unwrap();
        fs::write(root.join("bundle.json"), json.as_bytes()).unwrap();
        fs::set_permissions(root.join("bundle.json"), fs::Permissions::from_mode(0o644)).unwrap();
        let validated = BundleValidator::validate_tree(&root).unwrap();
        assert_eq!(validated.version, "10.0.0");
        assert_eq!(validated.target, OFFICIAL_TARGET);
        assert!(validated.files.iter().any(|f| f.path == "bin/agent-bar"));
        assert!(!validated.files.iter().any(|f| f.path == "bundle.json"));
        // Sorted by raw bytes.
        let paths: Vec<_> = validated.files.iter().map(|f| f.path.as_str()).collect();
        let mut sorted = paths.clone();
        sorted.sort_by(|a, b| a.as_bytes().cmp(b.as_bytes()));
        assert_eq!(paths, sorted);
    }

    #[test]
    fn validate_detects_version_mismatch_and_extra_file() {
        let dir = tempdir().unwrap();
        let root = dir.path().join("othavi0.agent-bar");
        write_minimal_plugin(&root, "10.0.0");
        let builder = BundleBuilder::new("10.0.0", ZERO_COMMIT).unwrap();
        let receipt = BundleValidator::build_receipt(&builder, &root).unwrap();
        fs::write(root.join("bundle.json"), receipt.to_pretty_json().unwrap()).unwrap();

        // Extra file after receipt was built, inside the shipped scope
        // (`components/`) -- outside it, e.g. loose at the repo root, is
        // deliberately not this validator's business.
        fs::write(root.join("components/evil.txt"), b"x").unwrap();
        assert!(BundleValidator::validate_tree(&root).is_err());

        // Version mismatch via receipt mutation.
        fs::remove_file(root.join("components/evil.txt")).unwrap();
        let mut bad = receipt.clone();
        bad.version = "10.0.1".into();
        fs::write(root.join("bundle.json"), bad.to_pretty_json().unwrap()).unwrap();
        assert!(BundleValidator::validate_tree(&root).is_err());
    }

    #[test]
    fn validate_detects_checksum_and_mode_mismatch() {
        let dir = tempdir().unwrap();
        let root = dir.path().join("othavi0.agent-bar");
        write_minimal_plugin(&root, "10.0.0");
        let builder = BundleBuilder::new("10.0.0", ZERO_COMMIT).unwrap();
        let mut receipt = BundleValidator::build_receipt(&builder, &root).unwrap();
        // Corrupt a checksum.
        receipt.files[0].sha256 =
            "ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff".into();
        fs::write(root.join("bundle.json"), receipt.to_pretty_json().unwrap()).unwrap();
        assert!(BundleValidator::validate_tree(&root).is_err());

        // Restore and break mode.
        let receipt = BundleValidator::build_receipt(&builder, &root).unwrap();
        fs::write(root.join("bundle.json"), receipt.to_pretty_json().unwrap()).unwrap();
        fs::set_permissions(root.join("Service.qml"), fs::Permissions::from_mode(0o600)).unwrap();
        assert!(BundleValidator::validate_tree(&root).is_err());
    }

    #[test]
    fn validate_rejects_symlink_in_tree() {
        let dir = tempdir().unwrap();
        let root = dir.path().join("othavi0.agent-bar");
        write_minimal_plugin(&root, "10.0.0");
        // Inside the shipped scope (`components/`) -- a symlink loose at the
        // repo root is outside collect_inventory's business now.
        std::os::unix::fs::symlink("/etc/passwd", root.join("components/link")).unwrap();
        let builder = BundleBuilder::new("10.0.0", ZERO_COMMIT).unwrap();
        // build_receipt itself walks and rejects symlinks.
        assert!(BundleValidator::build_receipt(&builder, &root).is_err());
    }

    #[test]
    fn stamp_from_source_root() {
        let dir = tempdir().unwrap();
        let root = dir.path().join("othavi0.agent-bar");
        let version = "10.0.0";
        write_stamp_source_root(&root, version);

        let helper = dir.path().join("agent-bar");
        // Stand-in helper that reports the package version for BUNDLE-006.
        fs::write(
            &helper,
            format!(
                "#!/bin/sh\nif [ \"$1\" = version ] || [ \"$1\" = --version ]; then echo {version}; fi\n"
            ),
        )
        .unwrap();
        fs::set_permissions(&helper, fs::Permissions::from_mode(0o755)).unwrap();

        let builder = BundleBuilder::new(version, ZERO_COMMIT).unwrap();
        let receipt = builder.stamp(&root, &helper).unwrap();
        assert_eq!(receipt.version, version);
        assert_eq!(receipt.plugin_id, PLUGIN_ID);
        // stamp creates these two artifacts fresh.
        assert!(root.join("bin/agent-bar").is_file());
        assert!(root.join("preview.png").is_file());
        assert!(root.join("bundle.json").is_file());
        assert!(receipt.files.iter().any(|f| f.path == "bin/agent-bar"));
        assert!(receipt.files.iter().any(|f| f.path == "preview.png"));
        // Non-shipped source material (the preview's own origin file) stays
        // out of the receipt.
        assert!(!receipt
            .files
            .iter()
            .any(|f| f.path == "docs/media/demo.png"));
        BundleValidator::validate_tree(&root).unwrap();
    }

    #[test]
    fn stamp_refuses_missing_shipped_file_or_dir() {
        let dir = tempdir().unwrap();
        let root = dir.path().join("othavi0.agent-bar");
        let version = "10.0.0";
        write_stamp_source_root(&root, version);

        let helper = dir.path().join("agent-bar");
        fs::write(
            &helper,
            format!(
                "#!/bin/sh\nif [ \"$1\" = version ] || [ \"$1\" = --version ]; then echo {version}; fi\n"
            ),
        )
        .unwrap();
        fs::set_permissions(&helper, fs::Permissions::from_mode(0o755)).unwrap();
        let builder = BundleBuilder::new(version, ZERO_COMMIT).unwrap();

        // A missing SHIPPED_ROOT_FILES entry refuses the stamp, naming the file.
        fs::remove_file(root.join("CoreView.js")).unwrap();
        let err = builder.stamp(&root, &helper).unwrap_err().to_string();
        assert!(
            err.contains("missing shipped file CoreView.js"),
            "unexpected error: {err}"
        );

        // A missing SHIPPED_DIRS entry refuses the stamp, naming the directory.
        fs::write(root.join("CoreView.js"), b"// core\n").unwrap();
        fs::set_permissions(root.join("CoreView.js"), fs::Permissions::from_mode(0o644)).unwrap();
        fs::remove_dir_all(root.join("icons")).unwrap();
        let err = builder.stamp(&root, &helper).unwrap_err().to_string();
        assert!(
            err.contains("missing shipped directory: icons"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn validate_tree_rejects_helper_version_mismatch() {
        let dir = tempdir().unwrap();
        let root = dir.path().join("othavi0.agent-bar");
        write_minimal_plugin(&root, "10.0.0");
        // Helper reports a different version than the receipt/manifest.
        fs::write(
            root.join("bin/agent-bar"),
            "#!/bin/sh\nif [ \"$1\" = version ] || [ \"$1\" = --version ]; then echo 9.9.9; fi\n",
        )
        .unwrap();
        fs::set_permissions(
            root.join("bin/agent-bar"),
            fs::Permissions::from_mode(0o755),
        )
        .unwrap();
        let builder = BundleBuilder::new("10.0.0", ZERO_COMMIT).unwrap();
        let receipt = BundleValidator::build_receipt(&builder, &root).unwrap();
        fs::write(root.join("bundle.json"), receipt.to_pretty_json().unwrap()).unwrap();
        let err = BundleValidator::validate_tree(&root).unwrap_err();
        assert!(
            err.to_string().contains("helper version"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn source_commit_and_semver_gates() {
        assert!(validate_source_commit(ZERO_COMMIT).is_ok());
        assert!(validate_source_commit("abc").is_err());
        assert!(validate_semver_strict("10.0.0").is_ok());
        assert!(validate_semver_strict("v10.0.0").is_err());
        assert!(BundleBuilder::new("nope", ZERO_COMMIT).is_err());
    }
}
