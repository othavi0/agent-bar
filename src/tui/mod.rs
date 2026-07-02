pub mod action;
pub mod event_loop;
pub mod fetch;
pub mod login_spawn;
pub mod login_state;
pub mod render;
pub mod state;
pub mod theme_bridge;
pub mod update;
pub mod widgets;

use anyhow::Context as _;

use crate::providers::{Ctx, OwnedCtx};

/// Abre o terminal alternado, inicializa o AppState, e executa o event loop.
pub async fn run_tui(ctx: &Ctx<'_>) -> anyhow::Result<()> {
    // Instala panic hook que restaura o terminal antes de imprimir o backtrace.
    let original_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
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

    let octx = OwnedCtx::from_ctx(ctx);
    let result = event_loop::run(octx, &mut terminal).await;

    ratatui::try_restore().context("falha ao restaurar o terminal")?;

    // Restaura o nível original (relevante se algo logar após a TUI fechar).
    log::set_max_level(prior_level);

    result
}
