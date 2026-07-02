//! Efeitos tachyonfx: coalesce na troca de tela, sweep no fetch.
//!
//! `enabled=false` (`settings.menu.animations`) vira tudo no-op — nem
//! `on_event` empurra pro manager, nem `process` toca o buffer. O gate vive
//! aqui (não em `AppState`) porque estes efeitos rodam DEPOIS do render, no
//! buffer já pintado (`terminal.draw` no `event_loop`) — não fazem parte do
//! contrato de `render()`, que continua puro e determinístico p/ snapshot.

use std::time::Duration;

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use tachyonfx::{fx, EffectManager, Interpolation, Motion};

use super::state::FxEvent;

/// Gerencia os efeitos tachyonfx em voo. `content_area` (parâmetro de
/// `on_event`) não é usada pelos efeitos atuais (coalesce/sweep operam
/// sobre a `area` recebida por `process`, no frame seguinte) — mantida na
/// assinatura pra efeitos futuros escopados a uma região específica.
pub struct Effects {
    manager: EffectManager<()>,
    enabled: bool,
}

impl Effects {
    pub fn new(enabled: bool) -> Self {
        Self {
            manager: EffectManager::default(),
            enabled,
        }
    }

    /// Traduz um `FxEvent` (drenado de `AppState.fx_queue` pelo event_loop)
    /// no efeito tachyonfx correspondente. No-op se `enabled=false`.
    pub fn on_event(&mut self, ev: FxEvent, _content_area: Rect) {
        if !self.enabled {
            return;
        }
        match ev {
            // Coalesce (~280ms, SineOut): a tela nova "se forma" em vez de
            // trocar seca — feedback de navegação (Geral↔Histórico etc.).
            FxEvent::ScreenChanged => self
                .manager
                .add_effect(fx::coalesce((280, Interpolation::SineOut))),
            // Sweep esquerda→direita (~900ms, QuadOut): dado novo "varre" a
            // tela quando um fetch termina.
            FxEvent::FetchLanded => self.manager.add_effect(fx::sweep_in(
                Motion::LeftToRight,
                10,
                0,
                ratatui::style::Color::Black,
                (900, Interpolation::QuadOut),
            )),
        }
    }

    /// Avança todos os efeitos em voo por `elapsed` e pinta o resultado
    /// direto no buffer já renderizado. Chamado 1x por frame, DEPOIS de
    /// `render()` (dentro do fechamento de `terminal.draw`). No-op se
    /// `enabled=false` — nunca bloqueia input (só manipula o buffer, sem IO
    /// nem espera).
    pub fn process(&mut self, elapsed: Duration, buf: &mut Buffer, area: Rect) {
        if !self.enabled {
            return;
        }
        self.manager.process_effects(elapsed.into(), buf, area);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Confirma o padrão de uso real (`event_loop::run`): o buffer é
    /// REPINTADO com o conteúdo correto a cada frame (via `render()`) ANTES
    /// de `effects.process()` rodar em cima dele — só assim `sweep_in`
    /// converge de volta pro estilo original ao fim da animação. Descoberto
    /// investigando por que um probe ingênuo (processar o MESMO buffer sem
    /// repintar) nunca convergia: o shader relê `cell.fg` a cada chamada
    /// como base do lerp, então sem repintura ele reblenda um valor já
    /// parcialmente esmaecido e fica preso num ponto fixo — nunca o bug
    /// real do event_loop (que sempre re-renderiza antes de processar).
    #[test]
    fn sweep_in_converges_when_buffer_is_repainted_every_frame() {
        use ratatui::buffer::Buffer;
        use ratatui::layout::Rect;
        use ratatui::style::{Color, Style};
        use tachyonfx::{fx, EffectManager, Interpolation, Motion};

        let area = Rect::new(0, 0, 20, 5);
        let target_style = Style::default().fg(Color::Rgb(200, 200, 200));
        let paint = |buf: &mut Buffer| {
            for y in 0..area.height {
                for x in 0..area.width {
                    buf[(x, y)].set_symbol("X").set_style(target_style);
                }
            }
        };

        let mut buf = Buffer::empty(area);
        paint(&mut buf);

        let mut manager: EffectManager<()> = EffectManager::default();
        manager.add_effect(fx::sweep_in(
            Motion::LeftToRight,
            10,
            0,
            Color::Black,
            (900, Interpolation::QuadOut),
        ));

        let mut saw_faded_cell = false;
        for _ in 0..40 {
            paint(&mut buf); // repintura fresca, como `render()` faz a cada draw
            manager.process_effects(Duration::from_millis(30).into(), &mut buf, area);
            if buf[(0, 0)].fg != Color::Rgb(200, 200, 200) {
                saw_faded_cell = true;
            }
        }
        assert!(
            saw_faded_cell,
            "sweep_in nunca esmaeceu nenhuma célula — efeito não está visível"
        );
        // Após 40 * 30ms = 1200ms (> os 900ms configurados), o efeito já
        // completou e o manager o removeu — o buffer converge de volta ao
        // estilo alvo em toda a área (nenhum resquício do faded_color).
        for y in 0..area.height {
            for x in 0..area.width {
                assert_eq!(
                    buf[(x, y)].fg,
                    Color::Rgb(200, 200, 200),
                    "célula ({x},{y}) deveria ter convergido de volta ao estilo alvo"
                );
            }
        }
    }

    #[test]
    fn disabled_on_event_does_not_panic_and_process_is_noop() {
        let mut fx = Effects::new(false);
        let area = Rect::new(0, 0, 10, 4);
        fx.on_event(FxEvent::ScreenChanged, area);
        fx.on_event(FxEvent::FetchLanded, area);

        let mut buf = Buffer::empty(area);
        let before = buf.clone();
        fx.process(Duration::from_millis(30), &mut buf, area);
        assert_eq!(buf, before, "enabled=false não deve tocar o buffer");
    }

    #[test]
    fn enabled_process_runs_without_panic() {
        let mut fx = Effects::new(true);
        let area = Rect::new(0, 0, 10, 4);
        fx.on_event(FxEvent::ScreenChanged, area);
        fx.on_event(FxEvent::FetchLanded, area);

        let mut buf = Buffer::empty(area);
        // Só garante que não panica e que os efeitos são drenados ao longo
        // do tempo (determinismo de VALOR de pixel não é o contrato aqui —
        // isso é smoke visual, não snapshot).
        for _ in 0..40 {
            fx.process(Duration::from_millis(30), &mut buf, area);
        }
    }
}
