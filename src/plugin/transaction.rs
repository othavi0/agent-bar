//! Same-filesystem plugin transactions with journal and exclusive gate (MIG-001..006).

use std::fs::{self, File, OpenOptions};
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::plugin::ownership::{capture_evidence, OwnershipClass, OwnershipEvidence};
use crate::plugin::paths::{
    ensure_not_symlink, same_filesystem, validate_archive_entry_path, validate_txid, PathError,
    PluginPaths, PLUGIN_ID,
};
use crate::support::{ExclusiveMaintenanceGuard, MaintenanceGate};

#[derive(Debug, Error)]
pub enum TransactionError {
    #[error("{0}")]
    Message(String),
    #[error(transparent)]
    Path(#[from] PathError),
    #[error(transparent)]
    Io(#[from] io::Error),
}

impl TransactionError {
    pub fn msg(s: impl Into<String>) -> Self {
        Self::Message(s.into())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TxStep {
    Preflight,
    OwnershipScan,
    Backup,
    Stage,
    ValidateStaged,
    Exchange,
    Rescan,
    Health,
    Commit,
    Rollback,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TxFailPoint {
    AfterBackup,
    AfterStage,
    AfterValidate,
    AtExchange,
    AfterExchange,
    AtRescan,
    AtHealth,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct JournalEntry {
    pub step: TxStep,
    pub detail: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TransactionJournal {
    pub txid: String,
    pub operation: String,
    pub completed: Vec<TxStep>,
    pub entries: Vec<JournalEntry>,
}

impl TransactionJournal {
    pub fn new(txid: impl Into<String>, operation: impl Into<String>) -> Self {
        Self {
            txid: txid.into(),
            operation: operation.into(),
            completed: Vec::new(),
            entries: Vec::new(),
        }
    }

    pub fn record(&mut self, step: TxStep, detail: impl Into<String>) {
        self.completed.push(step);
        self.entries.push(JournalEntry {
            step,
            detail: detail.into(),
        });
    }

    pub fn write_to(&self, path: &Path) -> io::Result<()> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let json = serde_json::to_vec_pretty(self)
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
        let mut f = OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .open(path)?;
        f.write_all(&json)?;
        f.sync_all()?;
        Ok(())
    }

    pub fn read_from(path: &Path) -> io::Result<Self> {
        let bytes = fs::read(path)?;
        serde_json::from_slice(&bytes).map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))
    }
}

#[derive(Debug, Clone)]
pub struct TransactionPlan {
    pub txid: String,
    pub operation: String,
    pub target: PathBuf,
    pub stage: PathBuf,
    pub quarantine: PathBuf,
    pub backup_root: PathBuf,
    pub journal_path: PathBuf,
}

impl TransactionPlan {
    pub fn for_plugin_replace(
        paths: &PluginPaths,
        txid: &str,
        operation: impl Into<String>,
        backup_stamp: &str,
    ) -> Result<Self, TransactionError> {
        validate_txid(txid)?;
        Ok(Self {
            txid: txid.to_string(),
            operation: operation.into(),
            target: paths.plugin_root.clone(),
            stage: paths.stage_dir(txid)?,
            quarantine: paths.quarantine_dir(txid)?,
            backup_root: paths.backup_root(backup_stamp),
            journal_path: paths.journal_path(txid)?,
        })
    }
}

#[derive(Debug, Clone)]
pub struct TxReport {
    pub txid: String,
    pub ok: bool,
    pub rolled_back: bool,
    pub journal: TransactionJournal,
    pub evidence: Vec<OwnershipEvidence>,
    pub message: String,
}

/// Atomic directory exchange via `renameat2(RENAME_EXCHANGE)` when available.
pub fn exchange_paths(a: &Path, b: &Path) -> Result<(), TransactionError> {
    if !same_filesystem(a, b)? {
        return Err(TransactionError::msg(
            "cross-filesystem exchange rejected before mutation",
        ));
    }
    #[cfg(target_os = "linux")]
    {
        use rustix::fs::{renameat_with, RenameFlags, CWD};
        match renameat_with(CWD, a, CWD, b, RenameFlags::EXCHANGE) {
            Ok(()) => Ok(()),
            Err(err) => {
                // Unsupported / invalid → fail before callers mutate further.
                Err(TransactionError::msg(format!(
                    "renameat2 RENAME_EXCHANGE failed before commit: {err}"
                )))
            }
        }
    }
    #[cfg(not(target_os = "linux"))]
    {
        // Portable fallback for non-Linux CI: swap via temporary name on same FS.
        let parent = a
            .parent()
            .ok_or_else(|| TransactionError::msg("exchange path has no parent"))?;
        let tmp = parent.join(format!(".agent-bar-exchange-tmp-{}", std::process::id()));
        fs::rename(a, &tmp)?;
        fs::rename(b, a)?;
        fs::rename(&tmp, b)?;
        Ok(())
    }
}

/// Inspect a `.tar.zst` (or plain tar when magic is missing) and reject unsafe entries
/// before any extraction write.
pub fn inspect_tar_zst_entries(bytes: &[u8]) -> Result<Vec<String>, TransactionError> {
    let mut reader: Box<dyn Read> = if bytes.len() >= 4 && bytes[0..4] == [0x28, 0xB5, 0x2F, 0xFD] {
        Box::new(zstd::stream::read::Decoder::new(io::Cursor::new(bytes))?)
    } else {
        Box::new(io::Cursor::new(bytes))
    };
    let mut archive = tar::Archive::new(&mut reader);
    let mut names = Vec::new();
    for entry in archive.entries()? {
        let entry = entry?;
        let header = entry.header();
        let path = entry.path()?.into_owned();
        let rel = path
            .to_str()
            .ok_or_else(|| TransactionError::msg("non-utf8 archive path"))?
            .to_string();
        validate_archive_entry_path(&rel)?;
        let kind = header.entry_type();
        if kind.is_symlink() || kind.is_hard_link() {
            return Err(TransactionError::msg(format!(
                "archive link rejected: {rel}"
            )));
        }
        if matches!(
            kind,
            tar::EntryType::Fifo
                | tar::EntryType::Char
                | tar::EntryType::Block
                | tar::EntryType::GNUSparse
        ) {
            return Err(TransactionError::msg(format!(
                "archive special file rejected: {rel}"
            )));
        }
        names.push(rel);
    }
    Ok(names)
}

/// Exclusive maintenance transaction runner with injectable fail points.
pub struct Transaction {
    pub plan: TransactionPlan,
    pub journal: TransactionJournal,
    _gate: ExclusiveMaintenanceGuard,
    fail_point: Option<TxFailPoint>,
    evidence: Vec<OwnershipEvidence>,
}

impl Transaction {
    /// Acquire exclusive maintenance gate, then build plan (final recheck held under lock).
    pub fn begin(
        paths: &PluginPaths,
        gate: &MaintenanceGate,
        txid: &str,
        operation: impl Into<String>,
        backup_stamp: &str,
    ) -> Result<Self, TransactionError> {
        let guard = gate
            .lock_exclusive()
            .map_err(|e| TransactionError::msg(format!("exclusive maintenance lock: {e}")))?;
        let plan = TransactionPlan::for_plugin_replace(paths, txid, operation, backup_stamp)?;
        // Preflight under exclusive lock: stage parent exists, no symlink target.
        fs::create_dir_all(&paths.plugins_dir)?;
        fs::create_dir_all(&paths.transactions_dir)?;
        fs::create_dir_all(&paths.backups_dir)?;
        ensure_not_symlink(&paths.plugins_dir)?;
        if paths.plugin_root.exists() {
            ensure_not_symlink(&paths.plugin_root)?;
        }
        let mut journal = TransactionJournal::new(&plan.txid, &plan.operation);
        journal.record(TxStep::Preflight, "exclusive lock held; layout ready");
        journal.write_to(&plan.journal_path)?;
        Ok(Self {
            plan,
            journal,
            _gate: guard,
            fail_point: None,
            evidence: Vec::new(),
        })
    }

    pub fn with_fail_point(mut self, point: TxFailPoint) -> Self {
        self.fail_point = Some(point);
        self
    }

    fn maybe_fail(&self, point: TxFailPoint) -> Result<(), TransactionError> {
        if self.fail_point == Some(point) {
            return Err(TransactionError::msg(format!(
                "injected failure at {point:?}"
            )));
        }
        Ok(())
    }

    /// Replace `target` with contents of `staged_source` (already prepared dir).
    /// On failure after exchange, restores previous bytes via quarantine swap-back.
    pub fn replace_plugin_dir(
        &mut self,
        staged_source: &Path,
    ) -> Result<TxReport, TransactionError> {
        // Ownership scan of current target (if present).
        if self.plan.target.exists() {
            let ev = capture_evidence(
                &self.plan.target,
                OwnershipClass::OwnedCurrent,
                "plugin root before replace",
            );
            self.evidence.push(ev);
        }
        self.journal
            .record(TxStep::OwnershipScan, "captured before evidence");
        self.journal.write_to(&self.plan.journal_path)?;

        // Backup outside target (MIG-006).
        fs::create_dir_all(&self.plan.backup_root)?;
        if self.plan.target.exists() {
            let dest = self.plan.backup_root.join("plugin");
            copy_dir_all(&self.plan.target, &dest)?;
            self.journal
                .record(TxStep::Backup, format!("backup at {}", dest.display()));
        } else {
            self.journal
                .record(TxStep::Backup, "no existing plugin root");
        }
        self.journal.write_to(&self.plan.journal_path)?;
        self.maybe_fail(TxFailPoint::AfterBackup)?;

        // Stage as destination-local hidden sibling (MIG-002A).
        if self.plan.stage.exists() {
            fs::remove_dir_all(&self.plan.stage)?;
        }
        copy_dir_all(staged_source, &self.plan.stage)?;
        if !same_filesystem(&self.plan.stage, &self.plan.plugins_parent())? {
            let _ = fs::remove_dir_all(&self.plan.stage);
            return Err(TransactionError::msg(
                "stage not on same filesystem as plugins dir",
            ));
        }
        self.journal.record(
            TxStep::Stage,
            format!("staged at {}", self.plan.stage.display()),
        );
        self.journal.write_to(&self.plan.journal_path)?;
        self.maybe_fail(TxFailPoint::AfterStage)?;

        // Validate staged has a manifest (minimal health for this primitive).
        let manifest = self.plan.stage.join("manifest.json");
        if !manifest.is_file() {
            let _ = fs::remove_dir_all(&self.plan.stage);
            return Err(TransactionError::msg("staged bundle missing manifest.json"));
        }
        ensure_not_symlink(&self.plan.stage)?;
        self.journal
            .record(TxStep::ValidateStaged, "manifest.json present");
        self.journal.write_to(&self.plan.journal_path)?;
        self.maybe_fail(TxFailPoint::AfterValidate)?;

        // Exchange / install.
        self.maybe_fail(TxFailPoint::AtExchange)?;
        let had_target = self.plan.target.exists();
        if had_target {
            // stage <-> target; stage path becomes quarantine content location.
            // After EXCHANGE: target has new, stage path has old.
            exchange_paths(&self.plan.stage, &self.plan.target)?;
            // Move old (now at stage path) to quarantine name.
            if self.plan.quarantine.exists() {
                fs::remove_dir_all(&self.plan.quarantine)?;
            }
            fs::rename(&self.plan.stage, &self.plan.quarantine)?;
        } else {
            fs::rename(&self.plan.stage, &self.plan.target)?;
        }
        self.journal
            .record(TxStep::Exchange, "destination-local exchange done");
        self.journal.write_to(&self.plan.journal_path)?;

        // Any failure after exchange must fully restore previous components.
        if let Err(err) = self.post_exchange(had_target) {
            self.rollback_after_exchange(had_target)?;
            return Ok(self.report(false, true, format!("rolled back: {err}")));
        }

        // Drop quarantine on success.
        if self.plan.quarantine.exists() {
            let _ = fs::remove_dir_all(&self.plan.quarantine);
        }

        Ok(self.report(true, false, "plugin replaced"))
    }

    fn post_exchange(&mut self, _had_target: bool) -> Result<(), TransactionError> {
        self.maybe_fail(TxFailPoint::AfterExchange)?;
        self.maybe_fail(TxFailPoint::AtRescan)?;
        self.journal
            .record(TxStep::Rescan, "rescan deferred to omarchy module");
        self.maybe_fail(TxFailPoint::AtHealth)?;
        if !self.plan.target.join("manifest.json").is_file() {
            return Err(TransactionError::msg("health check: missing manifest.json"));
        }
        self.journal
            .record(TxStep::Health, "manifest still present after exchange");
        self.journal.record(TxStep::Commit, "transaction committed");
        self.journal.write_to(&self.plan.journal_path)?;
        Ok(())
    }

    fn rollback_after_exchange(&mut self, had_target: bool) -> Result<(), TransactionError> {
        if had_target && self.plan.quarantine.exists() {
            // quarantine holds previous; swap back.
            exchange_paths(&self.plan.quarantine, &self.plan.target)?;
            let _ = fs::remove_dir_all(&self.plan.quarantine);
        } else if !had_target && self.plan.target.exists() {
            fs::remove_dir_all(&self.plan.target)?;
        }
        // Restore from backup if still needed.
        let backup_plugin = self.plan.backup_root.join("plugin");
        if backup_plugin.exists() && !self.plan.target.exists() {
            copy_dir_all(&backup_plugin, &self.plan.target)?;
        }
        self.journal
            .record(TxStep::Rollback, "restored previous plugin root");
        self.journal.write_to(&self.plan.journal_path)?;
        Ok(())
    }

    fn report(&self, ok: bool, rolled_back: bool, message: impl Into<String>) -> TxReport {
        TxReport {
            txid: self.plan.txid.clone(),
            ok,
            rolled_back,
            journal: self.journal.clone(),
            evidence: self.evidence.clone(),
            message: message.into(),
        }
    }
}

impl TransactionPlan {
    fn plugins_parent(&self) -> PathBuf {
        self.target
            .parent()
            .map(Path::to_path_buf)
            .unwrap_or_else(|| PathBuf::from("."))
    }
}

/// Remove every shell.json layout object whose `id` is exactly `agent-bar.usage`.
///
/// Other plugins, sections, and non-entry objects are preserved. Serialization is
/// pretty JSON with a trailing newline (byte-for-byte restore uses the pre-mutation
/// backup, not this output).
pub fn remove_exact_plugin_entries(shell_bytes: &[u8]) -> Result<Vec<u8>, TransactionError> {
    use serde_json::Value;
    let mut value: Value = serde_json::from_slice(shell_bytes)
        .map_err(|e| TransactionError::msg(format!("invalid shell.json: {e}")))?;
    remove_plugin_entries_in_value(&mut value);
    let mut out = serde_json::to_vec_pretty(&value)
        .map_err(|e| TransactionError::msg(format!("serialize shell.json: {e}")))?;
    if !out.ends_with(b"\n") {
        out.push(b'\n');
    }
    Ok(out)
}

fn remove_plugin_entries_in_value(value: &mut serde_json::Value) {
    use serde_json::Value;
    match value {
        Value::Array(items) => {
            items.retain(|item| {
                item.get("id")
                    .and_then(|v| v.as_str())
                    .is_none_or(|id| id != PLUGIN_ID)
            });
            for item in items.iter_mut() {
                remove_plugin_entries_in_value(item);
            }
        }
        Value::Object(map) => {
            for v in map.values_mut() {
                remove_plugin_entries_in_value(v);
            }
        }
        _ => {}
    }
}

/// Same-filesystem rename for destination-local quarantine (MIG-002A).
///
/// Refuses cross-filesystem moves before any mutation of `src`.
pub fn quarantine_rename(src: &Path, dest: &Path) -> Result<(), TransactionError> {
    if !src.exists() {
        return Err(TransactionError::msg(format!(
            "quarantine source missing: {}",
            src.display()
        )));
    }
    let dest_parent = dest
        .parent()
        .ok_or_else(|| TransactionError::msg("quarantine dest has no parent"))?;
    fs::create_dir_all(dest_parent)?;
    ensure_not_symlink(dest_parent)?;
    // Compare devices using the live source and the destination parent.
    if !same_filesystem(src, dest_parent)? {
        return Err(TransactionError::msg(
            "cross-filesystem quarantine rename rejected before mutation",
        ));
    }
    if dest.exists() {
        if dest.is_dir() {
            fs::remove_dir_all(dest)?;
        } else {
            fs::remove_file(dest)?;
        }
    }
    fs::rename(src, dest)?;
    Ok(())
}

/// Write `bytes` to `path` via a same-directory temp + rename (atomic replace).
pub fn atomic_write_bytes(path: &Path, bytes: &[u8]) -> Result<(), TransactionError> {
    let parent = path
        .parent()
        .ok_or_else(|| TransactionError::msg("atomic write path has no parent"))?;
    fs::create_dir_all(parent)?;
    let tmp = parent.join(format!(".agent-bar-shell-{}.tmp", std::process::id()));
    {
        let mut f = OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .open(&tmp)?;
        f.write_all(bytes)?;
        f.sync_all()?;
    }
    fs::rename(&tmp, path)?;
    if let Ok(dir) = File::open(parent) {
        let _ = dir.sync_all();
    }
    Ok(())
}

/// Recursive regular-file directory copy (refuses special files).
pub(crate) fn copy_dir_all(src: &Path, dst: &Path) -> io::Result<()> {
    fs::create_dir_all(dst)?;
    for entry in fs::read_dir(src)? {
        let entry = entry?;
        let ty = entry.file_type()?;
        let to = dst.join(entry.file_name());
        if ty.is_dir() {
            copy_dir_all(&entry.path(), &to)?;
        } else if ty.is_file() {
            fs::copy(entry.path(), &to)?;
        } else {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("refusing to copy special file {}", entry.path().display()),
            ));
        }
    }
    // Best-effort dir fsync on Linux.
    if let Ok(dir) = File::open(dst) {
        let _ = dir.sync_all();
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::plugin::paths::txid_from_bytes;
    use crate::plugin::paths::PluginPaths;
    use std::os::unix::fs::PermissionsExt;
    use tempfile::tempdir;

    fn setup() -> (tempfile::TempDir, PluginPaths, MaintenanceGate, String) {
        let dir = tempdir().unwrap();
        let home = dir.path().join("home");
        let plugins = dir.path().join("plugins");
        let state = dir.path().join("state");
        fs::create_dir_all(&plugins).unwrap();
        let paths = PluginPaths::with_plugins_dir(&home, &plugins, &state);
        let gate = MaintenanceGate::open(paths.maintenance_lock.clone()).unwrap();
        let txid = txid_from_bytes(b"test-tx-1");
        (dir, paths, gate, txid)
    }

    fn write_bundle(root: &Path, marker: &str) {
        fs::create_dir_all(root.join("bin")).unwrap();
        fs::write(
            root.join("manifest.json"),
            format!(r#"{{"id":"agent-bar.usage","version":"{marker}"}}"#),
        )
        .unwrap();
        fs::write(
            root.join("bin/agent-bar"),
            format!("#!/bin/sh\necho {marker}\n"),
        )
        .unwrap();
        let mut perms = fs::metadata(root.join("bin/agent-bar"))
            .unwrap()
            .permissions();
        perms.set_mode(0o755);
        fs::set_permissions(root.join("bin/agent-bar"), perms).unwrap();
    }

    #[test]
    fn replace_installs_when_missing() {
        let (_dir, paths, gate, txid) = setup();
        let staged = paths.plugins_dir.join("incoming");
        write_bundle(&staged, "10.0.0");
        let mut tx = Transaction::begin(&paths, &gate, &txid, "install", "2026-07-26T00").unwrap();
        let report = tx.replace_plugin_dir(&staged).unwrap();
        assert!(report.ok);
        assert!(!report.rolled_back);
        assert!(paths.plugin_root.join("manifest.json").is_file());
        let body = fs::read_to_string(paths.plugin_root.join("manifest.json")).unwrap();
        assert!(body.contains("10.0.0"));
        assert!(tx.journal.completed.contains(&TxStep::Commit));
    }

    #[test]
    fn replace_exchanges_existing_and_keeps_backup_outside() {
        let (_dir, paths, gate, txid) = setup();
        write_bundle(&paths.plugin_root, "9.0.0");
        let staged = paths.plugins_dir.join("incoming");
        write_bundle(&staged, "10.0.0");
        let mut tx = Transaction::begin(&paths, &gate, &txid, "update", "t1").unwrap();
        let report = tx.replace_plugin_dir(&staged).unwrap();
        assert!(report.ok);
        let body = fs::read_to_string(paths.plugin_root.join("manifest.json")).unwrap();
        assert!(body.contains("10.0.0"));
        let backup = paths.backup_root("t1").join("plugin/manifest.json");
        assert!(backup.is_file());
        let old = fs::read_to_string(backup).unwrap();
        assert!(old.contains("9.0.0"));
        assert!(!paths.backup_root("t1").starts_with(&paths.plugin_root));
    }

    #[test]
    fn fail_after_stage_does_not_touch_target() {
        let (_dir, paths, gate, txid) = setup();
        write_bundle(&paths.plugin_root, "9.0.0");
        let staged = paths.plugins_dir.join("incoming");
        write_bundle(&staged, "10.0.0");
        let mut tx = Transaction::begin(&paths, &gate, &txid, "update", "t2")
            .unwrap()
            .with_fail_point(TxFailPoint::AfterStage);
        let err = tx.replace_plugin_dir(&staged).unwrap_err();
        assert!(err.to_string().contains("AfterStage"));
        let body = fs::read_to_string(paths.plugin_root.join("manifest.json")).unwrap();
        assert!(body.contains("9.0.0"), "target unchanged: {body}");
    }

    #[test]
    fn fail_after_exchange_rolls_back_byte_for_byte() {
        let (_dir, paths, gate, txid) = setup();
        write_bundle(&paths.plugin_root, "9.0.0");
        let before = fs::read(paths.plugin_root.join("manifest.json")).unwrap();
        let staged = paths.plugins_dir.join("incoming");
        write_bundle(&staged, "10.0.0");
        let mut tx = Transaction::begin(&paths, &gate, &txid, "update", "t3")
            .unwrap()
            .with_fail_point(TxFailPoint::AfterExchange);
        let report = tx.replace_plugin_dir(&staged).unwrap();
        assert!(!report.ok);
        assert!(report.rolled_back);
        assert!(report.journal.completed.contains(&TxStep::Rollback));
        let after = fs::read(paths.plugin_root.join("manifest.json")).unwrap();
        assert_eq!(after, before, "byte-for-byte rollback of previous plugin");
    }

    #[test]
    fn exclusive_gate_blocks_second_begin() {
        let (_dir, paths, gate, txid) = setup();
        let _tx = Transaction::begin(&paths, &gate, &txid, "update", "t4").unwrap();
        assert!(gate.try_lock_shared().unwrap().is_none());
    }

    #[test]
    fn journal_persists_steps() {
        let (_dir, paths, gate, txid) = setup();
        let staged = paths.plugins_dir.join("incoming");
        write_bundle(&staged, "10.0.0");
        let mut tx = Transaction::begin(&paths, &gate, &txid, "install", "t5").unwrap();
        tx.replace_plugin_dir(&staged).unwrap();
        let loaded = TransactionJournal::read_from(&tx.plan.journal_path).unwrap();
        assert!(loaded.completed.contains(&TxStep::Commit));
        assert_eq!(loaded.txid, txid);
    }

    #[test]
    fn inspect_rejects_symlink_entry() {
        let mut builder = tar::Builder::new(Vec::new());
        let mut header = tar::Header::new_gnu();
        header.set_entry_type(tar::EntryType::Symlink);
        header.set_path("evil-link").unwrap();
        header.set_link_name("/tmp/target").unwrap();
        header.set_size(0);
        header.set_cksum();
        builder.append(&header, std::io::empty()).unwrap();
        let bytes = builder.into_inner().unwrap();
        let err = inspect_tar_zst_entries(&bytes).unwrap_err();
        assert!(
            err.to_string().contains("link") || err.to_string().contains("rejected"),
            "{err}"
        );
    }

    #[test]
    fn inspect_accepts_safe_entries() {
        let mut builder = tar::Builder::new(Vec::new());
        let mut header = tar::Header::new_gnu();
        header.set_path("manifest.json").unwrap();
        header.set_size(2);
        header.set_cksum();
        builder.append(&header, &b"{}"[..]).unwrap();
        let bytes = builder.into_inner().unwrap();
        let names = inspect_tar_zst_entries(&bytes).unwrap();
        assert_eq!(names, vec!["manifest.json".to_string()]);
    }

    #[test]
    fn exchange_rejects_when_paths_missing_same_fs_check_ok_for_siblings() {
        let dir = tempdir().unwrap();
        let a = dir.path().join("a");
        let b = dir.path().join("b");
        fs::create_dir_all(&a).unwrap();
        fs::create_dir_all(&b).unwrap();
        fs::write(a.join("x"), b"1").unwrap();
        fs::write(b.join("y"), b"2").unwrap();
        exchange_paths(&a, &b).unwrap();
        assert!(b.join("x").is_file());
        assert!(a.join("y").is_file());
    }

    #[test]
    fn external_shared_writer_blocked_while_exclusive() {
        let (_dir, paths, gate, txid) = setup();
        let _tx = Transaction::begin(&paths, &gate, &txid, "update", "t6").unwrap();
        // Simulate cache writer needing shared lock: must not acquire.
        assert!(gate.try_lock_shared().unwrap().is_none());
    }

    #[test]
    fn remove_exact_plugin_entries_preserves_neighbors() {
        let shell = br#"{
  "bar": {
    "left": [
      {"id": "omarchy.menu"},
      {"id": "agent-bar.usage", "refreshIntervalSec": 60},
      {"id": "omarchy.workspaces"}
    ]
  }
}"#;
        let out = remove_exact_plugin_entries(shell).unwrap();
        let v: serde_json::Value = serde_json::from_slice(&out).unwrap();
        let left = v["bar"]["left"].as_array().unwrap();
        assert_eq!(left.len(), 2);
        assert_eq!(left[0]["id"], "omarchy.menu");
        assert_eq!(left[1]["id"], "omarchy.workspaces");
        assert!(!out
            .windows(PLUGIN_ID.len())
            .any(|w| w == PLUGIN_ID.as_bytes()));
    }

    #[test]
    fn quarantine_rename_moves_same_fs_sibling() {
        let dir = tempdir().unwrap();
        let src = dir.path().join("agent-bar.usage");
        let dest = dir
            .path()
            .join(".agent-bar.usage.quarantine-aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa");
        fs::create_dir_all(&src).unwrap();
        fs::write(src.join("manifest.json"), b"{}").unwrap();
        quarantine_rename(&src, &dest).unwrap();
        assert!(!src.exists());
        assert!(dest.join("manifest.json").is_file());
    }
}
