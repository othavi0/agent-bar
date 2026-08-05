//! Strict word-based CLI for the private Agent Bar helper.

mod command;
mod exit;
mod grammar;

pub use command::{
    CacheMode, Command, ConfigCommand, ConfigInput, DoctorCommand, HelpTopic, NotificationMode,
    ProviderId, SetupOptions, StatusFormat, StatusOptions, UpdateCommand,
};
pub use exit::{
    CliFailure, GENERIC_FAILURE, GRAMMAR, INTERNAL, PLUGIN, SERIALIZATION, SUCCESS, VALIDATION,
};
pub use grammar::parse;

use std::io::{self, BufRead, IsTerminal, Write};
use std::path::{Path, PathBuf};

use serde::Serialize;

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
            out.push_str("  agent-bar update [check|apply]\n");
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
             Arguments (any order, each at most once):\n\
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
        Some(HelpTopic::Update) => "update — print usage; no interactive flow\n\
             update check — report whether a newer release exists\n\
             update apply — delegate to 'omarchy plugin update agent-bar.usage'\n"
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
        return Err(CliFailure::grammar(grammar::SETUP_PLUGINS_DIR_ABSOLUTE));
    }
    if path
        .file_name()
        .and_then(|s| s.to_str())
        .is_some_and(|n| n == "agent-bar.usage")
    {
        return Err(CliFailure::grammar(
            grammar::SETUP_PLUGINS_DIR_NOT_PLUGIN_ROOT,
        ));
    }
    let meta = std::fs::metadata(path).map_err(|_| {
        CliFailure::validation(format!(
            "setup plugins-dir path cannot be read: {}; create it, or check the permissions on its parents",
            path.display()
        ))
    })?;
    if !meta.is_dir() {
        return Err(CliFailure::validation(format!(
            "setup plugins-dir path is not a directory: {}; pass the parent directory",
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

/// Three call sites reach this condition — a missing plugin root, a missing
/// helper, and a non-executable helper — and all three want the same sentence.
const SETUP_REQUIRES_PLUGIN_TREE: &str =
    "setup requires a complete plugin tree at <plugin-root>/bin/agent-bar; \
     use install.sh for first bootstrap from a release archive";

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
        Command::Setup(options) => dispatch_setup(options),
        Command::Update(UpdateCommand::Interactive) => dispatch_update_interactive(),
        Command::Update(UpdateCommand::Check) => dispatch_update_check(),
        Command::Update(UpdateCommand::Apply) => dispatch_update_apply(),
        Command::Config(config) => dispatch_config(config),
        Command::Login(provider) => dispatch_login(provider),
        Command::Status(opts) => dispatch_status(opts),
        Command::Uninstall { purge } => dispatch_uninstall(purge),
        Command::Doctor(cmd) => dispatch_doctor(cmd),
    }
}

/// Resolve the complete plugin tree that contains this helper binary.
///
/// Expects the installed layout `<plugin-root>/bin/agent-bar`. First bootstrap
/// from a release archive remains `install.sh` when no local tree exists.
fn resolve_plugin_source_root() -> Result<PathBuf, CliFailure> {
    use crate::plugin::BundleValidator;

    let exe =
        std::env::current_exe().map_err(|e| CliFailure::plugin(format!("current_exe: {e}")))?;
    let exe = fs_canonicalize(&exe);
    let Some(bin_dir) = exe.parent() else {
        return Err(CliFailure::plugin(SETUP_REQUIRES_PLUGIN_TREE));
    };
    if bin_dir
        .file_name()
        .and_then(|s| s.to_str())
        .is_none_or(|n| n != "bin")
    {
        return Err(CliFailure::plugin(SETUP_REQUIRES_PLUGIN_TREE));
    }
    let Some(root) = bin_dir.parent() else {
        return Err(CliFailure::plugin(SETUP_REQUIRES_PLUGIN_TREE));
    };
    if !root.join("manifest.json").is_file() {
        return Err(CliFailure::plugin(
            "setup source tree is missing manifest.json",
        ));
    }
    if root.join("bundle.json").is_file() {
        BundleValidator::validate_tree(root).map_err(|e| CliFailure::plugin(e.to_string()))?;
    }
    Ok(root.to_path_buf())
}

fn fs_canonicalize(path: &Path) -> PathBuf {
    std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf())
}

fn paths_equal(a: &Path, b: &Path) -> bool {
    fs_canonicalize(a) == fs_canonicalize(b)
}

fn dispatch_setup(options: SetupOptions) -> Result<(), CliFailure> {
    use crate::plugin::{
        resolve_absolute_executable, shell_has_plugin_entry, txid_from_bytes, OmarchyClient,
        PluginPaths, ProcessCommandRunner, Transaction,
    };
    use crate::settings::{default_settings_path, migrate_live_paths};
    use crate::support::maintenance_gate::MaintenanceGate;
    use crate::support::{Clock, SystemClock};

    let home = std::env::var_os("HOME")
        .ok_or_else(|| CliFailure::plugin("HOME is required for setup".to_string()))?;
    let home = PathBuf::from(home);
    let xdg_state = std::env::var_os("XDG_STATE_HOME").map(PathBuf::from);

    let (paths, is_production) = match options {
        SetupOptions::Production => (PluginPaths::production(home.clone(), xdg_state), true),
        SetupOptions::PluginsDir(path) => {
            let parent = validate_plugins_dir(&path)?;
            let state = std::env::var_os("XDG_STATE_HOME")
                .map(PathBuf::from)
                .unwrap_or_else(|| home.join(".local/state"));
            (
                PluginPaths::with_plugins_dir(home.clone(), parent, state),
                false,
            )
        }
    };

    let clock = SystemClock;
    let stamp = format!("{}", Clock::now_utc(&clock));
    let backup_stamp = stamp.replace(':', "-");

    // MIG-007..016: explicit settings/shell migration under exclusive gate before
    // plugin swap. Reads never write; setup is the authorized apply path.
    {
        let gate = MaintenanceGate::open(&paths.maintenance_lock)
            .map_err(|e| CliFailure::plugin(format!("open maintenance lock: {e}")))?;
        let _exclusive = gate
            .lock_exclusive()
            .map_err(|e| CliFailure::plugin(format!("exclusive maintenance lock: {e}")))?;
        let settings_path = default_settings_path();
        let shell_path = home.join(".config/omarchy/shell.json");
        let migrate_backup = paths.backup_root(&format!("setup-migrate-{backup_stamp}"));
        let report = migrate_live_paths(&settings_path, &shell_path, &migrate_backup)
            .map_err(|e| CliFailure::plugin(e.to_string()))?;
        if report.already_migrated {
            eprintln!("settings already at v10; migration skipped");
        } else if report.settings_written {
            eprintln!(
                "migrated settings to v10 (shell_written={})",
                report.shell_written
            );
            if !report.unknown_keys.is_empty() {
                eprintln!(
                    "legacy keys retained in backup only: {}",
                    report.unknown_keys.join(", ")
                );
            }
            if let Some(root) = report.backup_root {
                eprintln!("migration backup: {}", root.display());
            }
        }
    }

    let source = resolve_plugin_source_root()?;
    if !paths_equal(&source, &paths.plugin_root) {
        std::fs::create_dir_all(&paths.plugins_dir)
            .map_err(|e| CliFailure::plugin(format!("create plugins dir: {e}")))?;
        let gate = MaintenanceGate::open(&paths.maintenance_lock)
            .map_err(|e| CliFailure::plugin(format!("open maintenance lock: {e}")))?;
        let txid = txid_from_bytes(format!("setup:{stamp}").as_bytes());
        let mut tx = Transaction::begin(&paths, &gate, &txid, "setup", &backup_stamp)
            .map_err(|e| CliFailure::plugin(e.to_string()))?;
        let report = tx
            .replace_plugin_dir(&source)
            .map_err(|e| CliFailure::plugin(e.to_string()))?;
        if !report.ok {
            return Err(CliFailure::plugin(report.message));
        }
        eprintln!("plugin installed at {}", paths.plugin_root.display());
    } else {
        eprintln!("plugin already present at {}", paths.plugin_root.display());
    }

    // Production setup activates Quattro placement. Injected plugins-dir is
    // tree-only (CLI-009 isolated testing) and never runs omarchy.
    if is_production {
        let omarchy_bin = resolve_absolute_executable("omarchy")
            .map_err(|e| CliFailure::plugin(e.to_string()))?;
        let client = OmarchyClient::new(ProcessCommandRunner).with_program(omarchy_bin);
        let shell_json = home.join(".config/omarchy/shell.json");
        let has_entry = shell_has_plugin_entry(&shell_json);
        client
            .activate(has_entry)
            .map_err(|e| CliFailure::plugin(e.to_string()))?;
        if has_entry {
            eprintln!("omarchy plugin rescan completed");
        } else {
            eprintln!("omarchy plugin enable agent-bar.usage completed");
        }
    }
    Ok(())
}

fn dispatch_doctor(cmd: DoctorCommand) -> Result<(), CliFailure> {
    use crate::plugin::{default_ownership_rules, doctor_clean, doctor_scan, PluginPaths};
    use crate::support::{Clock, SystemClock};

    let home = std::env::var_os("HOME")
        .ok_or_else(|| CliFailure::plugin("HOME is required for doctor".to_string()))?;
    let home = PathBuf::from(home);
    let rules = default_ownership_rules(&home);

    match cmd {
        DoctorCommand::Scan => {
            let report = doctor_scan(&home, &[], &rules);
            print_doctor_report("scan", &report);
            Ok(())
        }
        DoctorCommand::Clean => {
            let xdg_state = std::env::var_os("XDG_STATE_HOME").map(PathBuf::from);
            let paths = PluginPaths::production(home.clone(), xdg_state);
            let clock = SystemClock;
            let stamp = format!("{}", Clock::now_utc(&clock)).replace(':', "-");
            let backup = paths.backup_root(&format!("doctor-clean-{stamp}"));
            let report = doctor_clean(&home, &[], &rules, &backup)
                .map_err(|e| CliFailure::plugin(e.to_string()))?;
            print_doctor_report("clean", &report);
            Ok(())
        }
    }
}

fn print_doctor_report(mode: &str, report: &crate::plugin::DoctorReport) {
    println!("Agent Bar doctor {mode}");
    println!(
        "mode: {}",
        if report.read_only {
            "read-only"
        } else {
            "clean"
        }
    );
    println!("findings: {}", report.findings.len());
    for ev in &report.findings {
        println!(
            "  [{}] {} — {}",
            ev.class.as_str(),
            ev.path.display(),
            ev.reason
        );
    }
    println!("removable (owned/legacy): {}", report.removable.len());
    for path in &report.removable {
        println!("  {}", path.display());
    }
    println!("retained (modified/ambiguous): {}", report.retained.len());
    for path in &report.retained {
        println!("  {}", path.display());
    }
    if !report.read_only {
        println!("removed: {}", report.removed.len());
        for path in &report.removed {
            println!("  {}", path.display());
        }
        if let Some(backup) = &report.backup_root {
            println!("backup: {}", backup.display());
        }
    }
}

/// Pure uninstall confirmation gate (TTY phrase or non-TTY structured JSON).
///
/// Standard uninstall does not read stdin until preflight has already succeeded
/// (caller responsibility). Exit code 3 on any confirmation failure; zero mutation
/// happens inside this function.
pub fn confirm_uninstall<R, E>(
    is_tty: bool,
    purge: bool,
    stdin: &mut R,
    stderr: &mut E,
) -> Result<(), CliFailure>
where
    R: BufRead,
    E: Write,
{
    use crate::plugin::{UninstallConfirmation, UNINSTALL_TTY_PHRASE, UNINSTALL_TTY_PROMPT};

    if is_tty {
        write!(stderr, "{UNINSTALL_TTY_PROMPT}")
            .map_err(|err| CliFailure::internal(err.to_string()))?;
        let _ = stderr.flush();
        let mut line = String::new();
        match stdin.read_line(&mut line) {
            Ok(0) => Err(CliFailure::validation("uninstall confirmation aborted")),
            Ok(_) => {
                let trimmed = line.trim_end_matches(['\r', '\n']);
                if trimmed == UNINSTALL_TTY_PHRASE {
                    Ok(())
                } else {
                    Err(CliFailure::validation("uninstall confirmation rejected"))
                }
            }
            Err(err) => Err(CliFailure::internal(err.to_string())),
        }
    } else {
        let mut buf = Vec::new();
        stdin
            .read_to_end(&mut buf)
            .map_err(|err| CliFailure::internal(err.to_string()))?;
        UninstallConfirmation::parse_strict(&buf, purge)
            .map_err(|err| CliFailure::validation(err.to_string()))?;
        Ok(())
    }
}

/// Exact successful `uninstall` stdout document (git-plugin-distribution
/// Task 3). Own-state purge only — the plugin tree, shell.json entry, and
/// cache/backups GC that the old worker chain owned all belong to
/// `omarchy plugin remove` now.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct UninstallDelegation<'a> {
    schema_version: u32,
    operation: &'static str,
    purged: bool,
    delegated: bool,
    unit: &'a str,
}

