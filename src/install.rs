//! Verificacao de presenca de comandos no PATH. Port de `src/install.ts`.
//!
//! `ensure_amp_cli` (guidance de instalacao do Amp, port de `src/amp-cli.ts`)
//! foi removida (v9, limpeza de legado) por nao ter caller: o locator real
//! em producao e `providers::amp_cli::find_amp_bin`.

use crate::providers::amp_cli::which_in_path;

/// Verifica se `cmd` existe no `$PATH`. Usa `which_in_path` do providers/amp_cli.rs
/// (sem duplicar logica). Retorna `true` se encontrado.
pub fn has_cmd(cmd: &str) -> bool {
    which_in_path(cmd).is_some()
}

/// Retorna `true` se `cmd` for encontrado no PATH. Se ausente, imprime um
/// aviso com `install_hint` via `log::warn` e retorna `false`.
pub fn ensure_command(cmd: &str, install_hint: &str) -> bool {
    if has_cmd(cmd) {
        return true;
    }
    log::warn!("{cmd} não encontrado. {install_hint}");
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    // has_cmd detecta comandos reais do sistema (sh sempre existe em Linux).
    #[test]
    fn has_cmd_finds_sh() {
        let _env = crate::test_support::env_guard();
        assert!(has_cmd("sh"), "sh deve existir no PATH");
    }

    #[test]
    fn has_cmd_misses_nonexistent() {
        assert!(!has_cmd("__agent_bar_nonexistent_cmd_xyz__"));
    }

    #[test]
    fn ensure_command_true_when_present() {
        let _env = crate::test_support::env_guard();
        // sh esta disponivel; nao loga warn.
        assert!(ensure_command("sh", "hint nao deve aparecer"));
    }

    #[test]
    fn ensure_command_false_when_absent() {
        let _env = crate::test_support::env_guard();
        assert!(!ensure_command(
            "__agent_bar_nonexistent_cmd_xyz__",
            "instale-o"
        ));
    }
}
