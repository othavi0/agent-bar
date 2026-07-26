//! Argv-only process execution with timeout, output limits, and no shell.

use std::path::PathBuf;
use std::process::Stdio;
use std::time::Duration;

use tokio::io::AsyncReadExt;
use tokio::process::Command;
use tokio::time::timeout;

use crate::support::redact::redact_process_bytes;

/// Exact argv process specification. Never join into a shell string.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProcessSpec {
    pub program: PathBuf,
    pub args: Vec<String>,
    pub env: Vec<(String, String)>,
    pub clear_env: bool,
    pub timeout: Duration,
    pub max_stdout_bytes: usize,
    pub max_stderr_bytes: usize,
}

impl ProcessSpec {
    pub fn new(
        program: impl Into<PathBuf>,
        args: impl IntoIterator<Item = impl Into<String>>,
    ) -> Self {
        Self {
            program: program.into(),
            args: args.into_iter().map(Into::into).collect(),
            env: Vec::new(),
            clear_env: false,
            timeout: Duration::from_secs(10),
            max_stdout_bytes: 1024 * 1024,
            max_stderr_bytes: 1024 * 1024,
        }
    }

    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }

    pub fn with_max_output(mut self, max: usize) -> Self {
        self.max_stdout_bytes = max;
        self.max_stderr_bytes = max;
        self
    }

    pub fn with_env(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.env.push((key.into(), value.into()));
        self
    }
}

/// Captured process output after redaction of controls/ANSI.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProcessOutput {
    pub exit_code: Option<i32>,
    pub stdout: String,
    pub stderr: String,
    pub timed_out: bool,
    pub stdout_truncated: bool,
    pub stderr_truncated: bool,
}

/// Process runner failure independent of provider domain state.
#[derive(Debug)]
pub enum ProcessError {
    Io(std::io::Error),
    Spawn(String),
}

impl std::fmt::Display for ProcessError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(err) => write!(f, "{err}"),
            Self::Spawn(msg) => write!(f, "{msg}"),
        }
    }
}

impl std::error::Error for ProcessError {}

impl From<std::io::Error> for ProcessError {
    fn from(value: std::io::Error) -> Self {
        Self::Io(value)
    }
}

/// Runnable process seam for adapters and tests.
pub trait ProcessRunner: Send + Sync {
    fn run(
        &self,
        spec: &ProcessSpec,
    ) -> impl std::future::Future<Output = Result<ProcessOutput, ProcessError>> + Send;
}

/// Tokio-backed runner: argv only, never `sh -c` / `bash -lc` / `eval`.
#[derive(Debug, Default, Clone, Copy)]
pub struct TokioProcessRunner;

impl ProcessRunner for TokioProcessRunner {
    async fn run(&self, spec: &ProcessSpec) -> Result<ProcessOutput, ProcessError> {
        run_process(spec).await
    }
}

