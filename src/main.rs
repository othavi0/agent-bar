//! Private Agent Bar helper binary (v10 CLI surface).

#![cfg_attr(not(test), deny(clippy::unwrap_used, clippy::expect_used))]

use std::path::PathBuf;
use std::time::Duration;

use agent_bar::cli::{self, SUCCESS};
use agent_bar::plugin::maintenance::RealSleeper;
use agent_bar::plugin::{
    is_maintenance_worker_exe, MaintenanceWorker, PluginPaths, ProcessCommandRunner,
};

fn main() {
    // RUST_LOG controls diagnostics (CLI-008); no verbose flag.
    agent_bar::logger::init(false);

    // Worker mode is selected by the copied executable basename before public
    // CLI parsing (BUNDLE-032C).
    if let Ok(exe) = std::env::current_exe() {
        if is_maintenance_worker_exe(&exe) {
            let code = run_maintenance_worker();
            std::process::exit(code);
        }
    }

    let args: Vec<String> = std::env::args().skip(1).collect();
    let command = match cli::parse(&args) {
        Ok(command) => command,
        Err(failure) => {
            eprintln!("{}", failure.message);
            std::process::exit(failure.exit_code);
        }
    };

    if let Err(failure) = cli::dispatch(command) {
        if !failure.message.is_empty() {
            eprintln!("{}", failure.message);
        }
        std::process::exit(failure.exit_code);
    }

    std::process::exit(SUCCESS);
}

fn run_maintenance_worker() -> i32 {
    let mut args = std::env::args().skip(1);
    let txid = match args.next() {
        Some(t) => t,
        None => {
            eprintln!("maintenance worker requires journal transaction id");
            return 5;
        }
    };
    if args.next().is_some() {
        eprintln!("maintenance worker accepts exactly one argv (txid)");
        return 2;
    }

    let home = match std::env::var_os("HOME") {
        Some(h) => PathBuf::from(h),
        None => {
            eprintln!("HOME is required for maintenance worker");
            return 5;
        }
    };
    let xdg_state = std::env::var_os("XDG_STATE_HOME").map(PathBuf::from);
    let paths = PluginPaths::production(home, xdg_state);
    let runner = ProcessCommandRunner;
    let sleeper = RealSleeper;
    let start = Duration::from_secs(0);
    // Wall-clock approximation for monotonic deadlines inside the worker unit
    // (systemd also enforces RuntimeMaxSec).
    let start_instant = std::time::Instant::now();

    match MaintenanceWorker::run_worker_from_journal(
        &paths,
        &runner,
        "omarchy-shell",
        &txid,
        &sleeper,
        start,
        &|| start + start_instant.elapsed(),
        None,
    ) {
        Ok(()) => 0,
        Err(err) => {
            eprintln!("maintenance worker failed: {err}");
            5
        }
    }
}