/// Remove `path` and everything under it; a missing path is success
/// (idempotent purge), any other I/O error propagates.
fn remove_dir_all_idempotent(path: &Path) -> Result<(), CliFailure> {
    match std::fs::remove_dir_all(path) {
        Ok(()) => Ok(()),
        Err(err) if err.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(err) => Err(CliFailure::plugin(format!(
            "remove {}: {err}",
            path.display()
        ))),
    }
}

/// `uninstall [purge]`: own-XDG-state purge under the maintenance gate, then
/// unconditional detached delegation to the omarchy CLI (git-plugin-
/// distribution Task 3).
///
/// The old worker chain quarantined the plugin tree, stripped the exact
/// shell.json entry, and polled for absence itself over a copied worker
/// binary — none of that survives git-native distribution: `omarchy plugin
/// remove` owns the plugin tree and shell.json now, and this helper only
/// purges the state it exclusively owns (settings, cache, XDG state)
/// before handing off, mirroring `update apply`'s Task 2 delegation shape.
fn dispatch_uninstall(purge: bool) -> Result<(), CliFailure> {
    use crate::plugin::{
        resolve_absolute_executable, txid_from_bytes, CommandRunner, PluginPaths,
        ProcessCommandRunner,
    };
    use crate::settings::default_settings_path;
    use crate::support::maintenance_gate::MaintenanceGate;
    use crate::support::{Clock, SystemClock};

    let home = std::env::var_os("HOME")
        .ok_or_else(|| CliFailure::plugin("HOME is required for uninstall".to_string()))?;
    let home = PathBuf::from(home);
    let xdg_state = std::env::var_os("XDG_STATE_HOME").map(PathBuf::from);
    let paths = PluginPaths::production(home.clone(), xdg_state);

    // Exclusive barrier (ARCH-026), mirroring dispatch_update_apply (Task 2):
    // block shared status/settings and any other maintenance operation while
    // the purge and handoff run.
    let gate = MaintenanceGate::open(&paths.maintenance_lock)
        .map_err(|e| CliFailure::plugin(format!("open maintenance lock: {e}")))?;
    let exclusive = gate
        .lock_exclusive()
        .map_err(|e| CliFailure::plugin(format!("exclusive maintenance lock: {e}")))?;

    let is_tty = io::stdin().is_terminal();
    let stdin = io::stdin();
    let mut locked_in = stdin.lock();
    let stderr = io::stderr();
    let mut locked_err = stderr.lock();
    confirm_uninstall(is_tty, purge, &mut locked_in, &mut locked_err)?;

    if purge {
        let settings_dir = default_settings_path()
            .parent()
            .map(Path::to_path_buf)
            .ok_or_else(|| {
                CliFailure::plugin("settings path has no parent directory".to_string())
            })?;
        let cache_dir = {
            let base = std::env::var_os("XDG_CACHE_HOME")
                .map(PathBuf::from)
                .unwrap_or_else(|| home.join(".cache"));
            base.join("agent-bar")
        };
        remove_dir_all_idempotent(&settings_dir)?;
        remove_dir_all_idempotent(&cache_dir)?;
    }

    // `paths.xdg_state` ($XDG_STATE_HOME/agent-bar) holds `maintenance.lock`
    // itself. Drop the exclusive guard before removing that directory:
    // unlinking a file while its flock is still held is safe on Linux, but
    // the drop-then-remove order is kept explicit — and covered by the
    // `uninstall_purge_removes_xdg_state_and_delegates_remove` test — rather
    // than relying on that platform detail.
    drop(exclusive);
    if purge {
        remove_dir_all_idempotent(&paths.xdg_state)?;
    }

    let omarchy_bin =
        resolve_absolute_executable("omarchy").map_err(|e| CliFailure::plugin(e.to_string()))?;
    let systemd_run = resolve_absolute_executable("systemd-run")
        .map_err(|e| CliFailure::plugin(e.to_string()))?;

    let clock = SystemClock;
    let txid = txid_from_bytes(format!("uninstall:{}", Clock::now_utc(&clock)).as_bytes());
    let unit = format!("agent-bar-remove-{txid}.service");
    let unit_flag = format!("--unit={unit}");

    let argv: [&str; 9] = [
        "--user",
        "--collect",
        unit_flag.as_str(),
        "--",
        omarchy_bin.as_str(),
        "plugin",
        "remove",
        "agent-bar.usage",
        "--yes",
    ];

    let runner = ProcessCommandRunner;
    let out = runner
        .run(&systemd_run, &argv)
        .map_err(|e| CliFailure::plugin(e.to_string()))?;
    if out.code != 0 {
        return Err(CliFailure::plugin(format!(
            "failed to start remove unit: {}",
            out.stderr.trim()
        )));
    }

    let doc = UninstallDelegation {
        schema_version: 1,
        operation: "uninstall",
        purged: purge,
        delegated: true,
        unit: &unit,
    };
    let json = serde_json::to_string(&doc).map_err(|e| CliFailure::plugin(e.to_string()))?;
    println!("{json}");
    Ok(())
}

