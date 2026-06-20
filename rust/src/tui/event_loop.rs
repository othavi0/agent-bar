use futures::StreamExt as _;
use ratatui::crossterm::event::{Event, EventStream};
use ratatui::DefaultTerminal;
use tokio::time::{interval, Duration};

use crate::providers::{fetch_all, registry, Ctx};

use super::action::Action;
use super::render::render;
use super::state::{AppState, FetchStatus};
use super::update::update;

/// Event loop principal. Corre até `state.should_quit`.
pub async fn run(ctx: &Ctx<'_>, terminal: &mut DefaultTerminal) -> anyhow::Result<()> {
    let mut state = AppState::new();

    let mut events = EventStream::new();
    let mut data_tick = interval(Duration::from_secs(60));
    let mut anim_tick = interval(Duration::from_millis(30));

    // Tick imediato: dispara o fetch inicial sem esperar 60s.
    // Marcamos como Loading e fazemos o fetch logo na 1a iteracao.
    let mut initial_fetch = true;

    loop {
        terminal.draw(|f| render(&state, f))?;

        if state.should_quit {
            break;
        }

        if initial_fetch {
            initial_fetch = false;
            state.status = FetchStatus::Loading;
            let providers = registry();
            // Fetch inline: bloqueia o loop mas e aceitavel no v1 (timeout 10s por provider).
            match tokio::time::timeout(Duration::from_secs(30), fetch_all(&providers, ctx)).await {
                Ok(quotas) => {
                    for a in update(&mut state, Action::DataFetched(quotas)) {
                        update(&mut state, a);
                    }
                }
                Err(_) => {
                    for a in update(&mut state, Action::FetchFailed("fetch timeout".to_string())) {
                        update(&mut state, a);
                    }
                }
            }
            continue;
        }

        tokio::select! {
            maybe_ev = events.next() => {
                if let Some(Ok(Event::Key(key))) = maybe_ev {
                    let follow_ups = update(&mut state, Action::Key(key));
                    for a in follow_ups {
                        update(&mut state, a);
                    }
                }
            }

            _ = data_tick.tick() => {
                state.status = FetchStatus::Loading;
                let providers = registry();
                match tokio::time::timeout(
                    Duration::from_secs(30),
                    fetch_all(&providers, ctx),
                )
                .await
                {
                    Ok(quotas) => {
                        for a in update(&mut state, Action::DataFetched(quotas)) {
                            update(&mut state, a);
                        }
                    }
                    Err(_) => {
                        for a in update(
                            &mut state,
                            Action::FetchFailed("fetch timeout".to_string()),
                        ) {
                            update(&mut state, a);
                        }
                    }
                }
            }

            _ = anim_tick.tick() => {
                update(&mut state, Action::AnimTick);
            }
        }
    }

    Ok(())
}
