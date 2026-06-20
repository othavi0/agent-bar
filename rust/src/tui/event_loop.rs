use futures::StreamExt as _;
use ratatui::crossterm::event::{Event, EventStream};
use ratatui::DefaultTerminal;
use tokio::time::{interval, Duration};

use crate::providers::extras::get_amp_extra;
use crate::providers::{fetch_all, registry, Ctx};
use crate::usage::{self, AggregateOptions};

use super::action::Action;
use super::render::render;
use super::state::{AppState, FetchStatus};
use super::update::update;

/// Taxa de cambio padrao US$/BRL (configuravel via settings.fx_rate na T8).
/// TODO: settings.fx_rate na T8
const DEFAULT_FX_RATE: f64 = 5.50;

/// Calcula o UsageSummary a partir dos logs locais e despacha Action::UsageComputed.
/// Roda no mesmo arm do data_tick (sincrono, aceitavel no v1 — cache incremental
/// do engine amortiza o custo). Pode ser revisitado pra thread separada se lento.
fn compute_and_dispatch_usage(state: &mut AppState, ctx: &Ctx<'_>) {
    // Extrai amp_meta do ProviderView do Amp, se disponivel.
    let amp_meta = state
        .providers
        .iter()
        .find(|pv| pv.quota.provider == "amp")
        .and_then(|pv| get_amp_extra(&pv.quota))
        .and_then(|amp_extra| amp_extra.meta.as_ref());

    let claude_dir = ctx.home.join(".claude").join("projects");
    let codex_dir = ctx.home.join(".codex").join("sessions");

    let summary = usage::aggregate(AggregateOptions {
        claude_dir: &claude_dir,
        codex_dir: &codex_dir,
        fx_rate: DEFAULT_FX_RATE,
        amp_meta,
    });

    update(state, Action::UsageComputed(summary));
}

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
                    compute_and_dispatch_usage(&mut state, ctx);
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
                        compute_and_dispatch_usage(&mut state, ctx);
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