/// Execute `spec` without a shell, enforcing timeout and output caps.
pub async fn run_process(spec: &ProcessSpec) -> Result<ProcessOutput, ProcessError> {
    // Hard guard: program path must not be a shell used with -c.
    let program_name = spec
        .program
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("");
    if matches!(program_name, "sh" | "bash" | "zsh" | "dash")
        && spec.args.first().is_some_and(|a| a == "-c" || a == "-lc")
    {
        return Err(ProcessError::Spawn(
            "shell process invocation is forbidden".into(),
        ));
    }

    let mut command = Command::new(&spec.program);
    command
        .args(&spec.args)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true);
    if spec.clear_env {
        command.env_clear();
    }
    for (key, value) in &spec.env {
        command.env(key, value);
    }

    let mut child = command.spawn().map_err(|err| {
        ProcessError::Spawn(format!("failed to spawn {}: {err}", spec.program.display()))
    })?;

    let mut stdout_pipe = child.stdout.take();
    let mut stderr_pipe = child.stderr.take();
    let max_out = spec.max_stdout_bytes;
    let max_err = spec.max_stderr_bytes;

    let read_fut = async {
        let stdout_task = async {
            let mut buf = Vec::new();
            let mut truncated = false;
            if let Some(pipe) = stdout_pipe.as_mut() {
                let mut chunk = [0u8; 8192];
                loop {
                    let n = pipe.read(&mut chunk).await?;
                    if n == 0 {
                        break;
                    }
                    if truncated {
                        // Drain so the child cannot block on a full pipe.
                        continue;
                    }
                    let remaining = max_out.saturating_sub(buf.len());
                    if remaining == 0 {
                        truncated = true;
                        continue;
                    }
                    let take = n.min(remaining);
                    buf.extend_from_slice(&chunk[..take]);
                    if take < n {
                        truncated = true;
                    }
                }
            }
            Ok::<_, std::io::Error>((buf, truncated))
        };
        let stderr_task = async {
            let mut buf = Vec::new();
            let mut truncated = false;
            if let Some(pipe) = stderr_pipe.as_mut() {
                let mut chunk = [0u8; 8192];
                loop {
                    let n = pipe.read(&mut chunk).await?;
                    if n == 0 {
                        break;
                    }
                    if truncated {
                        continue;
                    }
                    let remaining = max_err.saturating_sub(buf.len());
                    if remaining == 0 {
                        truncated = true;
                        continue;
                    }
                    let take = n.min(remaining);
                    buf.extend_from_slice(&chunk[..take]);
                    if take < n {
                        truncated = true;
                    }
                }
            }
            Ok::<_, std::io::Error>((buf, truncated))
        };
        let (stdout, stderr) = tokio::join!(stdout_task, stderr_task);
        Ok::<_, std::io::Error>((stdout?, stderr?))
    };

    let wait_fut = async {
        let status = child.wait().await?;
        Ok::<_, std::io::Error>(status)
    };

    let joined = timeout(spec.timeout, async {
        let ((stdout, stderr), status) = tokio::try_join!(read_fut, wait_fut)?;
        Ok::<_, std::io::Error>((stdout, stderr, status))
    })
    .await;

    match joined {
        Ok(Ok(((stdout_raw, stdout_truncated), (stderr_raw, stderr_truncated), status))) => {
            Ok(ProcessOutput {
                exit_code: status.code(),
                stdout: redact_process_bytes(&stdout_raw),
                stderr: redact_process_bytes(&stderr_raw),
                timed_out: false,
                stdout_truncated,
                stderr_truncated,
            })
        }
        Ok(Err(err)) => Err(ProcessError::Io(err)),
        Err(_elapsed) => {
            // Timeout: kill and reap.
            let _ = child.kill().await;
            let _ = child.wait().await;
            Ok(ProcessOutput {
                exit_code: None,
                stdout: String::new(),
                stderr: String::new(),
                timed_out: true,
                stdout_truncated: false,
                stderr_truncated: false,
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[tokio::test]
    async fn preserves_argv_and_exit_code() {
        let spec = ProcessSpec::new("/bin/echo", ["hello", "world"]);
        let out = TokioProcessRunner.run(&spec).await.unwrap();
        assert_eq!(out.exit_code, Some(0));
        assert_eq!(out.stdout.trim_end(), "hello world");
        assert!(!out.timed_out);
    }

    #[tokio::test]
    async fn timeout_kills_and_reaps() {
        let spec = ProcessSpec::new("/bin/sleep", ["5"]).with_timeout(Duration::from_millis(100));
        let out = TokioProcessRunner.run(&spec).await.unwrap();
        assert!(out.timed_out);
        assert_eq!(out.exit_code, None);
    }

    #[tokio::test]
    async fn enforces_stdout_limit() {
        // Print more than the cap via printf-like python or yes | head.
        // Use a small shell-free generator: /usr/bin/printf if available.
        let program = if Path::new("/usr/bin/printf").exists() {
            "/usr/bin/printf"
        } else {
            "/bin/echo"
        };
        let spec = if program.ends_with("printf") {
            ProcessSpec::new(program, ["%1024s", "x"])
                .with_max_output(64)
                .with_timeout(Duration::from_secs(2))
        } else {
            ProcessSpec::new(program, ["x".repeat(200)])
                .with_max_output(64)
                .with_timeout(Duration::from_secs(2))
        };
        let out = TokioProcessRunner.run(&spec).await.unwrap();
        assert!(out.stdout.len() <= 64 || out.stdout_truncated);
        if program.ends_with("printf") {
            assert!(out.stdout_truncated || out.stdout.len() == 64);
            assert!(out.stdout.len() <= 64);
        }
    }

    #[tokio::test]
    async fn redacts_ansi_from_stdout() {
        let program = if Path::new("/usr/bin/printf").exists() {
            "/usr/bin/printf"
        } else {
            return; // environment without printf; skip
        };
        let spec = ProcessSpec::new(program, [r"%b", r"\033[31mred\033[0m"]);
        let out = TokioProcessRunner.run(&spec).await.unwrap();
        assert_eq!(out.stdout, "red");
        assert!(!out.stdout.contains('\u{1b}'));
    }

    #[tokio::test]
    async fn rejects_shell_dash_c() {
        let spec = ProcessSpec::new("/bin/sh", ["-c", "echo pwned"]);
        let err = TokioProcessRunner.run(&spec).await.unwrap_err();
        assert!(err.to_string().contains("shell"));
    }

    #[tokio::test]
    async fn env_injection_without_joining_argv() {
        let program = if Path::new("/usr/bin/printenv").exists() {
            "/usr/bin/printenv"
        } else if Path::new("/bin/printenv").exists() {
            "/bin/printenv"
        } else {
            return;
        };
        let spec = ProcessSpec::new(program, ["AGENT_BAR_TEST_ENV"])
            .with_env("AGENT_BAR_TEST_ENV", "ok-value")
            .with_timeout(Duration::from_secs(2));
        let out = TokioProcessRunner.run(&spec).await.unwrap();
        assert_eq!(out.stdout.trim_end(), "ok-value");
    }
}
