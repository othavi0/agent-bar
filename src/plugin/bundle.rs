//! Plugin bundle assembly, receipt (`bundle.json`), and tree validation.

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
/// Manifest template placeholder replaced at assemble time.
pub const MANIFEST_VERSION_PLACEHOLDER: &str = "__AGENT_BAR_VERSION__";

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

/// Builds a complete `agent-bar.usage/` tree and writes `bundle.json`.
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

    /// Assemble plugin tree under `output` from repo-relative sources.
    ///
    /// `repo_root` must contain `assets/omarchy`, `scripts/agent-bar-open-terminal`,
    /// and a built helper at `helper_bin`.
    pub fn assemble(
        &self,
        output: &Path,
        repo_root: &Path,
        helper_bin: &Path,
    ) -> Result<BundleReceipt, BundleError> {
        if output.exists() {
            return Err(BundleError::msg(format!(
                "assemble output already exists: {}",
                output.display()
            )));
        }
        let assets = repo_root.join("assets/omarchy");
        if !assets.is_dir() {
            return Err(BundleError::msg(format!(
                "missing assets/omarchy under {}",
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
        let readme_src = repo_root.join("assets/dist/README.md");
        if !readme_src.is_file() {
            return Err(BundleError::msg(format!(
                "dist README not found: {}",
                readme_src.display()
            )));
        }
        let license_src = repo_root.join("LICENSE");
        if !license_src.is_file() {
            return Err(BundleError::msg(format!(
                "LICENSE not found: {}",
                license_src.display()
            )));
        }
        let preview_src = repo_root.join("docs/media/demo.png");
        if !preview_src.is_file() {
            return Err(BundleError::msg(format!(
                "preview image not found: {}",
                preview_src.display()
            )));
        }

        fs::create_dir_all(output)?;
        // Top-level QML / JS / manifest (version substituted).
        copy_asset_tree(&assets, output, &self.version)?;

        // Private helper.
        let bin_dir = output.join("bin");
        fs::create_dir_all(&bin_dir)?;
        let dest_helper = bin_dir.join("agent-bar");
        fs::copy(helper_bin, &dest_helper)?;
        set_unix_mode(&dest_helper, 0o755)?;

        // Terminal helper.
        let scripts = output.join("scripts");
        fs::create_dir_all(&scripts)?;
        let dest_term = scripts.join("agent-bar-open-terminal");
        fs::copy(&terminal, &dest_term)?;
        set_unix_mode(&dest_term, 0o755)?;

        // Distribution repo metadata: the assembled tree is the complete
        // repo state pushed to the git plugin distribution repo, so it
        // carries its own README, LICENSE, and marketplace preview image.
        fs::copy(&readme_src, output.join("README.md"))?;
        set_unix_mode(&output.join("README.md"), 0o644)?;
        fs::copy(&license_src, output.join("LICENSE"))?;
        set_unix_mode(&output.join("LICENSE"), 0o644)?;
        fs::copy(&preview_src, output.join("preview.png"))?;
        set_unix_mode(&output.join("preview.png"), 0o644)?;

        // Deterministic non-exec modes for ordinary files.
        normalize_bundle_modes(output)?;

        let receipt = BundleValidator::build_receipt(self, output)?;
        let receipt_path = output.join("bundle.json");
        let json = receipt.to_pretty_json()?;
        write_bytes_atomic(&receipt_path, json.as_bytes(), 0o644)?;

        // Final validation including the written receipt.
        BundleValidator::validate_tree(output)?;
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

fn copy_asset_tree(assets: &Path, output: &Path, version: &str) -> Result<(), BundleError> {
    fn walk(src: &Path, dst: &Path, version: &str) -> Result<(), BundleError> {
        fs::create_dir_all(dst)?;
        for entry in fs::read_dir(src)? {
            let entry = entry?;
            let ft = entry.file_type()?;
            let name = entry.file_name();
            let name_str = name.to_string_lossy();
            // Skip development-only files that must never ship.
            if name_str == "Widget.qml" {
                // v9 leftover; not part of the v10 bundle tree.
                continue;
            }
            let from = entry.path();
            let to = dst.join(&name);
            if ft.is_dir() {
                walk(&from, &to, version)?;
            } else if ft.is_file() {
                if name_str == "manifest.json" {
                    let raw = fs::read_to_string(&from)?;
                    let substituted = raw.replace(MANIFEST_VERSION_PLACEHOLDER, version);
                    if substituted.contains(MANIFEST_VERSION_PLACEHOLDER) {
                        return Err(BundleError::msg(
                            "manifest still contains version placeholder after substitution",
                        ));
                    }
                    write_bytes_atomic(&to, substituted.as_bytes(), 0o644)?;
                } else {
                    fs::copy(&from, &to)?;
                    set_unix_mode(&to, 0o644)?;
                }
            } else {
                return Err(BundleError::msg(format!(
                    "refusing to copy special file {}",
                    from.display()
                )));
            }
        }
        Ok(())
    }
    walk(assets, output, version)
}

fn normalize_bundle_modes(root: &Path) -> Result<(), BundleError> {
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        for entry in fs::read_dir(&dir)? {
            let entry = entry?;
            let path = entry.path();
            let ft = entry.file_type()?;
            if ft.is_dir() {
                stack.push(path);
            } else if ft.is_file() {
                let rel = path
                    .strip_prefix(root)
                    .map_err(|e| BundleError::msg(e.to_string()))?;
                let rel_s = rel.to_string_lossy().replace('\\', "/");
                if rel_s == "bin/agent-bar" || rel_s == "scripts/agent-bar-open-terminal" {
                    set_unix_mode(&path, 0o755)?;
                } else {
                    set_unix_mode(&path, 0o644)?;
                }
            }
        }
    }
    Ok(())
}

/// True for a real directory named `.git` sitting directly under `root`.
///
/// Mirrors omarchy-plugin-validate's `find "$PLUGIN_DIR" -name .git -prune`:
/// installed plugins are git checkouts, and git's own bookkeeping directory
/// at the tree root is never part of the plugin contract. A `.git` deeper in
/// the tree, or one that is itself a symlink, gets no such pass.
fn is_root_git_dir(root: &Path, dir: &Path, name: &OsStr, is_dir: bool) -> bool {
    is_dir && dir == root && name == OsStr::new(".git")
}

/// Collect (relative path with `/`, sha256, size, mode) for every regular file.
fn collect_inventory(root: &Path) -> Result<Vec<(String, String, u64, String)>, BundleError> {
    let mut out = Vec::new();
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        let mut entries: Vec<_> = fs::read_dir(&dir)?.collect::<Result<Vec<_>, _>>()?;
        entries.sort_by_key(|e| e.file_name());
        for entry in entries {
            let path = entry.path();
            let ft = entry.file_type()?;
            if ft.is_symlink() {
                return Err(BundleError::msg(format!(
                    "symlink rejected in bundle: {}",
                    path.display()
                )));
            }
            if is_root_git_dir(root, &dir, &entry.file_name(), ft.is_dir()) {
                continue;
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
                validate_receipt_path(&rel_s)?;
                let bytes = fs::read(&path)?;
                let sha = hash_bytes(&bytes);
                let mode = mode_string_of(&path)?;
                out.push((rel_s, sha, bytes.len() as u64, mode));
            } else {
                return Err(BundleError::msg(format!(
                    "special file rejected in bundle: {}",
                    path.display()
                )));
            }
        }
    }
    out.sort_by(|a, b| a.0.as_bytes().cmp(b.0.as_bytes()));
    Ok(out)
}

fn reject_special_files(root: &Path) -> Result<(), BundleError> {
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        for entry in fs::read_dir(&dir)? {
            let entry = entry?;
            let path = entry.path();
            let meta = fs::symlink_metadata(&path)?;
            let ft = meta.file_type();
            if ft.is_symlink() {
                return Err(BundleError::msg(format!(
                    "symlink rejected: {}",
                    path.display()
                )));
            }
            if is_root_git_dir(root, &dir, &entry.file_name(), ft.is_dir()) {
                continue;
            }
            if ft.is_dir() {
                stack.push(path);
            } else if !ft.is_file() {
                return Err(BundleError::msg(format!(
                    "special file rejected: {}",
                    path.display()
                )));
            }
        }
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
    use std::path::PathBuf;
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
        assert!(json.contains("\"pluginId\": \"agent-bar.usage\""));
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

    fn write_minimal_plugin(root: &Path, version: &str) {
        fs::create_dir_all(root.join("bin")).unwrap();
        fs::create_dir_all(root.join("scripts")).unwrap();
        fs::create_dir_all(root.join("icons")).unwrap();
        fs::write(
            root.join("manifest.json"),
            format!(
                r#"{{
  "schemaVersion": 1,
  "id": "agent-bar.usage",
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
        fs::write(root.join("Service.qml"), b"// service\n").unwrap();
        fs::write(root.join("BarWidget.qml"), b"// bar\n").unwrap();
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
        fs::write(root.join("README.md"), b"# Agent Bar\n").unwrap();
        fs::write(root.join("LICENSE"), b"MIT\n").unwrap();
        fs::write(root.join("preview.png"), b"png").unwrap();
        for p in [
            "Service.qml",
            "BarWidget.qml",
            "manifest.json",
            "icons/claude.png",
            "README.md",
            "LICENSE",
            "preview.png",
        ] {
            fs::set_permissions(root.join(p), fs::Permissions::from_mode(0o644)).unwrap();
        }
    }

    #[test]
    fn build_receipt_and_validate_tree() {
        let dir = tempdir().unwrap();
        let root = dir.path().join("agent-bar.usage");
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
        let root = dir.path().join("agent-bar.usage");
        write_minimal_plugin(&root, "10.0.0");
        let builder = BundleBuilder::new("10.0.0", ZERO_COMMIT).unwrap();
        let receipt = BundleValidator::build_receipt(&builder, &root).unwrap();
        fs::write(root.join("bundle.json"), receipt.to_pretty_json().unwrap()).unwrap();

        // Extra file after receipt was built.
        fs::write(root.join("evil.txt"), b"x").unwrap();
        assert!(BundleValidator::validate_tree(&root).is_err());

        // Version mismatch via receipt mutation.
        fs::remove_file(root.join("evil.txt")).unwrap();
        let mut bad = receipt.clone();
        bad.version = "10.0.1".into();
        fs::write(root.join("bundle.json"), bad.to_pretty_json().unwrap()).unwrap();
        assert!(BundleValidator::validate_tree(&root).is_err());
    }

    #[test]
    fn validate_detects_checksum_and_mode_mismatch() {
        let dir = tempdir().unwrap();
        let root = dir.path().join("agent-bar.usage");
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
        let root = dir.path().join("agent-bar.usage");
        write_minimal_plugin(&root, "10.0.0");
        std::os::unix::fs::symlink("/etc/passwd", root.join("link")).unwrap();
        let builder = BundleBuilder::new("10.0.0", ZERO_COMMIT).unwrap();
        // build_receipt itself walks and rejects symlinks.
        assert!(BundleValidator::build_receipt(&builder, &root).is_err());
    }

    #[test]
    fn assemble_from_repo_assets() {
        let repo = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let dir = tempdir().unwrap();
        let helper = dir.path().join("agent-bar");
        let version = env!("CARGO_PKG_VERSION");
        // Stand-in helper that reports the package version for BUNDLE-006.
        fs::write(
            &helper,
            format!(
                "#!/bin/sh\nif [ \"$1\" = version ] || [ \"$1\" = --version ]; then echo {version}; fi\n"
            ),
        )
        .unwrap();
        fs::set_permissions(&helper, fs::Permissions::from_mode(0o755)).unwrap();

        let out = dir.path().join("agent-bar.usage");
        let builder = BundleBuilder::new(version, ZERO_COMMIT).unwrap();
        let receipt = builder.assemble(&out, &repo, &helper).unwrap();
        assert_eq!(receipt.version, version);
        assert_eq!(receipt.plugin_id, PLUGIN_ID);
        assert!(out.join("Service.qml").is_file());
        assert!(out.join("icons/claude.png").is_file());
        assert!(out.join("icons/amp.svg").is_file());
        assert!(out.join("components/ProviderChip.qml").is_file());
        assert!(!out.join("Widget.qml").exists());
        assert!(out.join("bundle.json").is_file());
        // No global executable at install root.
        assert!(!out.join("agent-bar").exists());
        BundleValidator::validate_tree(&out).unwrap();
    }

    #[test]
    fn validate_tree_rejects_helper_version_mismatch() {
        let dir = tempdir().unwrap();
        let root = dir.path().join("agent-bar.usage");
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
