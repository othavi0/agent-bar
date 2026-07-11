//! Suporte compartilhado para testes que tocam env de processo sensível a
//! mutação concorrente (PATH). `#[cfg(test)]`-only: nunca compilado em release.

/// Serializa testes que MUTAM ou LEEM env sensível a mutação (PATH).
/// `std::env::set_var` é process-wide; dois testes em paralelo — um
/// mutando, outro lendo — flakam. Envenenamento é ignorado de propósito
/// (um teste que panicou não deve derrubar os vizinhos).
pub static ENV_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

pub fn env_guard() -> std::sync::MutexGuard<'static, ()> {
    ENV_LOCK.lock().unwrap_or_else(|p| p.into_inner())
}
