//! Shared test seams and filesystem/clock primitives for v10.

pub mod atomic_file;
mod clock;
mod fs;
pub mod maintenance_gate;
pub mod redact;

pub use atomic_file::{replace_atomically, replace_atomically_with, FileMutator, StdFileMutator};
pub use clock::{Clock, SystemClock};
pub use fs::{FileMetadata, FileSystem, RealFileSystem};
pub use maintenance_gate::{
    shared_gate, ExclusiveMaintenanceGuard, MaintenanceGate, SharedMaintenanceGate,
    SharedMaintenanceGuard,
};
pub use redact::{redact_process_bytes, strip_ansi_and_controls};

#[cfg(test)]
pub use atomic_file::{AtomicFailPoint, FailingMutator};
