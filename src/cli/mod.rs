//! Strict word-based CLI for the private Agent Bar helper.

mod command;
mod exit;
mod grammar;

pub use command::{
    CacheMode, Command, ConfigCommand, ConfigInput, DoctorCommand, HelpTopic, NotificationMode,
    ProviderId, ReleaseVersion, SetupOptions, StatusFormat, StatusOptions, UpdateCommand,
};
pub use exit::{
    CliFailure, GENERIC_FAILURE, GRAMMAR, INTERNAL, PLUGIN, SERIALIZATION, SUCCESS, VALIDATION,
};
pub use grammar::parse;

use std::io::{self, BufRead, IsTerminal, Write};
use std::path::{Path, PathBuf};

/// Package version line for `version` / `--version` (exact semver + newline).
pub fn version_stdout() -> String {
    format!("{}\n", env!("CARGO_PKG_VERSION"))
}

/// Public help text for the plugin-first product.
pub fn help_text(topic: Option<HelpTopic>) -> String {
    match topic {
        None => {
            let mut out = String::new();
            out.push_str("Agent Bar — Omarchy Quattro plugin helper\n");
            out.push('\n');
            out.push_str("The normal interface is the agent-bar.usage Quickshell plugin.\n");
            out.push_str("This private helper is for diagnostics, recovery, and tests.\n");
            out.push('\n');
            out.push_str("Usage:\n");
            out.push_str("  agent-bar\n");
            out.push_str("  agent-bar status [format human|json] [provider <id>]\n");
            out.push_str("                 [cache use|bypass] [notifications evaluate|skip]\n");
            out.push_str("  agent-bar login <provider>\n");
            out.push_str("  agent-bar config show\n");
            out.push_str("  agent-bar config apply stdin|file <path>|json <value>\n");
            out.push_str("  agent-bar setup [plugins-dir <absolute-parent>]\n");
            out.push_str("  agent-bar update [check|apply <version>]\n");
            out.push_str("  agent-bar uninstall [purge]\n");
            out.push_str("  agent-bar doctor scan|clean\n");
            out.push_str("  agent-bar help [<command>]\n");
            out.push_str("  agent-bar version\n");
            out.push('\n');
            out.push_str("Providers: claude, codex, amp, grok\n");
            out
        }
        Some(HelpTopic::Status) => "status — collect provider quota windows\n\
             \n\
             Clauses (any order, each at most once):\n\
               format human|json          default: human\n\
               provider <id>              single provider (even if disabled)\n\
               cache use|bypass           default: use\n\
               notifications evaluate|skip  default: skip\n\
             \n\
             Bare agent-bar equals status format human.\n"
            .to_owned(),
        Some(HelpTopic::Login) => {
            "login <provider> — delegate to the official provider login command\n\
             Providers: claude, codex, amp, grok\n"
                .to_owned()
        }
        Some(HelpTopic::Config) => "config show — print canonical settings JSON (read-only)\n\
             config apply stdin|file <path>|json <value> — replace settings\n"
            .to_owned(),
        Some(HelpTopic::Setup) => {
            "setup — install the agent-bar.usage plugin into the production Quattro parent\n\
             setup plugins-dir <path> — install under an existing writable absolute parent\n\
             <path> must not be the plugin root itself (…/agent-bar.usage).\n"
                .to_owned()
        }
        Some(HelpTopic::Update) => "update — interactive update (TTY only)\n\
             update check — report whether a newer release exists\n\
             update apply <version> — apply a strict semantic version\n\
             Non-TTY callers must use check/apply.\n"
            .to_owned(),
        Some(HelpTopic::Uninstall) => {
            "uninstall — remove the plugin (keeps settings and backups)\n\
             uninstall purge — also delete settings and owned backups\n\
             Both forms require confirmation.\n"
                .to_owned()
        }
        Some(HelpTopic::Doctor) => "doctor scan — read-only ownership and legacy scan\n\
             doctor clean — remove confirmed owned legacy artifacts after backup\n"
            .to_owned(),
        Some(HelpTopic::Help) => "help [<command>] — show general or topic help\n".to_owned(),
        Some(HelpTopic::Version) => {
            "version — print the helper semantic version and exit\n".to_owned()
        }
    }
}

