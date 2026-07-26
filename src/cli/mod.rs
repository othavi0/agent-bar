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
        Command::Update(UpdateCommand::Interactive) => dispatch_update_interactive(),
        Command::Update(UpdateCommand::Check) => dispatch_update_check(),
        Command::Update(UpdateCommand::Apply(version)) => dispatch_update_apply(&version.as_str()),
        Command::Config(config) => dispatch_config(config),
        Command::Login(provider) => dispatch_login(provider),
        Command::Status(opts) => dispatch_status(opts),
        Command::Uninstall { .. } | Command::Doctor(_) => {
            Err(CliFailure::internal("command is not implemented yet"))
        }
    }
}

fn dispatch_update_check() -> Result<(), CliFailure> {
    use crate::plugin::{ReqwestReleaseHttp, UpdateCheck, UpdateCheckProbe};
    use crate::support::{Clock, SystemClock};

    let http = ReqwestReleaseHttp::new().map_err(|e| CliFailure::plugin(e.to_string()))?;
    let clock = SystemClock;
    let probe = UpdateCheckProbe::default();
    let doc =
        UpdateCheck::run(&http, &clock, &probe).map_err(|e| CliFailure::plugin(e.to_string()))?;
    let json = doc
        .to_stdout_json()
        .map_err(|e| CliFailure::plugin(e.to_string()))?;
    print!("{json}");
    let _ = Clock::now_utc(&clock);
    Ok(())
}

fn dispatch_update_apply(version: &str) -> Result<(), CliFailure> {
    use crate::plugin::{
        apply_version_allowed, collect_worker_env, download_with_policy, stage_update_bundle,
        txid_from_bytes, MaintenanceJournalPayload, MaintenanceOp, MaintenanceWorker, PluginPaths,
        ProcessCommandRunner, ReqwestReleaseHttp, UpdateCheck, UpdateCheckProbe,
        WORKER_ENV_ALLOWLIST,
    };
    use crate::support::{Clock, SystemClock};

    let http = ReqwestReleaseHttp::new().map_err(|e| CliFailure::plugin(e.to_string()))?;
    let clock = SystemClock;
    let probe = UpdateCheckProbe::default();
    // Fresh check required before apply (BUNDLE-022).
    let doc =
        UpdateCheck::run(&http, &clock, &probe).map_err(|e| CliFailure::plugin(e.to_string()))?;
    let selected =
        apply_version_allowed(&doc, version).map_err(|e| CliFailure::validation(e.to_string()))?;

    let home = std::env::var_os("HOME")
        .ok_or_else(|| CliFailure::plugin("HOME is required for update apply".to_string()))?;
    let xdg_state = std::env::var_os("XDG_STATE_HOME").map(PathBuf::from);
    let paths = PluginPaths::production(PathBuf::from(home), xdg_state);

    let txid = txid_from_bytes(
        format!(
            "update:{}:{}:{}",
            version,
            selected.source_commit,
            Clock::now_utc(&clock)
        )
        .as_bytes(),
    );

    let archive =
        UpdateCheck::download_archive(&http, &selected.archive_url, &selected.archive_sha256)
            .map_err(|e| CliFailure::plugin(e.to_string()))?;
    // Corroborating checksum sidecar (not a substitute for the pinned hash).
    if let Ok(side) = download_with_policy(&http, &selected.checksum_url) {
        let text = String::from_utf8_lossy(&side);
        if !text.contains(&selected.archive_sha256) {
            return Err(CliFailure::plugin(
                "checksum sidecar does not corroborate pinned archive hash",
            ));
        }
    }

    let (stage, receipt) = stage_update_bundle(&paths, &txid, &archive)
        .map_err(|e| CliFailure::plugin(e.to_string()))?;
    if receipt.version != version {
        return Err(CliFailure::plugin(format!(
            "staged version {} does not equal requested {version}",
            receipt.version
        )));
    }

    let current_exe =
        std::env::current_exe().map_err(|e| CliFailure::plugin(format!("current_exe: {e}")))?;
    let previous = probe.current_version.clone();
    let payload = MaintenanceJournalPayload {
        txid: txid.clone(),
        operation: MaintenanceOp::Update,
        expected_version: Some(version.to_string()),
        previous_version: Some(previous),
        stage_path: stage.display().to_string(),
        plugin_root: paths.plugin_root.display().to_string(),
        quarantine_path: paths
            .quarantine_dir(&txid)
            .map_err(|e| CliFailure::plugin(e.to_string()))?
            .display()
            .to_string(),
        selected: Some(selected),
        omarchy_bin: "omarchy".into(),
        omarchy_shell_bin: "omarchy-shell".into(),
        is_fresh_install: !paths.plugin_root.exists(),
        is_v9_rollback: false,
    };

    let env_pairs = collect_worker_env(
        std::env::vars().filter(|(k, _)| WORKER_ENV_ALLOWLIST.contains(&k.as_str())),
    );
    let runner = ProcessCommandRunner;
    let unit = MaintenanceWorker::handoff_update(
        &paths,
        &runner,
        &current_exe,
        &txid,
        &payload,
        &env_pairs,
        "systemd-run",
    )
    .map_err(|e| CliFailure::plugin(e.to_string()))?;
    eprintln!("maintenance handoff accepted: {unit}");
    Ok(())
}

