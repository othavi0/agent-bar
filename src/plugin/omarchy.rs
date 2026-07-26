//! Exact Quattro/Omarchy CLI argv for enable and rescan (MIG-019A/B).

use std::path::Path;

use thiserror::Error;

use crate::plugin::paths::PLUGIN_ID;

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

/// Recording runner for tests.
#[cfg(test)]
#[derive(Debug, Default)]
pub struct RecordingRunner {
    pub calls: std::sync::Mutex<Vec<(String, Vec<String>)>>,
    pub responses: std::sync::Mutex<Vec<Result<CommandOutput, OmarchyError>>>,
}

#[cfg(test)]
impl RecordingRunner {
    pub fn with_ok_responses(n: usize) -> Self {
        let mut responses = Vec::new();
        for _ in 0..n {
            responses.push(Ok(CommandOutput {
                code: 0,
                stdout: String::new(),
                stderr: String::new(),
            }));
        }
        Self {
            calls: std::sync::Mutex::new(Vec::new()),
            responses: std::sync::Mutex::new(responses),
        }
    }

    pub fn recorded(&self) -> Vec<(String, Vec<String>)> {
        self.calls.lock().unwrap_or_else(|e| e.into_inner()).clone()
    }
}

#[cfg(test)]
impl CommandRunner for RecordingRunner {
    fn run(&self, program: &str, args: &[&str]) -> Result<CommandOutput, OmarchyError> {
        self.calls.lock().unwrap_or_else(|e| e.into_inner()).push((
            program.to_string(),
            args.iter().map(|s| (*s).to_string()).collect(),
        ));
        let mut q = self.responses.lock().unwrap_or_else(|e| e.into_inner());
        if q.is_empty() {
            return Ok(CommandOutput {
                code: 0,
                stdout: String::new(),
                stderr: String::new(),
            });
        }
        q.remove(0)
    }
}

/// Exact argv for fresh setup (MIG-019A). Never followed by `bar plugin add`.
pub fn enable_argv() -> Vec<&'static str> {
    vec!["plugin", "enable", PLUGIN_ID]
}

/// Exact argv for existing setup/update (MIG-018 / MIG-019B).
pub fn rescan_argv() -> Vec<&'static str> {
    vec!["plugin", "rescan"]
}

/// Client that only issues approved Omarchy commands.
pub struct OmarchyClient<R: CommandRunner> {
    runner: R,
    program: String,
}

impl<R: CommandRunner> OmarchyClient<R> {
    pub fn new(runner: R) -> Self {
        Self {
            runner,
            program: "omarchy".into(),
        }
    }

    pub fn with_program(mut self, program: impl Into<String>) -> Self {
        self.program = program.into();
        self
    }

    /// Fresh setup: `omarchy plugin enable agent-bar.usage` only.
    pub fn enable_plugin(&self) -> Result<CommandOutput, OmarchyError> {
        let args = enable_argv();
        let out = self.runner.run(&self.program, &args)?;
        if out.code != 0 {
            return Err(OmarchyError::msg(format!(
                "omarchy plugin enable failed (exit {}): {}",
                out.code,
                out.stderr.trim()
            )));
        }
        Ok(out)
    }

    /// Existing setup/update: `omarchy plugin rescan` only. Does not edit shell.json.
    pub fn rescan(&self) -> Result<CommandOutput, OmarchyError> {
        let args = rescan_argv();
        let out = self.runner.run(&self.program, &args)?;
        if out.code != 0 {
            return Err(OmarchyError::msg(format!(
                "omarchy plugin rescan failed (exit {}): {}",
                out.code,
                out.stderr.trim()
            )));
        }
        Ok(out)
    }

    /// Choose enable vs rescan based on whether the plugin entry already exists.
    pub fn activate(&self, shell_has_entry: bool) -> Result<CommandOutput, OmarchyError> {
        if shell_has_entry {
            self.rescan()
        } else {
            self.enable_plugin()
        }
    }
}