/// True when the live plugin root is not a git checkout (BUNDLE-021 v-next):
/// `omarchy plugin update` can only fast-forward a git-managed install, so a
/// tarball-installed tree must be reinstalled via `omarchy plugin add`.
fn reinstall_required() -> Result<bool, CliFailure> {
    use crate::plugin::PluginPaths;

    let home = std::env::var_os("HOME")
        .ok_or_else(|| CliFailure::plugin("HOME is required for update check".to_string()))?;
    let xdg_state = std::env::var_os("XDG_STATE_HOME").map(PathBuf::from);
    let paths = PluginPaths::production(PathBuf::from(home), xdg_state);
    Ok(!paths.plugin_root.join(".git").is_dir())
}

fn dispatch_update_check() -> Result<(), CliFailure> {
    use crate::plugin::{ReqwestReleaseHttp, UpdateCheck, UpdateCheckProbe};
    use crate::support::SystemClock;

    let http = ReqwestReleaseHttp::new().map_err(|e| CliFailure::plugin(e.to_string()))?;
    let clock = SystemClock;
    let probe = UpdateCheckProbe::default();
    let doc = UpdateCheck::run(&http, &clock, &probe, reinstall_required()?)
        .map_err(|e| CliFailure::plugin(e.to_string()))?;
    let json = doc
        .to_stdout_json()
        .map_err(|e| CliFailure::plugin(e.to_string()))?;
    print!("{json}");
    Ok(())
}

