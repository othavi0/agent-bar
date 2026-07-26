//! Plugin bundle paths, ownership classification, and maintenance transactions.

pub mod doctor;
pub mod omarchy;
pub mod ownership;
pub mod paths;
pub mod transaction;

pub use doctor::{doctor_clean, doctor_scan, DoctorError, DoctorReport};
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
    is_hidden_plugin_sibling, validate_archive_entry_path, validate_txid, PathError, PluginPaths,
    PLUGIN_ID,
};
pub use transaction::{
    exchange_paths, inspect_tar_zst_entries, JournalEntry, Transaction, TransactionError,
    TransactionJournal, TransactionPlan, TxFailPoint, TxReport, TxStep,
};
