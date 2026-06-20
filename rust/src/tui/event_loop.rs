use futures::StreamExt as _;
use ratatui::crossterm::event::{Event, EventStream};
use ratatui::DefaultTerminal;
use tokio::time::{interval, Duration};
use tui_input::backend::crossterm::EventHandler as _;

use crate::providers::extras::get_amp_extra;
use crate::providers::{fetch_all, registry, Ctx};
use crate::settings;
use crate::setup;
use crate::usage::{self, AggregateOptions};
use crate::waybar_integration::{self, get_default_waybar_integration_paths, ApplyOptions};

use super::action::Action;
use super::login_spawn::RealLogin;
use super::render::render;
use super::state::{AppState, FetchStatus, Tab};
use super::update::update;

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
        fx_rate: ctx.settings.fx_rate,
        amp_meta,
    });

    update(state, Action::UsageComputed(summary));
}

/// Persiste as edit_settings, aplica a integracao Waybar e recarrega o Waybar.
/// Chamado do event_loop quando SaveConfig e interceptado (nao e puro — faz IO).
fn handle_save_config(state: &mut AppState, ctx: &Ctx<'_>) {
    let edited = match state.config_state.as_ref() {
        Some(cs) => cs.edit_settings.clone(),
        None => return,
    };

    let result: Result<(), String> = (|| {
        settings::save(ctx.paths, &edited).map_err(|e| format!("save falhou: {e}"))?;

        let paths = get_default_waybar_integration_paths();
        let opts = ApplyOptions {
            paths,
            icons_dir: None,
            app_bin: None,
            terminal_script: None,
        };
        waybar_integration::apply_waybar_integration(&edited, opts)
            .map_err(|e| format!("apply falhou: {e}"))?;

        setup::reload_waybar();
        Ok(())
    })();

    for a in update(state, Action::ConfigSaveResult(result)) {
        update(state, a);
    }
}

/// Executa o login de um provider (IO): suspende o terminal, spawna o CLI,
/// restaura o terminal, e despacha LoginResult com o resultado.
fn handle_login(state: &mut AppState, provider_id: String) {
    use crate::tui::login_spawn::ProviderLogin as _;
    let login = RealLogin;
    let result = login.launch(&provider_id).map_err(|e| e.to_string());
    update(state, Action::LoginResult(result));
}

/// Despacha todas as follow-up actions retornadas por update (1 nivel de profundidade).
/// SaveConfig e LoginRequested sao interceptados aqui para IO.
fn drain(state: &mut AppState, ctx: &Ctx<'_>, actions: Vec<Action>) {
    for a in actions {
        match a {
            // InitConfig com settings reais do ctx (sobrescreve o placeholder do update).
            Action::InitConfig(_placeholder) => {
                for sub in update(state, Action::InitConfig(ctx.settings.clone())) {
                    update(state, sub);
                }
            }
            // SaveConfig e interceptado: nao re-entra no update, faz IO aqui.
            Action::SaveConfig => {
                handle_save_config(state, ctx);
            }
            // LoginRequested e interceptado: nao re-entra no update, faz IO aqui.
            Action::LoginRequested(id) => {
                handle_login(state, id);
            }
            other => {
                update(state, other);
            }
        }
    }
}

/// Event loop principal. Corre ate `state.should_quit`.
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
                    let follow_ups = update(&mut state, Action::DataFetched(quotas));
                    drain(&mut state, ctx, follow_ups);
                    compute_and_dispatch_usage(&mut state, ctx);
                }
                Err(_) => {
                    let follow_ups =
                        update(&mut state, Action::FetchFailed("fetch timeout".to_string()));
                    drain(&mut state, ctx, follow_ups);
                }
            }
            continue;
        }

        tokio::select! {
            maybe_ev = events.next() => {
                if let Some(Ok(ev)) = maybe_ev {
                    if let Event::Key(key) = &ev {
                        // Na aba Waybar em modo edicao, passa o evento cru ao Input antes
                        // de traduzir em Action (permite edicao caracter a caracter).
                        if state.tab == Tab::Waybar {
                            if let Some(cs) = state.config_state.as_mut() {
                                if cs.editing {
                                    cs.input.handle_event(&ev);
                                    // Nao dispatch Key normal: Esc/Enter sao tratados abaixo.
                                }
                            }
                        }
                        let follow_ups = update(&mut state, Action::Key(*key));
                        drain(&mut state, ctx, follow_ups);
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
                        let follow_ups = update(&mut state, Action::DataFetched(quotas));
                        drain(&mut state, ctx, follow_ups);
                        compute_and_dispatch_usage(&mut state, ctx);
                    }
                    Err(_) => {
                        let follow_ups = update(
                            &mut state,
                            Action::FetchFailed("fetch timeout".to_string()),
                        );
                        drain(&mut state, ctx, follow_ups);
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
