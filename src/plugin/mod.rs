//! Plugin bundle paths, ownership classification, and maintenance transactions.

pub mod bundle;
pub mod doctor;
pub mod maintenance;
pub mod omarchy;
pub mod ownership;
pub mod paths;
pub mod transaction;

pub use bundle::{
    BundleBuilder, BundleError, BundleFileEntry, BundleReceipt, BundleValidator, ReleaseBuilder,
    ReleaseMetadata, MINIMUM_QUICKSHELL_VERSION, OFFICIAL_TARGET, OMARCHY_CONTRACT,
};
pub use doctor::{doctor_clean, doctor_scan, DoctorError, DoctorReport};
pub use maintenance::{
    apply_version_allowed, classify_local_plugin, collect_worker_env, download_with_policy,
    is_maintenance_worker_exe, notify_uninstall_complete, poll_uninstall_absence,
    preflight_existing_health, prepare_local_plugin_for_update, require_absolute_executable,
    resolve_absolute_executable, stage_update_bundle, LocalPluginClass, LocalPluginPrep,
    MaintenanceError, MaintenanceJournalPayload, MaintenanceOp, MaintenanceWorker, RealSleeper,
    ReqwestReleaseHttp, UninstallConfirmation, UpdateCheck, UpdateCheckDocument, UpdateCheckProbe,
    MAINTENANCE_WORKER_NAME, UNINSTALL_TTY_PHRASE, UNINSTALL_TTY_PROMPT, WORKER_ENV_ALLOWLIST,
};
pub use omarchy::{
    argv_is_approved, enable_argv, rescan_argv, CommandOutput, CommandRunner, OmarchyClient,
    OmarchyError, ProcessCommandRunner,
};

#[cfg(test)]
pub use omarchy::RecordingRunner;
pub use ownership::{
    capture_evidence, classify_artifact, hash_bytes, hash_path, FileKind, OwnershipClass,
    OwnershipEvidence, OwnershipRules,
};
pub use paths::{
    is_hidden_plugin_sibling, txid_from_bytes, validate_archive_entry_path, validate_txid,
    PathError, PluginPaths, PLUGIN_ID,
};
pub use transaction::{
    atomic_write_bytes, exchange_paths, inspect_tar_zst_entries, quarantine_rename,
    remove_exact_plugin_entries, JournalEntry, Transaction, TransactionError, TransactionJournal,
    TransactionPlan, TxFailPoint, TxReport, TxStep,
};