/// Exact successful `update apply` stdout document (git-plugin-distribution
/// Task 2). `update apply` no longer downloads, stages, or swaps the plugin
/// tree itself — it hands the whole fast-forward to
/// `omarchy plugin update agent-bar.usage --yes`, running as a detached
/// transient unit so this process can return as soon as the handoff is
/// accepted.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct UpdateApplyDelegation<'a> {
    schema_version: u32,
    operation: &'static str,
    delegated: bool,
    unit: &'a str,
}

/// `update apply`: unconditional detached delegation to the omarchy CLI.
///
/// BUNDLE-021 v-next carries no archive/checksum/source-commit fields (Task
/// 1), so the old download/stage/exchange/health worker chain cannot run
/// anymore — `omarchy plugin update` owns the git fast-forward instead.
fn dispatch_update_apply() -> Result<(), CliFailure> {
    use crate::plugin::{
        resolve_absolute_executable, txid_from_bytes, CommandRunner, PluginPaths,
        ProcessCommandRunner,
    };
    use crate::support::maintenance_gate::MaintenanceGate;
    use crate::support::{Clock, SystemClock};

    let home = std::env::var_os("HOME")
        .ok_or_else(|| CliFailure::plugin("HOME is required for update apply".to_string()))?;
    let xdg_state = std::env::var_os("XDG_STATE_HOME").map(PathBuf::from);
    let paths = PluginPaths::production(PathBuf::from(home), xdg_state);

    // Exclusive barrier (ARCH-026): block shared status/settings and any other
    // maintenance operation while the handoff is issued.
    let gate = MaintenanceGate::open(&paths.maintenance_lock)
        .map_err(|e| CliFailure::plugin(format!("open maintenance lock: {e}")))?;
    let _exclusive = gate
        .lock_exclusive()
        .map_err(|e| CliFailure::plugin(format!("exclusive maintenance lock: {e}")))?;

    let omarchy_bin =
        resolve_absolute_executable("omarchy").map_err(|e| CliFailure::plugin(e.to_string()))?;
    let systemd_run = resolve_absolute_executable("systemd-run")
        .map_err(|e| CliFailure::plugin(e.to_string()))?;

    let clock = SystemClock;
    let txid = txid_from_bytes(format!("update-apply:{}", Clock::now_utc(&clock)).as_bytes());
    let unit = format!("agent-bar-update-{txid}.service");
    let unit_flag = format!("--unit={unit}");

    let argv: [&str; 9] = [
        "--user",
        "--collect",
        unit_flag.as_str(),
        "--",
        omarchy_bin.as_str(),
        "plugin",
        "update",
        "agent-bar.usage",
        "--yes",
    ];

    let runner = ProcessCommandRunner;
    let out = runner
        .run(&systemd_run, &argv)
        .map_err(|e| CliFailure::plugin(e.to_string()))?;
    if out.code != 0 {
        return Err(CliFailure::plugin(format!(
            "failed to start update unit: {}",
            out.stderr.trim()
        )));
    }

    let doc = UpdateApplyDelegation {
        schema_version: 1,
        operation: "updateApply",
        delegated: true,
        unit: &unit,
    };
    let json = serde_json::to_string(&doc).map_err(|e| CliFailure::plugin(e.to_string()))?;
    println!("{json}");
    Ok(())
}

