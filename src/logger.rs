//! Logging exclusivamente para stderr. stdout é reservado para payload de máquina.

/// Inicializa o logger global em stderr. `try_init` evita panic em re-init (testes).
pub fn init(verbose: bool) {
    let level = if verbose {
        log::LevelFilter::Debug
    } else {
        log::LevelFilter::Warn
    };
    let _ = env_logger::Builder::new()
        .filter_level(level)
        .target(env_logger::Target::Stderr)
        .try_init();
}