/// Filesystem validation for `setup plugins-dir` (CLI-009).
///
/// `<path>` must be an existing writable absolute parent that contains or can
/// receive the `agent-bar.usage` child. Relative paths and a direct plugin-root
/// path are rejected by the grammar before this runs.
pub fn validate_plugins_dir(path: &Path) -> Result<PathBuf, CliFailure> {
    if !path.is_absolute() {
        return Err(CliFailure::grammar(
            "setup plugins-dir path must be absolute",
        ));
    }
    if path
        .file_name()
        .and_then(|s| s.to_str())
        .is_some_and(|n| n == "agent-bar.usage")
    {
        return Err(CliFailure::grammar(
            "setup plugins-dir path must be the parent directory, not the plugin root",
        ));
    }
    let meta = std::fs::metadata(path).map_err(|_| {
        CliFailure::validation(format!(
            "setup plugins-dir path does not exist: {}",
            path.display()
        ))
    })?;
    if !meta.is_dir() {
        return Err(CliFailure::validation(format!(
            "setup plugins-dir path is not a directory: {}",
            path.display()
        )));
    }
    // Probe writability with a same-directory temporary file.
    let probe = path.join(".agent-bar-write-probe");
    match std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&probe)
    {
        Ok(_) => {
            let _ = std::fs::remove_file(&probe);
        }
        Err(_) => {
            return Err(CliFailure::validation(format!(
                "setup plugins-dir path is not writable: {}",
                path.display()
            )));
        }
    }
    Ok(path.to_path_buf())
}

/// Result of an update availability check for interactive confirmation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InteractiveUpdateOffer {
    UpToDate,
    Available { current: String, target: String },
}

/// Pure interactive `update` confirmation gate (CLI contract).
///
/// Non-TTY exits with validation code 3 and guidance. TTY with no update
/// prints the up-to-date line. TTY with an update requires the exact phrase
/// `update agent-bar` on stdin.
pub fn confirm_interactive_update<R, W, E>(
    is_tty: bool,
    offer: InteractiveUpdateOffer,
    stdin: &mut R,
    stdout: &mut W,
    stderr: &mut E,
) -> Result<(), CliFailure>
where
    R: BufRead,
    W: Write,
    E: Write,
{
    if !is_tty {
        // Message is printed once by the binary dispatcher on CliFailure.
        let _ = stderr;
        return Err(CliFailure::validation(
            "interactive update requires a TTY; use 'update check' and 'update apply <version>'",
        ));
    }

    match offer {
        InteractiveUpdateOffer::UpToDate => {
            writeln!(stdout, "Agent Bar is up to date.")
                .map_err(|err| CliFailure::internal(err.to_string()))?;
            Ok(())
        }
        InteractiveUpdateOffer::Available { current, target } => {
            writeln!(stdout, "Current version: {current}")
                .map_err(|err| CliFailure::internal(err.to_string()))?;
            writeln!(stdout, "Target version: {target}")
                .map_err(|err| CliFailure::internal(err.to_string()))?;
            write!(stderr, "Type update agent-bar to continue:")
                .map_err(|err| CliFailure::internal(err.to_string()))?;
            let _ = stderr.flush();
            let mut line = String::new();
            match stdin.read_line(&mut line) {
                Ok(0) => Err(CliFailure::validation("update confirmation aborted")),
                Ok(_) => {
                    let trimmed = line.trim_end_matches(['\r', '\n']);
                    if trimmed == "update agent-bar" {
                        Ok(())
                    } else {
                        Err(CliFailure::validation("update confirmation rejected"))
                    }
                }
                Err(err) => Err(CliFailure::internal(err.to_string())),
            }
        }
    }
}

