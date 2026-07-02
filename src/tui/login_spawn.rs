//! Trait `ProviderLogin` e implementacao real (`RealLogin`).
//!
//! Design: o `update` e puro e apenas sinaliza `Action::LoginRequested(id)`.
//! O event_loop intercepta a action e chama `RealLogin::launch`, que:
//!   1. Restaura o terminal (`ratatui::restore`).
//!   2. Spawna o CLI de login com stdio herdado.
//!   3. Aguarda o processo terminar.
//!   4. Re-inicializa o terminal (`ratatui::try_init`) + limpa o display.
//!
//! Comandos de login (ver spec `docs/superpowers/specs/2026-07-01-tui-redesign-design.md` §4.3):
//!   - claude: `claude` (sem args; o usuario digita `/login` dentro da REPL)
//!   - codex:  `codex login`
//!   - amp:    `amp login`

use std::process::Command;

use anyhow::{bail, Context as _};

use crate::install::ensure_command;
use crate::providers::amp_cli::find_amp_bin;

/// Trait mockavel para testes unitarios do update/event_loop.
pub trait ProviderLogin {
    fn launch(&self, provider_id: &str) -> anyhow::Result<()>;
}

/// Implementacao de producao: suspende ratatui, spawna o CLI, restaura.
pub struct RealLogin;

impl ProviderLogin for RealLogin {
    fn launch(&self, provider_id: &str) -> anyhow::Result<()> {
        // Desabilita a captura de mouse (Task 9) antes de devolver o terminal
        // ao CLI externo — senao o CLI de login herdaria cliques como escape
        // sequences no seu proprio stdin.
        let _ = ratatui::crossterm::execute!(
            std::io::stdout(),
            ratatui::crossterm::event::DisableMouseCapture
        );
        // Restaura o terminal antes de entregar o controle ao CLI externo.
        ratatui::restore();

        let result = run_login_cli(provider_id);

        // Re-inicializa o terminal independentemente de sucesso ou falha.
        match ratatui::try_init() {
            Ok(mut t) => {
                let _ = t.clear();
                // Reabilita a captura de mouse ao retomar a TUI.
                let _ = ratatui::crossterm::execute!(
                    std::io::stdout(),
                    ratatui::crossterm::event::EnableMouseCapture
                );
            }
            Err(e) => {
                log::warn!("falha ao re-inicializar terminal após login: {e}");
            }
        }

        result
    }
}

/// Executa o CLI de login para o provider, com stdio herdado do processo pai.
/// Retorna erro se o comando nao for encontrado ou se o processo falhar.
fn run_login_cli(provider_id: &str) -> anyhow::Result<()> {
    match provider_id {
        "claude" => {
            let ok = ensure_command("claude", "Instale o Claude Code CLI (binário: claude).");
            if !ok {
                bail!("claude CLI não encontrado no PATH");
            }
            let status = Command::new("claude")
                .stdin(std::process::Stdio::inherit())
                .stdout(std::process::Stdio::inherit())
                .stderr(std::process::Stdio::inherit())
                .status()
                .context("falha ao executar claude")?;
            if !status.success() {
                log::warn!("claude encerrou com código {:?}", status.code());
            }
            Ok(())
        }

        "codex" => {
            let ok = ensure_command("codex", "Instale o OpenAI Codex CLI (binário: codex).");
            if !ok {
                bail!("codex CLI não encontrado no PATH");
            }
            let status = Command::new("codex")
                .arg("login")
                .stdin(std::process::Stdio::inherit())
                .stdout(std::process::Stdio::inherit())
                .stderr(std::process::Stdio::inherit())
                .status()
                .context("falha ao executar codex login")?;
            if !status.success() {
                log::warn!("codex login encerrou com código {:?}", status.code());
            }
            Ok(())
        }

        "amp" => {
            let home = std::env::var("HOME").unwrap_or_default();
            let amp_bin = find_amp_bin(&home).ok_or_else(|| {
                anyhow::anyhow!(
                    "Amp CLI não encontrado. Instale com: {}",
                    crate::app_identity::AMP_INSTALL_COMMAND
                )
            })?;
            let status = Command::new(&amp_bin)
                .arg("login")
                .stdin(std::process::Stdio::inherit())
                .stdout(std::process::Stdio::inherit())
                .stderr(std::process::Stdio::inherit())
                .status()
                .context("falha ao executar amp login")?;
            if !status.success() {
                log::warn!("amp login encerrou com código {:?}", status.code());
            }
            Ok(())
        }

        other => bail!("provider desconhecido para login: {other}"),
    }
}

/// Funcao compartilhada usada pelo `action_right::login_stub` para lancar o
/// login a partir do right-click do Waybar (contexto sem TUI ativa).
///
/// Diferente de `RealLogin::launch`, esta funcao NAO tenta restaurar ratatui
/// (o terminal ja esta em modo normal no contexto do right-click). Ela apenas
/// executa o CLI com stdio herdado e aguarda.
pub fn launch_login_no_tui(provider_id: &str) -> anyhow::Result<()> {
    run_login_cli(provider_id)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Mock de ProviderLogin para testes unitarios.
    pub struct MockLogin {
        pub calls: std::cell::RefCell<Vec<String>>,
        pub result: Result<(), String>,
    }

    impl MockLogin {
        pub fn ok() -> Self {
            Self {
                calls: std::cell::RefCell::new(vec![]),
                result: Ok(()),
            }
        }

        pub fn failing(msg: &str) -> Self {
            Self {
                calls: std::cell::RefCell::new(vec![]),
                result: Err(msg.to_string()),
            }
        }
    }

    impl ProviderLogin for MockLogin {
        fn launch(&self, provider_id: &str) -> anyhow::Result<()> {
            self.calls.borrow_mut().push(provider_id.to_string());
            match &self.result {
                Ok(()) => Ok(()),
                Err(msg) => anyhow::bail!("{}", msg),
            }
        }
    }

    #[test]
    fn mock_login_records_call() {
        let mock = MockLogin::ok();
        mock.launch("claude").unwrap();
        mock.launch("codex").unwrap();
        assert_eq!(*mock.calls.borrow(), vec!["claude", "codex"]);
    }

    #[test]
    fn mock_login_failing_returns_error() {
        let mock = MockLogin::failing("erro simulado");
        let err = mock.launch("amp").unwrap_err();
        assert!(err.to_string().contains("erro simulado"));
        assert_eq!(*mock.calls.borrow(), vec!["amp"]);
    }

    #[test]
    fn run_login_cli_unknown_provider_errors() {
        let err = run_login_cli("unknown_xyz").unwrap_err();
        assert!(err.to_string().contains("provider desconhecido"));
    }

    // NB: Nao testamos login real (nao spawna claude/codex/amp ao vivo).
    // O mock cobre o contrato do trait. Smoke/integracao e manual.
}
