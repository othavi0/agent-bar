//! Detecção de tipo de instalação. Port de `src/runtime.ts` (`isCompiledBinary`),
//! adaptado: no Rust tudo é compilado, então a distinção é "sistema (AUR) vs dev/managed".

/// `true` quando rodando como instalação de sistema (AUR/pacote).
/// Seam de teste: `AGENT_BAR_FORCE_COMPILED=1` força `true` (igual ao TS).
/// Heurística de produção: o executável resolve sob `/usr/`.
pub fn is_system_install() -> bool {
    if std::env::var_os("AGENT_BAR_FORCE_COMPILED").as_deref() == Some(std::ffi::OsStr::new("1")) {
        return true;
    }
    std::env::current_exe()
        .map(|p| p.starts_with("/usr/"))
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[serial_test::serial]
    fn forced_compiled_env_is_system() {
        temp_env::with_var("AGENT_BAR_FORCE_COMPILED", Some("1"), || {
            assert!(is_system_install());
        });
    }

    #[test]
    #[serial_test::serial]
    fn unset_force_env_is_not_forced() {
        // Sem o env e fora de /usr/, deve ser false no ambiente de teste (cargo target/).
        temp_env::with_var("AGENT_BAR_FORCE_COMPILED", None::<&str>, || {
            // current_exe do test runner não está sob /usr/ → false.
            assert!(!is_system_install());
        });
    }
}
