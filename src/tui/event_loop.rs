use futures::StreamExt as _;
use ratatui::crossterm::event::{Event, EventStream};
use ratatui::DefaultTerminal;
use tokio::time::{interval, Duration};
use tui_input::backend::crossterm::EventHandler as _;

use crate::providers::extras::get_amp_extra;
use crate::providers::OwnedCtx;
use crate::settings;
use crate::setup;
use crate::usage;
use crate::waybar_integration::{self, get_default_waybar_integration_paths, ApplyOptions};

use super::action::Action;
use super::login_spawn::RealLogin;
use super::render::render;
use super::state::{AppState, Screen};
use super::update::update;

/// Dispara o parse pesado dos session logs FORA do thread do event loop
/// (`spawn_blocking`) e devolve os resultados via canal (`Action::UsageComputed` +
/// `Action::HistoryLoaded`). O parse cold pode levar ~10-20s; mantê-lo aqui — e não
/// inline no loop — deixa o `select!` livre p/ servir teclas/animação enquanto isso.
fn spawn_usage_load(
    bg_tx: &tokio::sync::mpsc::UnboundedSender<Action>,
    octx: &OwnedCtx,
    state: &AppState,
) {
    let claude_dir = octx.home.join(".claude").join("projects");
    let codex_dir = octx.home.join(".codex").join("sessions");
    let fx_rate = octx.settings.fx_rate;

    // amp_meta do ProviderView do Amp (clone OWNED p/ cruzar o spawn_blocking).
    let amp_meta: Option<std::collections::BTreeMap<String, String>> = state
        .providers
        .iter()
        .find(|pv| pv.quota.provider == "amp")
        .and_then(|pv| get_amp_extra(&pv.quota))
        .and_then(|e| e.meta.clone());

    // Cutoffs no contexto async: clock via OwnedCtx::now_ms() (a thread de fetch
    // nao tem o now_ms do Ctx original) p/ hoje; now_utc p/ a janela de 7d.
    let now_ms = OwnedCtx::now_ms();
    let today_start = time::OffsetDateTime::from_unix_timestamp_nanos((now_ms as i128) * 1_000_000)
        .map(|t| t.to_offset(octx.local_offset))
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
fn handle_save_config(state: &mut AppState, octx: &OwnedCtx) {
    let edited = match state.config_state.as_ref() {
        Some(cs) => cs.edit_settings.clone(),
        None => return,
    };

    let result: Result<(), String> = (|| {
        settings::save(&octx.paths, &edited).map_err(|e| format!("save falhou: {e}"))?;

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
/// restaura o terminal, e despacha LoginResult com o resultado. Chamado do
/// event_loop quando pending_login e consumido (nao e puro — faz IO).
fn handle_login(state: &mut AppState, provider_id: String) {
    use crate::tui::login_spawn::ProviderLogin as _;
    let login = RealLogin;
    let result = login.launch(&provider_id).map_err(|e| e.to_string());
    for a in update(state, Action::LoginResult(result)) {
        update(state, a);
    }
}

/// Despacha todas as follow-up actions retornadas por update (1 nivel de profundidade).
/// InitConfig, ReloadUsage, Refresh e LoginFinished sao interceptados aqui para IO.
fn drain(
    state: &mut AppState,
    octx: &OwnedCtx,
    bg_tx: &tokio::sync::mpsc::UnboundedSender<Action>,
    actions: Vec<Action>,
) {
    for a in actions {
        match a {
            // InitConfig com settings reais do octx (sobrescreve o placeholder do update).
            Action::InitConfig(_placeholder) => {
                for sub in update(state, Action::InitConfig(octx.settings.clone())) {
                    update(state, sub);
                }
            }
            // ReloadUsage e interceptado: redispara o parse de usage em background.
            Action::ReloadUsage => {
                spawn_usage_load(bg_tx, octx, state);
            }
            // Refresh (tecla [r]) e interceptado: dispara refetch real fora do
            // event loop. update() ja garantiu que so re-enfileira quando nao
            // ha fetch em voo (evita spawn_fetch duplicado).
            Action::Refresh => {
                super::fetch::spawn_fetch(bg_tx, octx.clone(), None);
            }
            // LoginFinished e interceptado: refetch so do provider que fez
            // login (nao passa por Refresh — o guard de fetch_pending
            // poderia engolir o refetch se uma onda cheia estiver em voo).
            Action::LoginFinished(id) => {
                super::fetch::spawn_fetch(bg_tx, octx.clone(), Some(id));
            }
            other => {
                update(state, other);
            }
        }
    }
}

/// Event loop principal. Corre ate `state.should_quit`. O fetch de quotas
/// (inicial e a cada 60s) roda numa thread propria (`tui::fetch::spawn_fetch`)
/// — o `select!` NUNCA espera rede; teclas e animacao respondem durante o fetch.
pub async fn run(octx: OwnedCtx, terminal: &mut DefaultTerminal) -> anyhow::Result<()> {
    let mut state = AppState::new();
    // Offset local real (T12 fix): sem isto, `local_offset` fica no default
    // UTC do AppState::new() e o pico do sparkline de 24h mostra hora UTC
    // rotulada como "local".
    state.local_offset = octx.local_offset;
    // Zonas clicaveis do frame atual (Task 9): populado por `render`, limpo
    // a cada `terminal.draw` (frames antigos nao devem vazar cliques).
    let mut hits = super::mouse::HitMap::default();

    // Canal p/ os resultados do fetch e do parse de usage em background. Mantém o loop livre.
    let (bg_tx, mut bg_rx) = tokio::sync::mpsc::unbounded_channel::<Action>();

    let mut events = EventStream::new();
    // interval_at: 1º tick em +60s (NÃO imediato). O fetch inicial abaixo já faz a
    // 1ª carga; o tick imediato do `interval` re-disparava fetch redundante no boot.
    let mut data_tick = tokio::time::interval_at(
        tokio::time::Instant::now() + Duration::from_secs(60),
        Duration::from_secs(60),
    );
    let mut anim_tick = interval(Duration::from_millis(30));

    // Fetch inicial: dispara em thread propria e segue — o select! serve
    // teclado/animação já, sem esperar a rede (bug que esta task mata).
    super::fetch::spawn_fetch(&bg_tx, octx.clone(), None);

    loop {
        terminal.draw(|f| {
            hits.clear();
            render(&state, f, &mut hits)
        })?;

        // IO pendente que exige frame previo: o draw acima ja pintou o
        // status ("Abrindo login para X..." / "Salvando...") antes de
        // suspender o terminal ou bloquear em IO (fix desta task).
        if let Some(id) = state.pending_login.take() {
            handle_login(&mut state, id.clone());
            let follow_ups = update(&mut state, Action::LoginFinished(id));
            drain(&mut state, &octx, &bg_tx, follow_ups);
            // login_spawn suspende/reinicializa o terminal com instância própria; clear() ressincroniza o buffer deste run()
            terminal.clear()?;
            continue;
        }
        if state.pending_save {
            state.pending_save = false;
            handle_save_config(&mut state, &octx);
            continue;
        }

        if state.should_quit {
            break;
        }

        tokio::select! {
            maybe_ev = events.next() => {
                if let Some(Ok(ev)) = maybe_ev {
                    if let Event::Key(key) = &ev {
                        // Na tela Waybar em modo edicao, passa o evento cru ao Input antes
                        // de traduzir em Action (permite edicao caracter a caracter).
                        if state.screen == Screen::Waybar {
                            if let Some(cs) = state.config_state.as_mut() {
                                if cs.editing {
                                    cs.input.handle_event(&ev);
                                    // Nao dispatch Key normal: Esc/Enter sao tratados abaixo.
                                }
                            }
                        }
                        let follow_ups = update(&mut state, Action::Key(*key));
                        drain(&mut state, &octx, &bg_tx, follow_ups);
                    } else if let Event::Mouse(m) = &ev {
                        use ratatui::crossterm::event::{MouseButton, MouseEventKind};
                        let action = match m.kind {
                            MouseEventKind::Down(MouseButton::Left) => {
                                hits.at(m.column, m.row).map(Action::Click)
                            }
                            MouseEventKind::Moved => Some(Action::Hover(hits.at(m.column, m.row))),
                            MouseEventKind::ScrollUp => Some(Action::Scroll(-1)),
                            MouseEventKind::ScrollDown => Some(Action::Scroll(1)),
                            _ => None,
                        };
                        if let Some(a) = action {
                            let follow_ups = update(&mut state, a);
                            drain(&mut state, &octx, &bg_tx, follow_ups);
                        }
                    }
                }
            }

            _ = data_tick.tick() => {
                // Guard igual ao Refresh: nao dispara uma 2a onda de fetch se
                // a anterior ainda estiver em voo (evita ondas sobrepostas
                // corromperem fetch_pending/status/last_update).
                if state.fetch_pending.is_empty() {
                    super::fetch::spawn_fetch(&bg_tx, octx.clone(), None);
                }
            }

            bg = bg_rx.recv() => {
                if let Some(action) = bg {
                    let follow_ups = update(&mut state, action);
                    drain(&mut state, &octx, &bg_tx, follow_ups);
                }
            }

            _ = anim_tick.tick() => {
                update(&mut state, Action::AnimTick);
            }
        }
    }

    Ok(())
}
