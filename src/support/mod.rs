//! Shared test seams and filesystem/clock primitives for v10.

mod clock;
mod fs;

pub use clock::{Clock, SystemClock};
pub use fs::{FileMetadata, FileSystem, RealFileSystem};