/// Dispatch a fully parsed command for the private helper binary.
///
/// Commands not yet implemented after the grammar freeze exit with code 70.
pub fn dispatch(command: Command) -> Result<(), CliFailure> {
    match command {
        Command::Version => {
            print!("{}", version_stdout());
            Ok(())
        }
        Command::Help(topic) => {
            print!("{}", help_text(topic));
            Ok(())
        }
        Command::Setup(SetupOptions::PluginsDir(path)) => {
            validate_plugins_dir(&path)?;
            Err(CliFailure::internal(
                "setup is not implemented yet (later plugin transaction task)",
            ))
        }
        Command::Setup(SetupOptions::Production) => Err(CliFailure::internal(
            "setup is not implemented yet (later plugin transaction task)",
        )),
        Command::Update(UpdateCommand::Interactive) => {
            let is_tty = io::stdin().is_terminal();
            // Network-backed update discovery arrives in the bundle task.
            // Until then interactive update only enforces the TTY gate and the
            // up-to-date path when no offer seam is wired.
            let offer = InteractiveUpdateOffer::UpToDate;
            let stdin = io::stdin();
            let mut locked_in = stdin.lock();
            let stdout = io::stdout();
            let mut locked_out = stdout.lock();
            let stderr = io::stderr();
            let mut locked_err = stderr.lock();
            confirm_interactive_update(
                is_tty,
                offer,
                &mut locked_in,
                &mut locked_out,
                &mut locked_err,
            )
        }
        Command::Update(UpdateCommand::Check) | Command::Update(UpdateCommand::Apply(_)) => Err(
            CliFailure::internal("update check/apply is not implemented yet (later bundle task)"),
        ),
        Command::Config(config) => dispatch_config(config),
        Command::Status(_) | Command::Login(_) | Command::Uninstall { .. } | Command::Doctor(_) => {
            Err(CliFailure::internal("command is not implemented yet"))
        }
    }
}

