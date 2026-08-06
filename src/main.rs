//! Private Agent Bar helper binary (v10 CLI surface).

#![cfg_attr(not(test), deny(clippy::unwrap_used, clippy::expect_used))]

use agent_bar::cli::{self, SUCCESS};

fn main() {
    // RUST_LOG controls diagnostics (CLI-008); no verbose flag. Stderr only.
    let _ = env_logger::Builder::new()
        .filter_level(log::LevelFilter::Warn)
        .target(env_logger::Target::Stderr)
        .try_init();

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
