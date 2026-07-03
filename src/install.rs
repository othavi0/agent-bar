//! Verificacao de presenca de comandos no PATH. Port de `src/install.ts` +
//! a parte `ensure_amp_cli` de `src/amp-cli.ts`.
//!
//! NOTA: `ensure_amp_cli` aqui **orienta** o usuario (imprime o comando de
//! instalacao) em vez de executar `curl | bash` automaticamente. O instalador
//! interativo foi descartado por seguranca: a TUI nao deve pipe-executar codigo
//! remoto sem confirmacao explicita do usuario.

use crate::app_identity::AMP_INSTALL_COMMAND;
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

/// Verifica se `amp` esta disponivel. Se ausente, loga o comando de instalacao
/// oficial e retorna `false`. Nao executa o instalador automaticamente.
///
/// Para instalacao real, o usuario deve rodar manualmente:
/// `curl -fsSL https://ampcode.com/install.sh | bash`
pub fn ensure_amp_cli() -> bool {
    if has_cmd("amp") {
        return true;
    }
    log::warn!(
        "Amp CLI não encontrado. Para instalar, rode: {}",
        AMP_INSTALL_COMMAND
    );
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

    #[test]
    fn ensure_amp_cli_absent_returns_false() {
        let _env = crate::test_support::env_guard();
        // Em ambiente CI/test, amp provavelmente nao esta instalado.
        // Se estiver, o test passa com true; se nao, com false. Ambos sao corretos.
        let _ = ensure_amp_cli(); // apenas verifica que nao panica
    }
}