fn dispatch_config(command: ConfigCommand) -> Result<(), CliFailure> {
    use crate::settings::{SettingsStore, StoreError};
    use std::io::Read;

    let store = SettingsStore::with_paths(
        crate::settings::store::default_settings_path(),
        crate::settings::store::default_maintenance_lock_path(),
    )
    .map_err(|err| CliFailure::validation(err.to_string()))?;

    let map_store_err = |err: StoreError| match err {
        StoreError::Validation(v) => CliFailure::validation(v.message().to_owned()),
        StoreError::Io(io_err) => CliFailure::validation(io_err.to_string()),
    };

    match command {
        ConfigCommand::Show => {
            let doc = store.show().map_err(map_store_err)?;
            let line = doc
                .to_canonical_json_line()
                .map_err(|err| CliFailure::validation(err.message().to_owned()))?;
            print!("{line}");
            Ok(())
        }
        ConfigCommand::Apply(input) => {
            let raw = match input {
                ConfigInput::Stdin => {
                    let mut buf = String::new();
                    io::stdin()
                        .read_to_string(&mut buf)
                        .map_err(|err| CliFailure::validation(err.to_string()))?;
                    buf
                }
                ConfigInput::File(path) => std::fs::read_to_string(&path)
                    .map_err(|err| CliFailure::validation(err.to_string()))?,
                ConfigInput::Json(value) => value,
            };
            let stored = store.apply_raw(raw.as_bytes()).map_err(map_store_err)?;
            let line = stored
                .to_canonical_json_line()
                .map_err(|err| CliFailure::validation(err.message().to_owned()))?;
            print!("{line}");
            Ok(())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    #[test]
    fn version_stdout_is_package_semver_plus_newline() {
        let out = version_stdout();
        assert_eq!(out, format!("{}\n", env!("CARGO_PKG_VERSION")));
        assert!(!out.contains('\0'));
    }

    #[test]
    fn interactive_update_rejects_non_tty() {
        let mut stdin = Cursor::new(Vec::new());
        let mut stdout = Vec::new();
        let mut stderr = Vec::new();
        let err = confirm_interactive_update(
            false,
            InteractiveUpdateOffer::Available {
                current: "10.0.0".into(),
                target: "10.0.1".into(),
            },
            &mut stdin,
            &mut stdout,
            &mut stderr,
        )
        .unwrap_err();
        assert_eq!(err.exit_code, VALIDATION);
        assert!(stdout.is_empty());
        assert!(stderr.is_empty());
        assert!(err.message.contains("update check"));
        assert!(err.message.contains("update apply"));
    }

    #[test]
    fn interactive_update_up_to_date_on_tty() {
        let mut stdin = Cursor::new(Vec::new());
        let mut stdout = Vec::new();
        let mut stderr = Vec::new();
        confirm_interactive_update(
            true,
            InteractiveUpdateOffer::UpToDate,
            &mut stdin,
            &mut stdout,
            &mut stderr,
        )
        .unwrap();
        assert_eq!(
            String::from_utf8_lossy(&stdout),
            "Agent Bar is up to date.\n"
        );
        assert!(stderr.is_empty());
    }

    #[test]
    fn interactive_update_accepts_exact_phrase() {
        let mut stdin = Cursor::new(b"update agent-bar\n".as_slice());
        let mut stdout = Vec::new();
        let mut stderr = Vec::new();
        confirm_interactive_update(
            true,
            InteractiveUpdateOffer::Available {
                current: "10.0.0".into(),
                target: "10.1.0".into(),
            },
            &mut stdin,
            &mut stdout,
            &mut stderr,
        )
        .unwrap();
        let out = String::from_utf8_lossy(&stdout);
        assert!(out.contains("10.0.0"));
        assert!(out.contains("10.1.0"));
        assert!(String::from_utf8_lossy(&stderr).contains("Type update agent-bar to continue:"));
    }

    #[test]
    fn interactive_update_rejects_wrong_phrase_and_eof() {
        let mut stdin = Cursor::new(b"nope\n".as_slice());
        let mut stdout = Vec::new();
        let mut stderr = Vec::new();
        let err = confirm_interactive_update(
            true,
            InteractiveUpdateOffer::Available {
                current: "10.0.0".into(),
                target: "10.1.0".into(),
            },
            &mut stdin,
            &mut stdout,
            &mut stderr,
        )
        .unwrap_err();
        assert_eq!(err.exit_code, VALIDATION);

        let mut stdin = Cursor::new(Vec::new());
        let err = confirm_interactive_update(
            true,
            InteractiveUpdateOffer::Available {
                current: "10.0.0".into(),
                target: "10.1.0".into(),
            },
            &mut stdin,
            &mut stdout,
            &mut stderr,
        )
        .unwrap_err();
        assert_eq!(err.exit_code, VALIDATION);
    }

    #[test]
    fn validate_plugins_dir_accepts_writable_parent() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().canonicalize().unwrap();
        let got = validate_plugins_dir(&path).unwrap();
        assert_eq!(got, path);
    }

    #[test]
    fn validate_plugins_dir_rejects_missing_and_plugin_root_name() {
        let missing = PathBuf::from("/tmp/agent-bar-missing-plugins-dir-xyz");
        let err = validate_plugins_dir(&missing).unwrap_err();
        assert_eq!(err.exit_code, VALIDATION);

        let dir = tempfile::tempdir().unwrap();
        let plugin_root = dir.path().join("agent-bar.usage");
        std::fs::create_dir_all(&plugin_root).unwrap();
        let err = validate_plugins_dir(&plugin_root).unwrap_err();
        assert_eq!(err.exit_code, GRAMMAR);
    }
}
