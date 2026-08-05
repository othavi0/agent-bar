//! Plugin bundle paths, ownership classification, and maintenance.

pub mod bundle;
pub mod doctor;
pub mod maintenance;
pub mod omarchy;
pub mod ownership;
pub mod paths;

pub use bundle::{
    BundleBuilder, BundleError, BundleFileEntry, BundleReceipt, BundleValidator,
    MINIMUM_QUICKSHELL_VERSION, OFFICIAL_TARGET, OMARCHY_CONTRACT,
};
pub use doctor::{default_ownership_rules, doctor_clean, doctor_scan, DoctorError, DoctorReport};
pub use maintenance::{
    require_absolute_executable, resolve_absolute_executable, MaintenanceError, ReqwestReleaseHttp,
    UninstallConfirmation, UpdateCheck, UpdateCheckDocument, UpdateCheckProbe,
    UNINSTALL_TTY_PHRASE, UNINSTALL_TTY_PROMPT,
};
pub use omarchy::{CommandOutput, CommandRunner, OmarchyError, ProcessCommandRunner};
pub use ownership::{
    capture_evidence, classify_artifact, hash_bytes, hash_path, FileKind, OwnershipClass,
    OwnershipEvidence, OwnershipRules,
};
pub use paths::{
    is_hidden_plugin_sibling, txid_from_bytes, validate_archive_entry_path, validate_txid,
    PathError, PluginPaths, PLUGIN_ID,
};