/// `update` (no subcommand): the old TTY confirmation flow required a
/// version-gated apply to offer, which git-native delegation no longer has —
/// `update apply` now applies unconditionally. Bare `update` just points
/// callers at the two real subcommands instead of pretending to be
/// interactive.
fn dispatch_update_interactive() -> Result<(), CliFailure> {
    eprintln!("agent-bar update has no interactive flow.");
    eprintln!("Use 'agent-bar update check' or 'agent-bar update apply'.");
    Err(CliFailure {
        message: String::new(),
        exit_code: VALIDATION,
    })
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
                "{} login executable was not found; install the provider CLI first",
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
    fn validate_plugins_dir_accepts_writable_parent() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().canonicalize().unwrap();
        let got = validate_plugins_dir(&path).unwrap();
        assert_eq!(got, path);
    }

    #[test]
    fn uninstall_tty_accepts_exact_phrase() {
        let mut stdin = Cursor::new(b"uninstall agent-bar\n".as_slice());
        let mut stderr = Vec::new();
        confirm_uninstall(true, false, &mut stdin, &mut stderr).unwrap();
        assert!(String::from_utf8_lossy(&stderr).contains("Type uninstall agent-bar to continue:"));
    }

    #[test]
    fn uninstall_tty_rejects_wrong_phrase_and_eof() {
        let mut stdin = Cursor::new(b"nope\n".as_slice());
        let mut stderr = Vec::new();
        let err = confirm_uninstall(true, false, &mut stdin, &mut stderr).unwrap_err();
        assert_eq!(err.exit_code, VALIDATION);

        let mut stdin = Cursor::new(Vec::new());
        let err = confirm_uninstall(true, true, &mut stdin, &mut stderr).unwrap_err();
        assert_eq!(err.exit_code, VALIDATION);
    }

    #[test]
    fn uninstall_json_confirmation_matrix() {
        let good = br#"{"schemaVersion":1,"operation":"uninstall","confirmed":true,"purgeSettingsAndBackups":false}"#;
        let mut stdin = Cursor::new(good.as_slice());
        let mut stderr = Vec::new();
        confirm_uninstall(false, false, &mut stdin, &mut stderr).unwrap();

        // command/purge mismatch
        let mut stdin = Cursor::new(good.as_slice());
        let err = confirm_uninstall(false, true, &mut stdin, &mut stderr).unwrap_err();
        assert_eq!(err.exit_code, VALIDATION);

        // false confirmation
        let bad = br#"{"schemaVersion":1,"operation":"uninstall","confirmed":false,"purgeSettingsAndBackups":false}"#;
        let mut stdin = Cursor::new(bad.as_slice());
        let err = confirm_uninstall(false, false, &mut stdin, &mut stderr).unwrap_err();
        assert_eq!(err.exit_code, VALIDATION);

        // malformed
        let mut stdin = Cursor::new(b"{not-json".as_slice());
        let err = confirm_uninstall(false, false, &mut stdin, &mut stderr).unwrap_err();
        assert_eq!(err.exit_code, VALIDATION);

        // trailing garbage
        let mut stdin = Cursor::new(
            br#"{"schemaVersion":1,"operation":"uninstall","confirmed":true,"purgeSettingsAndBackups":false}{}"#
                .as_slice(),
        );
        let err = confirm_uninstall(false, false, &mut stdin, &mut stderr).unwrap_err();
        assert_eq!(err.exit_code, VALIDATION);
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
