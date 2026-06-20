pub mod action;
pub mod event_loop;
pub mod state;
pub mod theme_bridge;
pub mod update;

use anyhow::Context as _;

use crate::providers::Ctx;

/// Abre o terminal alternado, inicializa o AppState, e executa o event loop.
pub async fn run_tui(ctx: &Ctx<'_>) -> anyhow::Result<()> {
    // Instala panic hook que restaura o terminal antes de imprimir o backtrace.
    let original_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let _ = ratatui::try_restore();
        original_hook(info);
    }));

    let mut terminal = ratatui::try_init().context("falha ao inicializar o terminal")?;

    let result = event_loop::run(ctx, &mut terminal).await;

    ratatui::try_restore().context("falha ao restaurar o terminal")?;

    result
}