fn dispatch_update_interactive() -> Result<(), CliFailure> {
    use crate::plugin::{ReqwestReleaseHttp, UpdateCheck, UpdateCheckProbe};
    use crate::support::SystemClock;

    let is_tty = io::stdin().is_terminal();
    let http = ReqwestReleaseHttp::new().map_err(|e| CliFailure::plugin(e.to_string()))?;
    let clock = SystemClock;
    let probe = UpdateCheckProbe::default();
    let doc =
        UpdateCheck::run(&http, &clock, &probe).map_err(|e| CliFailure::plugin(e.to_string()))?;

    let offer = if doc.available {
        let latest = doc
            .latest_compatible
            .as_ref()
            .ok_or_else(|| CliFailure::plugin("available without latestCompatible"))?;
        InteractiveUpdateOffer::Available {
            current: doc.current.version.clone(),
            target: latest.version.clone(),
        }
    } else {
        InteractiveUpdateOffer::UpToDate
    };

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
    )?;

    if let Some(latest) = doc.latest_compatible {
        if doc.available {
            return dispatch_update_apply(&latest.version);
        }
    }
    Ok(())
}

fn dispatch_status(opts: StatusOptions) -> Result<(), CliFailure> {
    use crate::settings::default_maintenance_lock_path;
    use crate::status::{format_human, CollectRequest, StatusCoordinator};
    use crate::support::maintenance_gate::shared_gate;

    let gate = shared_gate(default_maintenance_lock_path())
        .map_err(|err| CliFailure::internal(err.to_string()))?;
    let coordinator = StatusCoordinator::production(gate).map_err(CliFailure::internal)?;
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|err| CliFailure::internal(err.to_string()))?;
    let envelope = runtime
        .block_on(coordinator.collect(CollectRequest {
            format: opts.format,
            provider: opts.provider,
            cache: opts.cache,
            notifications: opts.notifications,
        }))
        .map_err(|err| CliFailure {
            message: err.to_string(),
            exit_code: SERIALIZATION,
        })?;

    match opts.format {
        StatusFormat::Json => {
            let line = envelope.to_json_line().map_err(|err| CliFailure {
                message: err.message().to_owned(),
                exit_code: err.exit_code(),
            })?;
            print!("{line}");
        }
        StatusFormat::Human => {
            print!("{}", format_human(&envelope));
        }
    }
    Ok(())
}

fn dispatch_login(provider: ProviderId) -> Result<(), CliFailure> {
    use crate::providers::adapter::run_login;
    use crate::providers::{adapter_for, ExecutionEnvironment, TokioProcessRunner};

    let adapter = adapter_for(provider);
    let env = ExecutionEnvironment::from_process();
    let discovery = adapter
        .discover(&env)
        .map_err(|err| CliFailure::validation(err.to_string()))?;
    if discovery.login_executable().is_none() {
        return Err(CliFailure {
            message: format!(
                "{} login executable was not found",
                adapter.descriptor().display_name
            ),
            exit_code: GENERIC_FAILURE,
        });
    }

    let runner = TokioProcessRunner;
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|err| CliFailure::internal(err.to_string()))?;
    let outcome = runtime
        .block_on(run_login(adapter, &discovery, &runner, &runner))
        .map_err(|err| CliFailure {
            message: err.to_string(),
            exit_code: GENERIC_FAILURE,
        })?;
    if outcome.exit_code == 0 {
        Ok(())
    } else {
        Err(CliFailure {
            message: String::new(),
            exit_code: outcome.exit_code,
        })
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
