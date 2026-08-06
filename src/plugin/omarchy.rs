//! Process execution seam for the omarchy CLI (MIG-019A/B lineage).
//!
//! `update apply` / `uninstall` build their own detached-unit argv inline
//! (git-plugin-distribution Tasks 2-3) and run it through [`CommandRunner`];
//! the argv-builder/`OmarchyClient` wrapper that used to sit in front of that
//! is gone with it — nothing calls `omarchy plugin enable|rescan` from this
//! helper anymore (`omarchy plugin add` is the install now).

use thiserror::Error;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum OmarchyError {
    #[error("{0}")]
    Message(String),
}

impl OmarchyError {
    fn msg(s: impl Into<String>) -> Self {
        Self::Message(s.into())
    }
}

/// Result of a single argv invocation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommandOutput {
    pub code: i32,
    pub stdout: String,
    pub stderr: String,
}

/// Injectable command runner for tests (records exact argv).
pub trait CommandRunner {
    fn run(&self, program: &str, args: &[&str]) -> Result<CommandOutput, OmarchyError>;
}

/// Production runner via `std::process::Command`.
#[derive(Debug, Default, Clone, Copy)]
pub struct ProcessCommandRunner;

impl CommandRunner for ProcessCommandRunner {
    fn run(&self, program: &str, args: &[&str]) -> Result<CommandOutput, OmarchyError> {
        let out = std::process::Command::new(program)
            .args(args)
            .output()
            .map_err(|e| OmarchyError::msg(format!("failed to spawn {program}: {e}")))?;
        Ok(CommandOutput {
            code: out.status.code().unwrap_or(1),
            stdout: String::from_utf8_lossy(&out.stdout).into_owned(),
            stderr: String::from_utf8_lossy(&out.stderr).into_owned(),
        })
    }
}
