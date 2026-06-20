pub mod theme_bridge;

use anyhow::Context as _;
use ratatui::crossterm::event::{self, Event, KeyCode};
use ratatui::widgets::Block;

use crate::providers::Ctx;

/// Abre o terminal alternado, desenha 1 frame com título e sai na 1ª tecla.
/// T1: esqueleto que prova abre/fecha limpo. O loop async vem na T2.
pub async fn run_tui(_ctx: &Ctx<'_>) -> anyhow::Result<()> {
    // Instala panic hook que restaura o terminal antes de imprimir o backtrace.
    let original_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        // Ignoramos o Result aqui: já estamos em panic, não há como reportar.
        let _ = ratatui::try_restore();
        original_hook(info);
    }));

    let mut terminal = ratatui::try_init().context("falha ao inicializar o terminal")?;

    loop {
        terminal
            .draw(|f| {
                f.render_widget(Block::bordered().title("agent-bar"), f.area());
            })
            .context("falha ao desenhar frame")?;

        if event::poll(std::time::Duration::from_millis(200))
            .context("falha ao fazer poll de evento")?
        {
            if let Event::Key(key) = event::read().context("falha ao ler evento")? {
                if key.code == KeyCode::Char('q') || key.code == KeyCode::Esc {
                    break;
                }
                // Qualquer outra tecla também sai (T1 = smoke, sai na 1ª tecla).
                break;
            }
        }
    }

    ratatui::try_restore().context("falha ao restaurar o terminal")?;
    Ok(())
}
