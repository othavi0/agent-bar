use futures::StreamExt as _;
use ratatui::crossterm::event::{Event, EventStream};
use ratatui::DefaultTerminal;
use tokio::time::{interval, Duration};
use tui_input::backend::crossterm::EventHandler as _;

use crate::providers::extras::get_amp_extra;
use crate::providers::{fetch_all, registry, Ctx};
use crate::settings;
use crate::setup;
use crate::usage;
use crate::waybar_integration::{self, get_default_waybar_integration_paths, ApplyOptions};

use super::action::Action;
use super::login_spawn::RealLogin;
use super::render::render;
use super::state::{AppState, FetchStatus, Tab};
use super::update::update;

/// Dispara o parse pesado dos session logs FORA do thread do event loop
/// (`spawn_blocking`) e devolve os resultados via canal (`Action::UsageComputed` +
/// `Action::HistoryLoaded`). O parse cold pode levar ~10-20s; mantê-lo aqui — e não
/// inline no loop — deixa o `select!` livre p/ servir teclas/animação enquanto isso.
fn spawn_usage_load(
    bg_tx: &tokio::sync::mpsc::UnboundedSender<Action>,
    ctx: &Ctx<'_>,
    state: &AppState,
) {
    let claude_dir = ctx.home.join(".claude").join("projects");
    let codex_dir = ctx.home.join(".codex").join("sessions");
    let fx_rate = ctx.settings.fx_rate;

    // amp_meta do ProviderView do Amp (clone OWNED p/ cruzar o spawn_blocking).
    let amp_meta: Option<std::collections::BTreeMap<String, String>> = state
        .providers
        .iter()
        .find(|pv| pv.quota.provider == "amp")
        .and_then(|pv| get_amp_extra(&pv.quota))
        .and_then(|e| e.meta.clone());

    // Cutoffs no contexto async: clock injetado (ctx) p/ hoje; now_utc p/ a janela de 7d.
    let today_start =
        time::OffsetDateTime::from_unix_timestamp_nanos((ctx.now_ms as i128) * 1_000_000)
            .map(|t| t.to_offset(ctx.local_offset))
            .map(|t| t.replace_time(time::Time::MIDNIGHT))
            .unwrap_or(time::OffsetDateTime::UNIX_EPOCH);
    let history_cutoff = time::OffsetDateTime::now_utc() - time::Duration::days(7);

    // std::thread (NÃO tokio::spawn_blocking): thread detached, não-rastreada pelo
    // runtime. Ao quitar a TUI o processo sai na hora — o runtime do tokio ESPERARIA
    // um spawn_blocking terminar (o parse cold leva ~18s → quit travava). O parse é só
    // estatística fire-and-forget; abandoná-lo no quit é correto.
    let tx = bg_tx.clone();
    std::thread::spawn(move || {
        // Custo de HOJE (escopado a meia-noite local).
        let today = usage::records_since(
            usage::AggregateOptions {
                claude_dir: &claude_dir,
                codex_dir: &codex_dir,
                fx_rate,
                amp_meta: None,
            },
            today_start,
        );
        let summary = usage::aggregate_records(today, fx_rate, amp_meta.as_ref());
        let _ = tx.send(Action::UsageComputed(summary));

        // History dos últimos 7 dias (records crus p/ a aba History).
        let records = usage::records_since(
            usage::AggregateOptions {
                claude_dir: &claude_dir,
                codex_dir: &codex_dir,
                fx_rate,
                amp_meta: None,
            },
            history_cutoff,
        );
        let _ = tx.send(Action::HistoryLoaded(records));
    });
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

    // Canal p/ os resultados do parse em background (spawn_blocking). Mantém o loop livre.
    let (bg_tx, mut bg_rx) = tokio::sync::mpsc::unbounded_channel::<Action>();

    let mut events = EventStream::new();
    // interval_at: 1º tick em +60s (NÃO imediato). O `initial_fetch` já faz a 1ª carga;
    // o tick imediato do `interval` re-disparava fetch+parse redundante logo após o boot.
    let mut data_tick = tokio::time::interval_at(
        tokio::time::Instant::now() + Duration::from_secs(60),
        Duration::from_secs(60),
    );
    let mut anim_tick = interval(Duration::from_millis(30));

    // Tick imediato: dispara o fetch inicial sem esperar 60s.
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
            // Fetch inline (async, ~sub-segundo). O parse pesado dos logs vai p/ background
            // via spawn_usage_load, entao o loop chega no select! quase imediatamente.
            match tokio::time::timeout(Duration::from_secs(30), fetch_all(&providers, ctx)).await {
                Ok(quotas) => {
                    let follow_ups = update(&mut state, Action::DataFetched(quotas));
                    drain(&mut state, ctx, follow_ups);
                    spawn_usage_load(&bg_tx, ctx, &state);
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
                        spawn_usage_load(&bg_tx, ctx, &state);
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

            bg = bg_rx.recv() => {
                if let Some(action) = bg {
                    update(&mut state, action);
                }
            }

            _ = anim_tick.tick() => {
                update(&mut state, Action::AnimTick);
            }
        }
    }

    Ok(())
}