/// Production convenience: true when `shell.json` already lists the plugin.
///
/// Tolerant of the exact shell.json shape (Omarchy schema, not ours). Returns
/// false when the file is missing or not JSON.
pub fn shell_has_plugin_entry(shell_json_path: &Path) -> bool {
    let Ok(content) = std::fs::read_to_string(shell_json_path) else {
        return false;
    };
    let Ok(value) = serde_json::from_str::<serde_json::Value>(&content) else {
        return false;
    };
    json_contains_string(&value, PLUGIN_ID)
}

fn json_contains_string(value: &serde_json::Value, needle: &str) -> bool {
    match value {
        serde_json::Value::String(s) => s == needle,
        serde_json::Value::Array(items) => items.iter().any(|v| json_contains_string(v, needle)),
        serde_json::Value::Object(map) => map.values().any(|v| json_contains_string(v, needle)),
        _ => false,
    }
}

/// Guard: approved argv never includes `bar plugin add`.
pub fn argv_is_approved(args: &[&str]) -> bool {
    if args.is_empty() {
        return false;
    }
    // Forbidden unconditional placement rewrite.
    if args.len() >= 3 && args[0] == "bar" && args[1] == "plugin" && args[2] == "add" {
        return false;
    }
    matches!(
        args,
        ["plugin", "enable", id] if *id == PLUGIN_ID
    ) || matches!(args, ["plugin", "rescan"])
}

// Allow RecordingRunner to be used behind reference for tests.
#[cfg(test)]
impl CommandRunner for &RecordingRunner {
    fn run(&self, program: &str, args: &[&str]) -> Result<CommandOutput, OmarchyError> {
        (*self).run(program, args)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn enable_argv_is_exact() {
        assert_eq!(enable_argv(), vec!["plugin", "enable", "agent-bar.usage"]);
        assert!(argv_is_approved(&enable_argv()));
    }

    #[test]
    fn rescan_argv_is_exact() {
        assert_eq!(rescan_argv(), vec!["plugin", "rescan"]);
        assert!(argv_is_approved(&rescan_argv()));
    }

    #[test]
    fn never_approves_bar_plugin_add() {
        assert!(!argv_is_approved(&[
            "bar",
            "plugin",
            "add",
            "agent-bar.usage"
        ]));
    }

    #[test]
    fn client_enable_and_rescan_argv() {
        let runner = RecordingRunner::with_ok_responses(3);
        let client = OmarchyClient::new(&runner);
        client.enable_plugin().unwrap();
        client.rescan().unwrap();
        client.activate(true).unwrap();
        let calls = runner.recorded();
        assert_eq!(
            calls[0],
            (
                "omarchy".into(),
                vec!["plugin".into(), "enable".into(), "agent-bar.usage".into()]
            )
        );
        assert_eq!(
            calls[1],
            ("omarchy".into(), vec!["plugin".into(), "rescan".into()])
        );
        assert_eq!(
            calls[2],
            ("omarchy".into(), vec!["plugin".into(), "rescan".into()])
        );
        for (_prog, args) in &calls {
            let slice: Vec<&str> = args.iter().map(String::as_str).collect();
            assert!(argv_is_approved(&slice));
            assert!(!args.iter().any(|a| a == "add"));
        }
    }

    #[test]
    fn activate_fresh_uses_enable() {
        let runner = RecordingRunner::with_ok_responses(1);
        let client = OmarchyClient::new(&runner);
        client.activate(false).unwrap();
        let calls = runner.recorded();
        assert_eq!(calls[0].1[1], "enable");
    }

    #[test]
    fn rescan_failure_surfaces() {
        let runner = RecordingRunner {
            calls: std::sync::Mutex::new(Vec::new()),
            responses: std::sync::Mutex::new(vec![Ok(CommandOutput {
                code: 1,
                stdout: String::new(),
                stderr: "rescan failed".into(),
            })]),
        };
        let client = OmarchyClient::new(&runner);
        let err = client.rescan().unwrap_err();
        assert!(err.to_string().contains("rescan failed"));
    }
}
