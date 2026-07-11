pub mod action;
pub mod effects;
pub mod event_loop;
pub mod fetch;
pub mod login_spawn;
pub mod login_state;
pub mod mouse;
pub mod render;
pub mod state;
pub mod theme_bridge;
pub mod update;
pub mod widgets;

use anyhow::Context as _;

use crate::providers::{Ctx, OwnedCtx};

/// Alvo de foco inicial da TUI (Task 11). `None` = boot default do menu
/// (foco no 1º provider habilitado nas settings, resolvido lazy no
/// `event_loop::run`). `Provider(id)` = action-right/chip de um provider
/// específico. `Login(id)` = boot direto na tela Login com aquele provider
/// pré-selecionado (ex. chip "fazer login"). Nunca índice fixo — sempre por
/// ID (T12 consome este enum ao construir o foco a partir do `action_right`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InitialFocus {
    Provider(String),
    Login(String),
}

/// Abre o terminal alternado, inicializa o AppState, e executa o event loop.
pub async fn run_tui(ctx: &Ctx<'_>, focus: Option<InitialFocus>) -> anyhow::Result<()> {
    // Instala panic hook que restaura o terminal antes de imprimir o backtrace.
    // Desabilita a captura de mouse primeiro (Task 9): ratatui::try_restore não
    // sabe sobre ela, e um panic com a captura ainda ativa deixaria o terminal
    // do usuário reportando cliques como escape sequences após o crash.
    let original_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let _ = ratatui::crossterm::execute!(
            std::io::stdout(),
            ratatui::crossterm::event::DisableMouseCapture
        );
        let _ = ratatui::try_restore();
        original_hook(info);
    }));

    // Silencia o logger enquanto a TUI ocupa o terminal (alternate screen no mesmo
    // stderr): um log::warn!/error! de provider (ex. fetch falho) escreveria escape
    // sequences por cima dos frames do ratatui e corromperia a tela. Os erros de
    // fetch já aparecem DENTRO da TUI via o estado do provider; o log é redundante.
    let prior_level = log::max_level();
    log::set_max_level(log::LevelFilter::Off);

    let mut terminal = ratatui::try_init().context("falha ao inicializar o terminal")?;
    // Captura de eventos de mouse (Task 9): entra APÓS o alternate screen —
    // ratatui::try_init já fez enable_raw_mode + EnterAlternateScreen acima.
    // Best-effort (mesmo padrão do Disable abaixo): se falhar, a TUI segue
    // funcionando só com teclado em vez de abortar com o terminal preso em
    // alternate screen/raw mode sem `try_restore` (o `?` propagaria o erro
    // ANTES do restore no fim desta função).
    if let Err(e) = ratatui::crossterm::execute!(
        std::io::stdout(),
        ratatui::crossterm::event::EnableMouseCapture
    ) {
        log::warn!("mouse capture indisponível: {e}");
    }

    let octx = OwnedCtx::from_ctx(ctx);
    let result = event_loop::run(octx, &mut terminal, focus).await;

    let _ = ratatui::crossterm::execute!(
        std::io::stdout(),
        ratatui::crossterm::event::DisableMouseCapture
    );
    ratatui::try_restore().context("falha ao restaurar o terminal")?;

    // Restaura o nível original (relevante se algo logar após a TUI fechar).
    log::set_max_level(prior_level);

    result
}
