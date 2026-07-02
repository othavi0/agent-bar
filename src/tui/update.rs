use ratatui::crossterm::event::{KeyCode, KeyEvent};

use super::action::Action;
use super::mouse::{ChipKind, MouseTarget};
use super::state::{
    sidebar_items, AppState, ConfigField, ConfigState, FetchStatus, HistoryRange, ProviderView,
    Screen, SidebarItem,
};

/// Translates a raw KeyEvent into a semantic Action, if applicable.
pub fn key_to_action(key: KeyEvent) -> Option<Action> {
    match key.code {
        KeyCode::Char('j') | KeyCode::Down => Some(Action::Down),
        KeyCode::Char('k') | KeyCode::Up => Some(Action::Up),
        KeyCode::Enter => Some(Action::OpenDetail),
        KeyCode::Esc => Some(Action::Back),
        KeyCode::Left => {
            // Will be resolved in update using current tab; return sentinel via Char('<')
            // Actually we return a SwitchTab action resolved here is not possible without state.
            // So we return a raw Left action wrapped — update will handle it.
            None // handled below
        }
        _ => None,
    }
}

/// Translates a KeyEvent into a semantic Action using current screen state:
/// up/down move the sidebar cursor, Enter/h/g/w activate a sidebar item
/// (jumping directly to Detail/History/Login/Waybar), Esc goes back to
/// Overview. Some screens (Waybar editing, Login list) intercept keys with
/// their own local semantics before falling through to this navigation.
fn key_to_action_with_state(key: KeyEvent, state: &AppState) -> Option<Action> {
    // Se o overlay de ajuda esta aberto, qualquer tecla fecha (Esc ou '?').
    if state.show_help {
        return match key.code {
            KeyCode::Esc | KeyCode::Char('?') | KeyCode::Char('q') => Some(Action::ToggleHelp),
            _ => None,
        };
    }

    // '?' global: abre o overlay de ajuda de qualquer contexto (exceto edicao).
    let in_config_edit = state.screen == Screen::Waybar
        && state
            .config_state
            .as_ref()
            .map(|cs| cs.editing)
            .unwrap_or(false);

    if key.code == KeyCode::Char('?') && !in_config_edit {
        return Some(Action::ToggleHelp);
    }

    // Na tela Waybar com campo em edicao, delega ao input buffer (so Esc/Enter escapam).
    if state.screen == Screen::Waybar {
        if let Some(cs) = &state.config_state {
            if cs.editing {
                return match key.code {
                    KeyCode::Esc => Some(Action::ConfigCancelEdit),
                    KeyCode::Enter => Some(Action::ConfigConfirmEdit),
                    _ => None, // o event_loop vai passar o evento cru ao Input
                };
            }
        }
        // Tela Waybar, fora do modo edicao. h/g/w saltam direto para outra
        // tela (substituem o antigo ←→ de troca de aba — sem conflito com
        // j/k/Enter/s/Esc/q, ja usados aqui).
        return match key.code {
            KeyCode::Char('j') | KeyCode::Down => Some(Action::ConfigDown),
            KeyCode::Char('k') | KeyCode::Up => Some(Action::ConfigUp),
            KeyCode::Enter => Some(Action::ConfigEnterEdit),
            KeyCode::Char('s') => Some(Action::SaveConfig),
            KeyCode::Esc => Some(Action::Back),
            KeyCode::Char('h') => Some(Action::Activate(SidebarItem::History)),
            KeyCode::Char('g') => Some(Action::Activate(SidebarItem::Login)),
            KeyCode::Char('w') => Some(Action::Activate(SidebarItem::Waybar)),
            KeyCode::Char('q') => Some(Action::Quit),
            _ => None,
        };
    }

    // Tela Login: navegacao e acao de login. h/w saltam direto para outra
    // tela (mesmo racional da tela Waybar acima).
    if state.screen == Screen::Login {
        return match key.code {
            KeyCode::Char('j') | KeyCode::Down => Some(Action::LoginDown),
            KeyCode::Char('k') | KeyCode::Up => Some(Action::LoginUp),
            KeyCode::Enter => {
                let id = match state.login_selected {
                    0 => "claude",
                    1 => "codex",
                    _ => "amp",
                };
                Some(Action::LoginRequested(id.to_string()))
            }
            KeyCode::Esc => Some(Action::Back),
            KeyCode::Char('h') => Some(Action::Activate(SidebarItem::History)),
            KeyCode::Char('g') => Some(Action::Activate(SidebarItem::Login)),
            KeyCode::Char('w') => Some(Action::Activate(SidebarItem::Waybar)),
            KeyCode::Char('q') => Some(Action::Quit),
            _ => None,
        };
    }

    // Tela History: 't' alterna o range do chart (24h/7d). Escopado à tela
    // (não junto do match genérico abaixo) — 't' ainda não tem significado
    // em Overview/Detail, então fica reservado em vez de virar global cedo
    // demais (mesmo racional do `in_config_edit` acima).
    if state.screen == Screen::History && key.code == KeyCode::Char('t') {
        return Some(Action::ToggleHistoryRange);
    }

    match key.code {
        KeyCode::Char('j') | KeyCode::Down => Some(Action::Down),
        KeyCode::Char('k') | KeyCode::Up => Some(Action::Up),
        KeyCode::Enter => Some(Action::OpenDetail),
        KeyCode::Esc => Some(Action::Back),
        KeyCode::Char('h') => Some(Action::Activate(SidebarItem::History)),
        KeyCode::Char('g') => Some(Action::Activate(SidebarItem::Login)),
        KeyCode::Char('w') => Some(Action::Activate(SidebarItem::Waybar)),
        KeyCode::Char('r') => Some(Action::Refresh),
        KeyCode::Char('q') => Some(Action::Quit),
        _ => None,
    }
}

/// Retorna a representacao em string de um campo das edit_settings.
fn field_value_string(field: ConfigField, cs: &ConfigState) -> String {
    let s = &cs.edit_settings;
    match field {
        ConfigField::Providers => s.waybar.providers.join(", "),
        ConfigField::ProviderOrder => s.waybar.provider_order.join(", "),
        ConfigField::Separators => format!("{:?}", s.waybar.separators).to_lowercase(),
        ConfigField::DisplayMode => format!("{:?}", s.waybar.display_mode).to_lowercase(),
        ConfigField::FxRate => format!("{:.2}", s.fx_rate),
        ConfigField::Signal => s
            .waybar
            .signal
            .map(|n| n.to_string())
            .unwrap_or_else(|| "none".to_string()),
        ConfigField::Interval => s.waybar.interval.to_string(),
    }
}

/// Aplica o valor textual do buffer de edicao ao campo das edit_settings.
/// Retorna Err com mensagem descritiva se o valor e invalido.
fn apply_field_edit(field: ConfigField, value: &str, cs: &mut ConfigState) -> Result<(), String> {
    let s = &mut cs.edit_settings;
    match field {
        ConfigField::Providers => {
            let providers: Vec<String> = value
                .split(',')
                .map(|p| p.trim().to_string())
                .filter(|p| !p.is_empty())
                .collect();
            let (normalized, order) =
                crate::settings::normalize_provider_selection(&providers, &s.waybar.provider_order);
            s.waybar.providers = normalized;
            s.waybar.provider_order = order;
            Ok(())
        }
        ConfigField::ProviderOrder => {
            let order: Vec<String> = value
                .split(',')
                .map(|p| p.trim().to_string())
                .filter(|p| !p.is_empty())
                .collect();
            let (normalized, new_order) =
                crate::settings::normalize_provider_selection(&s.waybar.providers, &order);
            s.waybar.providers = normalized;
            s.waybar.provider_order = new_order;
            Ok(())
        }
        ConfigField::Separators => {
            use crate::settings::SeparatorStyle;
            let sep = match value.trim() {
                "pill" => SeparatorStyle::Pill,
                "gap" => SeparatorStyle::Gap,
                "bare" => SeparatorStyle::Bare,
                "glass" => SeparatorStyle::Glass,
                "shadow" => SeparatorStyle::Shadow,
                "none" => SeparatorStyle::None,
                other => {
                    return Err(format!(
                        "separador invalido: '{other}' (pill/gap/bare/glass/shadow/none)"
                    ))
                }
            };
            s.waybar.separators = sep;
            Ok(())
        }
        ConfigField::DisplayMode => {
            use crate::settings::DisplayMode;
            let mode = match value.trim() {
                "remaining" => DisplayMode::Remaining,
                "used" => DisplayMode::Used,
                other => return Err(format!("modo invalido: '{other}' (remaining/used)")),
            };
            s.waybar.display_mode = mode;
            Ok(())
        }
        ConfigField::FxRate => {
            let rate: f64 = value
                .trim()
                .parse()
                .map_err(|_| format!("fxRate invalido: '{value}' (numero esperado)"))?;
            if !rate.is_finite() || rate <= 0.0 {
                return Err(format!("fxRate deve ser positivo: '{value}'"));
            }
            s.fx_rate = rate;
            Ok(())
        }
        ConfigField::Signal => {
            let trimmed = value.trim();
            if trimmed == "none" || trimmed.is_empty() {
                s.waybar.signal = None;
                return Ok(());
            }
            let n: i64 = trimmed
                .parse()
                .map_err(|_| format!("signal invalido: '{value}' (1-30 ou none)"))?;
            if !(1..=30).contains(&n) {
                return Err(format!("signal deve ser 1-30 ou none: '{value}'"));
            }
            s.waybar.signal = Some(n as u8);
            Ok(())
        }
        ConfigField::Interval => {
            let n: u32 = value
                .trim()
                .parse()
                .map_err(|_| format!("interval invalido: '{value}' (inteiro positivo)"))?;
            if n == 0 {
                return Err(format!("interval deve ser > 0: '{value}'"));
            }
            s.waybar.interval = n;
            Ok(())
        }
    }
}

/// Pure update function: mutates `state` based on `action`, returns follow-up actions.
/// No IO, no spawning, no clocks — fully testable.
pub fn update(state: &mut AppState, action: Action) -> Vec<Action> {
    match action {
        Action::Key(key) => {
            if let Some(semantic) = key_to_action_with_state(key, state) {
                return update(state, semantic);
            }
            vec![]
        }

        Action::Up => {
            state.sidebar_selected = state.sidebar_selected.saturating_sub(1);
            vec![]
        }

        Action::Down => {
            let max = sidebar_items(state.providers.len()).len() - 1;
            state.sidebar_selected = (state.sidebar_selected + 1).min(max);
            vec![]
        }

        Action::OpenDetail => {
            // Recursa (mesmo padrao do braço Action::Key): aplica a ativacao
            // na hora e propaga os follow-ups dela (ex. InitConfig ao ativar
            // Waybar), em vez de so devolver Activate sem aplicar.
            let items = sidebar_items(state.providers.len());
            match items.get(state.sidebar_selected).copied() {
                Some(item) => update(state, Action::Activate(item)),
                None => vec![],
            }
        }

        Action::Activate(item) => {
            let follow_ups = match item {
                SidebarItem::Overview => {
                    state.screen = Screen::Overview;
                    vec![]
                }
                SidebarItem::Provider(i) => {
                    state.selected = i;
                    state.screen = Screen::Detail;
                    vec![]
                }
                SidebarItem::History => {
                    state.screen = Screen::History;
                    vec![]
                }
                SidebarItem::Login => {
                    state.screen = Screen::Login;
                    vec![]
                }
                SidebarItem::Waybar => {
                    state.screen = Screen::Waybar;
                    // Inicializa config_state ao entrar na tela Waybar pela 1a vez.
                    // Nao podemos acessar settings aqui (update e puro), entao sinalizamos
                    // com um placeholder — o event_loop (drain) sobrescreve com as
                    // settings reais ao interceptar InitConfig (mesmo mecanismo do
                    // antigo SwitchTab para a aba Waybar).
                    if state.config_state.is_none() {
                        vec![Action::InitConfig(crate::settings::Settings {
                            version: 0, // sentinela; event_loop sobrescreve com real
                            waybar: crate::settings::Waybar {
                                providers: vec![],
                                show_percentage: true,
                                separators: crate::settings::SeparatorStyle::Gap,
                                provider_order: vec![],
                                display_mode: crate::settings::DisplayMode::Remaining,
                                signal: None,
                                interval: 60,
                            },
                            tooltip: crate::settings::Tooltip {},
                            models: Default::default(),
                            window_policy: Default::default(),
                            notify: crate::settings::Notify { enabled: true },
                            cache: crate::settings::CacheSettings {
                                ttl: Default::default(),
                            },
                            glyph_mode: crate::settings::GlyphMode::Box,
                            fx_rate: 5.50,
                        })]
                    } else {
                        vec![]
                    }
                }
            };
            // Sincroniza o cursor da sidebar com o item ativado — cobre
            // h/g/w, Enter (via OpenDetail) e cliques futuros da Task 9.
            // Sem isso, ativar por atalho deixa sidebar_selected apontando
            // pra outro item quando a Task 10 desenhar o highlight.
            if let Some(idx) = sidebar_items(state.providers.len())
                .iter()
                .position(|i| *i == item)
            {
                state.sidebar_selected = idx;
            }
            follow_ups
        }

        Action::SelectSidebar(i) => {
            state.sidebar_selected = i;
            vec![]
        }

        Action::Back => {
            state.screen = Screen::Overview;
            vec![]
        }

        Action::Refresh => {
            // Evita fetch duplicado se ja tem um em voo; senao, re-enfileira
            // Refresh UMA vez para o event_loop interceptar (mesmo padrao de
            // ReloadUsage/SaveConfig — o drain NAO re-entra no update com ele).
            if state.fetch_pending.is_empty() {
                vec![Action::Refresh]
            } else {
                vec![]
            }
        }

        Action::FetchStarted(ids) => {
            state.status = FetchStatus::Loading;
            // Uniao sem duplicatas (nao overwrite): ondas de fetch podem se
            // sobrepor (Refresh/tick de 60s disparados enquanto uma onda
            // anterior ainda resolve). Se um id ja esta pendente, a propria
            // onda em voo vai resolve-lo de novo — nao precisa re-adicionar.
            for id in ids {
                if !state.fetch_pending.contains(&id) {
                    state.fetch_pending.push(id);
                }
            }
            vec![]
        }

        Action::ProviderFetched(q) => {
            state.fetch_pending.retain(|id| id != &q.provider);
            let old_len = state.providers.len();
            match state
                .providers
                .iter_mut()
                .find(|pv| pv.quota.provider == q.provider)
            {
                Some(pv) => pv.quota = *q,
                None => {
                    state.providers.push(ProviderView::new(*q));
                    // Provider novo insere Provider(old_len) na posicao
                    // 1+old_len da sidebar (Overview + old_len providers
                    // antigos vem antes). Um cursor que ja apontava pra
                    // History/Login/Waybar (indices >= 1+old_len) passaria a
                    // apontar pro item seguinte sem isso — desloca 1 posicao
                    // pra manter o cursor no MESMO item logico.
                    if state.sidebar_selected > old_len {
                        state.sidebar_selected += 1;
                    }
                }
            }
            vec![]
        }

        Action::FetchCompleted { fetched_at } => {
            // NAO limpa fetch_pending incondicionalmente: cada ProviderFetched
            // ja remove o proprio id. Se sobrar algo aqui, e porque outra onda
            // (Refresh/tick de 60s) ainda esta em voo — mantem Loading em vez
            // de regredir pra Loaded (a onda que sobrar completa depois).
            //
            // Mesmo parse de timestamp usado pelo antigo DataFetched, mas
            // nunca regride: fica o mais recente entre o atual e o parseado
            // (ondas sobrepostas podem terminar fora de ordem).
            let parsed = time::OffsetDateTime::parse(
                &fetched_at,
                &time::format_description::well_known::Rfc3339,
            )
            .ok();
            if let Some(new) = parsed {
                state.last_update = Some(state.last_update.map_or(new, |cur| cur.max(new)));
            }
            if state.fetch_pending.is_empty() {
                state.status = FetchStatus::Loaded;
            }
            // Clamp selection if providers list shrank.
            if !state.providers.is_empty() && state.selected >= state.providers.len() {
                state.selected = state.providers.len() - 1;
            }
            vec![Action::ReloadUsage]
        }

        Action::ReloadUsage => vec![], // interceptada no event_loop; no update e no-op

        Action::FetchFailed(msg) => {
            // Estreitado pela Task 5: so cobre erro de RUNTIME da thread de
            // `spawn_fetch` (ex. falha ao construir o tokio Builder) — erros
            // de provider (rede/parse/auth) viajam embutidos no
            // ProviderQuota.error e chegam via ProviderFetched, nunca aqui.
            // Limpa fetch_pending: senao a thread morta deixa o spinner
            // girando pra sempre (nenhum ProviderFetched vai chegar).
            state.fetch_pending.clear();
            state.status = FetchStatus::Failed(msg);
            vec![]
        }

        Action::Quit => {
            state.should_quit = true;
            vec![]
        }

        Action::UsageComputed(summary) => {
            state.usage = Some(summary);
            vec![]
        }

        // --- Config tab actions ---
        Action::InitConfig(settings) => {
            // So inicializa se ainda nao existe (preserva edicoes em andamento).
            if state.config_state.is_none() {
                state.config_state = Some(ConfigState::new(&settings));
            } else if let Some(cs) = state.config_state.as_mut() {
                // Atualiza com as settings reais se ainda era o placeholder (version=0).
                if cs.edit_settings.version == 0 {
                    *cs = ConfigState::new(&settings);
                }
            }
            vec![]
        }

        Action::ConfigUp => {
            if let Some(cs) = state.config_state.as_mut() {
                if !cs.editing && cs.selected_field > 0 {
                    cs.selected_field -= 1;
                }
            }
            vec![]
        }

        Action::ConfigDown => {
            if let Some(cs) = state.config_state.as_mut() {
                let max = ConfigField::ALL.len().saturating_sub(1);
                if !cs.editing && cs.selected_field < max {
                    cs.selected_field += 1;
                }
            }
            vec![]
        }

        Action::ConfigEnterEdit => {
            if let Some(cs) = state.config_state.as_mut() {
                if !cs.editing {
                    let field = ConfigField::ALL[cs.selected_field];
                    let current = field_value_string(field, cs);
                    cs.input = tui_input::Input::new(current);
                    cs.editing = true;
                    cs.status_msg = None;
                }
            }
            vec![]
        }

        Action::ConfigCancelEdit => {
            if let Some(cs) = state.config_state.as_mut() {
                cs.editing = false;
                cs.input = tui_input::Input::default();
            }
            vec![]
        }

        Action::ConfigConfirmEdit => {
            if let Some(cs) = state.config_state.as_mut() {
                if cs.editing {
                    let field = ConfigField::ALL[cs.selected_field];
                    let value = cs.input.value().to_string();
                    match apply_field_edit(field, &value, cs) {
                        Ok(()) => {
                            cs.editing = false;
                            cs.input = tui_input::Input::default();
                            cs.status_msg =
                                Some("Campo atualizado. Pressione [s] para salvar.".to_string());
                        }
                        Err(e) => {
                            cs.status_msg = Some(format!("Erro: {e}"));
                            // Mantem edicao aberta para correcao.
                        }
                    }
                }
            }
            vec![]
        }

        Action::SaveConfig => {
            // Sinaliza pending_save: o event_loop pinta "Salvando..." no
            // frame atual e SO ENTAO faz o IO (persist + reload Waybar) no
            // topo do proximo loop. O update e puro, nao faz IO.
            if state.config_state.is_some() {
                if let Some(cs) = state.config_state.as_mut() {
                    cs.status_msg = Some("Salvando...".to_string());
                }
                state.pending_save = true;
            }
            vec![]
        }

        Action::ConfigSaveResult(result) => {
            if let Some(cs) = state.config_state.as_mut() {
                cs.status_msg = Some(match result {
                    Ok(()) => "Configuracao salva e Waybar recarregado.".to_string(),
                    Err(e) => format!("Erro ao salvar: {e}"),
                });
            }
            vec![]
        }

        // --- Aba Login ---
        Action::LoginUp => {
            if state.login_selected > 0 {
                state.login_selected -= 1;
            }
            state.login_status = None;
            vec![]
        }

        Action::LoginDown => {
            // 3 providers: indices 0, 1, 2.
            if state.login_selected < 2 {
                state.login_selected += 1;
            }
            state.login_status = None;
            vec![]
        }

        Action::LoginRequested(id) => {
            // Puro: sinaliza pending_login. O event_loop pinta o status no
            // frame atual e SO ENTAO suspende o terminal para o CLI de login
            // (fix: "Abrindo login..." nunca era pintado antes desta task).
            state.login_status = Some(format!("Abrindo login para {}...", id));
            state.pending_login = Some(id);
            vec![]
        }

        Action::LoginResult(result) => {
            state.login_status = Some(match result {
                // Refetch agora e automatico (LoginFinished), sem precisar de [r].
                Ok(()) => "Login concluido. Atualizando quota...".to_string(),
                Err(e) => format!("Erro no login: {e}"),
            });
            vec![]
        }

        Action::LoginFinished(id) => {
            // Re-enfileira UMA vez para o event_loop interceptar (mesmo
            // padrao de Refresh/ReloadUsage): o drain chama spawn_fetch(only)
            // direto, sem re-entrar no update com esta action.
            vec![Action::LoginFinished(id)]
        }

        Action::HistoryLoaded(records) => {
            state.history = Some(records);
            vec![]
        }

        Action::ToggleHistoryRange => {
            state.history_range = match state.history_range {
                HistoryRange::Day => HistoryRange::Week,
                HistoryRange::Week => HistoryRange::Day,
            };
            vec![]
        }

        Action::ToggleHelp => {
            state.show_help = !state.show_help;
            vec![]
        }

        Action::Tick => vec![],

        Action::AnimTick => {
            // Animação A (gauge lerp): cada provider avança display_ratio → target.
            for pv in &mut state.providers {
                let target = pv.target_ratio();
                pv.display_ratio += (target - pv.display_ratio) * 0.20;
            }
            // Animação C (throbber): avança o frame do spinner braille.
            state.throbber.advance();
            // Animação D (pulse): contador de frames para blink do ● crítico.
            state.anim_frame = state.anim_frame.wrapping_add(1);
            vec![]
        }

        // --- Mouse (Task 9) ---
        // Recursa via update() em vez de so devolver a action mapeada como
        // follow-up (mesmo padrao de Action::Key/Action::OpenDetail acima):
        // aplica o efeito NA HORA. Alem de manter clique e teclado simetricos
        // (ex. o Refresh clicado passa pelo MESMO guard anti-fetch-duplicado
        // que o Refresh da tecla [r] — devolver a action crua ignoraria o
        // guard, ja que drain() intercepta Action::Refresh direto sem
        // reentrar no update), a sincronicidade e o que os testes observam.
        Action::Click(target) => match target {
            MouseTarget::Sidebar(i) => {
                // sidebar_items() e a unica fonte de verdade dos indices validos
                // (SelectSidebar nao tem bounds-check); ignora i fora de faixa em
                // vez de deixar sidebar_selected apontando pra um item inexistente.
                let items = sidebar_items(state.providers.len());
                match items.get(i).copied() {
                    Some(item) => {
                        state.sidebar_selected = i;
                        update(state, Action::Activate(item))
                    }
                    None => vec![],
                }
            }
            MouseTarget::Card(i) => update(state, Action::Activate(SidebarItem::Provider(i))),
            MouseTarget::Chip(ChipKind::Open) => update(state, Action::OpenDetail),
            MouseTarget::Chip(ChipKind::Refresh) => update(state, Action::Refresh),
            MouseTarget::Chip(ChipKind::Help) => update(state, Action::ToggleHelp),
            MouseTarget::Chip(ChipKind::Quit) => update(state, Action::Quit),
            MouseTarget::Chip(ChipKind::Back) => update(state, Action::Back),
            MouseTarget::Chip(ChipKind::Login) => {
                update(state, Action::Activate(SidebarItem::Login))
            }
            MouseTarget::Chip(ChipKind::History) => {
                update(state, Action::Activate(SidebarItem::History))
            }
            MouseTarget::Chip(ChipKind::ToggleRange) => {
                update(state, Action::ToggleHistoryRange)
            }
        },

        Action::Hover(t) => {
            state.hover = t;
            vec![]
        }

        Action::Scroll(delta) => {
            // saturating_add_signed ja satura em 0 (limite inferior de u16) —
            // .max(0) e redundante (clippy::unnecessary_min_or_max).
            state.scroll = state.scroll.saturating_add_signed(delta as i16);
            vec![]
        }
    }
}

#[cfg(test)]
mod tests {
    use ratatui::crossterm::event::KeyModifiers;

    use super::*;
    use crate::providers::types::ProviderQuota;

    fn fake_quota(id: &str) -> ProviderQuota {
        ProviderQuota {
            provider: id.to_string(),
            display_name: id.to_string(),
            available: true,
            account: None,
            plan: None,
            plan_type: None,
            primary: None,
            secondary: None,
            models: None,
            extra: None,
            error: None,
        }
    }

    /// Quota com `primary.remaining` preenchido — usado pelos testes do fluxo
    /// de fetch assincrono (Task 5) que validam ProviderView/target_ratio.
    fn test_quota(id: &str, remaining: f64) -> ProviderQuota {
        use crate::providers::types::QuotaWindow;
        let mut q = fake_quota(id);
        q.primary = Some(QuotaWindow {
            remaining,
            resets_at: None,
            window_minutes: None,
            used: Some(100.0 - remaining),
            severity: None,
        });
        q
    }

    fn state_with_providers(n: usize) -> AppState {
        let mut s = AppState::new();
        s.providers = (0..n)
            .map(|i| ProviderView::new(fake_quota(&format!("p{i}"))))
            .collect();
        s
    }

    fn key_event(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::empty())
    }

    #[test]
    fn down_moves_sidebar_selected_and_clamps() {
        let mut state = state_with_providers(3);
        assert_eq!(state.sidebar_selected, 0);

        update(&mut state, Action::Down);
        assert_eq!(state.sidebar_selected, 1);

        update(&mut state, Action::Down);
        assert_eq!(state.sidebar_selected, 2);

        // sidebar_items(3) = [Overview, Provider(0..3), History, Login, Waybar] = 7 itens (indices 0..=6).
        for _ in 0..10 {
            update(&mut state, Action::Down);
        }
        assert_eq!(
            state.sidebar_selected, 6,
            "should clamp at sidebar_items.len()-1"
        );
    }

    #[test]
    fn sidebar_items_order() {
        let items = sidebar_items(2);
        assert_eq!(
            items,
            vec![
                SidebarItem::Overview,
                SidebarItem::Provider(0),
                SidebarItem::Provider(1),
                SidebarItem::History,
                SidebarItem::Login,
                SidebarItem::Waybar,
            ]
        );
    }

    #[test]
    fn up_down_move_sidebar_and_enter_activates() {
        let mut state = AppState::new();
        state.providers = vec![ProviderView::new(test_quota("claude", 80.0))];
        update(&mut state, Action::Down); // Overview → Provider(0)
        assert_eq!(state.sidebar_selected, 1);
        update(&mut state, Action::OpenDetail); // Enter
        assert_eq!(state.screen, Screen::Detail);
        assert_eq!(state.selected, 0);
        update(&mut state, Action::Back); // Esc
        assert_eq!(state.screen, Screen::Overview);
    }

    #[test]
    fn activate_history_login_waybar() {
        let mut state = AppState::new();
        update(&mut state, Action::Activate(SidebarItem::History));
        assert_eq!(state.screen, Screen::History);
        update(&mut state, Action::Activate(SidebarItem::Login));
        assert_eq!(state.screen, Screen::Login);
        let fu = update(&mut state, Action::Activate(SidebarItem::Waybar));
        assert_eq!(state.screen, Screen::Waybar);
        // Entrar na Waybar inicializa o config (comportamento atual do SwitchTab):
        assert!(matches!(fu.as_slice(), [Action::InitConfig(_)]));
    }

    #[test]
    fn provider_fetched_growth_shifts_sidebar_cursor_to_keep_same_item() {
        // Cursor comeca em History (indice 1 com 0 providers: [Overview,
        // History, Login, Waybar]). Um provider novo chega via
        // ProviderFetched (nao havia nenhum antes) — a sidebar cresce pra
        // [Overview, Provider(0), History, Login, Waybar] e History passa
        // pro indice 2. O cursor deve seguir o item logico (History), nao
        // ficar parado num indice que agora aponta pra outra coisa.
        let mut state = AppState::new();
        update(&mut state, Action::Activate(SidebarItem::History));
        assert_eq!(state.screen, Screen::History);
        assert_eq!(state.sidebar_selected, 1);

        update(
            &mut state,
            Action::ProviderFetched(Box::new(test_quota("claude", 80.0))),
        );

        assert_eq!(
            state.sidebar_selected, 2,
            "cursor deve seguir History (indice deslocado pelo provider novo)"
        );
        assert_eq!(
            sidebar_items(state.providers.len())[state.sidebar_selected],
            SidebarItem::History,
            "indice apos o shift deve continuar apontando pra History"
        );
    }

    #[test]
    fn activate_via_shortcut_syncs_sidebar_selected() {
        // Simula h/g/w: Activate chamado diretamente (nao via Down repetido)
        // deve sincronizar sidebar_selected pro indice do item ativado.
        let mut state = state_with_providers(2);
        assert_eq!(state.sidebar_selected, 0);

        update(&mut state, Action::Activate(SidebarItem::History));

        let expected = sidebar_items(2)
            .iter()
            .position(|i| *i == SidebarItem::History)
            .unwrap();
        assert_eq!(state.sidebar_selected, expected);
    }

    #[test]
    fn activate_provider_syncs_sidebar_selected() {
        let mut state = state_with_providers(2);
        // Cursor comeca em outro item (History) para provar que Activate
        // move o cursor, nao so o `selected` do Detail.
        update(&mut state, Action::Activate(SidebarItem::History));
        assert_ne!(state.sidebar_selected, 1);

        update(&mut state, Action::Activate(SidebarItem::Provider(0)));

        assert_eq!(state.screen, Screen::Detail);
        assert_eq!(state.selected, 0);
        assert_eq!(
            state.sidebar_selected, 1,
            "cursor deve sincronizar pro indice de Provider(0) na sidebar"
        );
    }

    #[test]
    fn fetch_started_sets_loading_and_pending() {
        let mut state = AppState::new();
        let fu = update(
            &mut state,
            Action::FetchStarted(vec!["claude".into(), "amp".into()]),
        );
        assert!(fu.is_empty());
        assert_eq!(state.status, FetchStatus::Loading);
        assert_eq!(
            state.fetch_pending,
            vec!["claude".to_string(), "amp".to_string()]
        );
    }

    #[test]
    fn fetch_started_unions_without_duplicating_across_overlapping_waves() {
        // Onda 1 (tick de 60s) resolve so "claude"; antes dela terminar, uma
        // onda 2 (Refresh) comeca com "claude" (de novo) + "codex". O
        // fetch_pending resultante deve ter cada id 1x, nao duplicado.
        let mut state = AppState::new();
        update(
            &mut state,
            Action::FetchStarted(vec!["claude".into(), "amp".into()]),
        );
        update(&mut state, Action::ProviderFetched(Box::new(test_quota("claude", 50.0))));
        // "claude" ja saiu do pending; "amp" ainda esta em voo quando a onda 2 comeca.
        assert_eq!(state.fetch_pending, vec!["amp".to_string()]);

        update(
            &mut state,
            Action::FetchStarted(vec!["claude".into(), "codex".into()]),
        );

        // "amp" (da onda 1) + "claude"/"codex" (da onda 2), sem duplicar "amp".
        let mut pending = state.fetch_pending.clone();
        pending.sort();
        assert_eq!(
            pending,
            vec!["amp".to_string(), "claude".to_string(), "codex".to_string()]
        );
        assert_eq!(state.status, FetchStatus::Loading);
    }

    #[test]
    fn provider_fetched_merges_by_id_and_clears_pending() {
        let mut state = AppState::new();
        update(&mut state, Action::FetchStarted(vec!["claude".into()]));
        let q = test_quota("claude", 80.0);
        update(&mut state, Action::ProviderFetched(Box::new(q.clone())));
        assert!(state.fetch_pending.is_empty());
        assert_eq!(state.providers.len(), 1);
        assert_eq!(state.providers[0].quota.provider, "claude");
        // Segundo fetch do mesmo provider substitui (nao duplica):
        update(&mut state, Action::ProviderFetched(Box::new(q)));
        assert_eq!(state.providers.len(), 1);
    }

    #[test]
    fn fetch_completed_sets_loaded_and_requests_usage_reload() {
        let mut state = AppState::new();
        update(&mut state, Action::FetchStarted(vec!["claude".into()]));
        update(
            &mut state,
            Action::ProviderFetched(Box::new(test_quota("claude", 80.0))),
        );
        let fu = update(
            &mut state,
            Action::FetchCompleted {
                fetched_at: "2026-07-01T18:00:00.000Z".into(),
            },
        );
        assert_eq!(state.status, FetchStatus::Loaded);
        assert!(state.last_update.is_some());
        assert!(matches!(fu.as_slice(), [Action::ReloadUsage]));
    }

    #[test]
    fn fetch_completed_with_pending_from_another_wave_stays_loading() {
        // Onda 1 ("claude"+"amp") e onda 2 ("codex") se sobrepoem. A onda 1
        // termina primeiro (FetchCompleted) mas "codex" (da onda 2) ainda
        // esta em voo: status deve permanecer Loading, e o pending restante
        // NAO pode ser apagado (senao a onda 2 nunca fecha o loop).
        let mut state = AppState::new();
        update(
            &mut state,
            Action::FetchStarted(vec!["claude".into(), "amp".into()]),
        );
        update(&mut state, Action::ProviderFetched(Box::new(test_quota("claude", 80.0))));
        update(&mut state, Action::ProviderFetched(Box::new(test_quota("amp", 60.0))));
        // Onda 2 comeca antes da onda 1 completar.
        update(&mut state, Action::FetchStarted(vec!["codex".into()]));

        let fu = update(
            &mut state,
            Action::FetchCompleted {
                fetched_at: "2026-07-01T18:00:00.000Z".into(),
            },
        );

        assert_eq!(
            state.status,
            FetchStatus::Loading,
            "status deve permanecer Loading — a onda 2 (codex) ainda esta em voo"
        );
        assert_eq!(
            state.fetch_pending,
            vec!["codex".to_string()],
            "pending da onda 2 nao pode ser apagado pelo FetchCompleted da onda 1"
        );
        assert!(matches!(fu.as_slice(), [Action::ReloadUsage]));
    }

    #[test]
    fn fetch_completed_never_regresses_last_update() {
        let mut state = AppState::new();
        update(&mut state, Action::FetchStarted(vec!["claude".into()]));
        update(
            &mut state,
            Action::ProviderFetched(Box::new(test_quota("claude", 80.0))),
        );
        update(
            &mut state,
            Action::FetchCompleted {
                fetched_at: "2026-07-01T18:00:00.000Z".into(),
            },
        );
        let after_first = state.last_update;
        assert!(after_first.is_some());

        // Uma 2a onda (mais lenta) termina com um fetched_at MAIS ANTIGO
        // (ex.: comecou antes, mas so completou depois) — last_update nao
        // pode regredir.
        update(&mut state, Action::FetchStarted(vec!["amp".into()]));
        update(
            &mut state,
            Action::ProviderFetched(Box::new(test_quota("amp", 60.0))),
        );
        update(
            &mut state,
            Action::FetchCompleted {
                fetched_at: "2026-07-01T17:00:00.000Z".into(), // 1h antes
            },
        );

        assert_eq!(
            state.last_update, after_first,
            "last_update nao deve regredir para um fetched_at mais antigo"
        );
    }

    #[test]
    fn fetch_failed_clears_pending() {
        let mut state = AppState::new();
        update(
            &mut state,
            Action::FetchStarted(vec!["claude".into(), "amp".into()]),
        );
        assert!(!state.fetch_pending.is_empty());

        update(&mut state, Action::FetchFailed("fetch runtime: boom".into()));

        assert!(
            state.fetch_pending.is_empty(),
            "FetchFailed deve limpar fetch_pending (senao o spinner gira pra sempre)"
        );
        assert_eq!(
            state.status,
            FetchStatus::Failed("fetch runtime: boom".to_string())
        );
    }

    #[test]
    fn fetch_flow_populates_providers_and_status() {
        // Migrado de `data_fetched_populates_providers_and_status` (Task 5):
        // o fluxo FetchStarted->ProviderFetched->FetchCompleted substitui o
        // antigo Action::DataFetched(AllQuotas), preservando as mesmas
        // validacoes (providers populados, status Loaded, last_update parseado).
        let mut state = AppState::new();
        assert_eq!(state.status, FetchStatus::Idle);
        assert!(state.providers.is_empty());
        assert!(state.last_update.is_none());

        update(
            &mut state,
            Action::FetchStarted(vec!["claude".into(), "codex".into()]),
        );
        update(
            &mut state,
            Action::ProviderFetched(Box::new(fake_quota("claude"))),
        );
        update(
            &mut state,
            Action::ProviderFetched(Box::new(fake_quota("codex"))),
        );
        update(
            &mut state,
            Action::FetchCompleted {
                fetched_at: "2026-06-19T12:00:00.000Z".to_string(),
            },
        );

        assert_eq!(state.status, FetchStatus::Loaded);
        assert_eq!(state.providers.len(), 2);
        assert_eq!(state.providers[0].quota.provider, "claude");
        assert_eq!(state.providers[1].quota.provider, "codex");
        assert!(
            state.last_update.is_some(),
            "last_update should be Some after FetchCompleted"
        );
    }

    #[test]
    fn key_q_sets_should_quit() {
        let mut state = AppState::new();
        assert!(!state.should_quit);

        // Key('q') → translated to Quit → should_quit = true
        update(&mut state, Action::Key(key_event(KeyCode::Char('q'))));

        assert!(
            state.should_quit,
            "should_quit should be true after Key('q')"
        );
    }

    #[test]
    fn anim_tick_lerps_display_ratio_toward_target() {
        use crate::providers::types::QuotaWindow;

        // Cria provider com remaining=80% → target_ratio=0.80
        let mut q = fake_quota("claude");
        q.primary = Some(QuotaWindow {
            remaining: 80.0,
            resets_at: None,
            window_minutes: None,
            used: Some(20.0),
            severity: None,
        });
        let mut state = AppState::new();
        // Inicializa com 0 (forçamos display_ratio inicial diferente do target)
        let mut pv = crate::tui::state::ProviderView::new(q);
        pv.display_ratio = 0.0; // ponto de partida artificial para testar a convergência
        state.providers = vec![pv];

        // Após 20 AnimTicks, display_ratio deve convergir próximo a 0.80
        for _ in 0..20 {
            update(&mut state, Action::AnimTick);
        }

        let display = state.providers[0].display_ratio;
        let target = 0.80_f64;
        let diff = (display - target).abs();
        assert!(
            diff < 0.01,
            "display_ratio {display:.4} deve estar próximo de {target:.2} após 20 ticks (diff={diff:.4})"
        );
    }

    #[test]
    fn anim_tick_increments_anim_frame_and_throbber() {
        let mut state = AppState::new();
        assert_eq!(state.anim_frame, 0);
        assert_eq!(state.throbber.index, 0);

        update(&mut state, Action::AnimTick);
        assert_eq!(state.anim_frame, 1);
        assert_eq!(state.throbber.index, 1);

        for _ in 0..5 {
            update(&mut state, Action::AnimTick);
        }
        // throbber wraps at 6: 1+5 = 6 → 6 % 6 = 0
        assert_eq!(
            state.throbber.index, 0,
            "throbber deve voltar a 0 após 6 ticks"
        );
        assert_eq!(state.anim_frame, 6);
    }

    #[test]
    fn display_ratio_initializes_to_target() {
        use crate::providers::types::QuotaWindow;
        let mut q = fake_quota("codex");
        q.primary = Some(QuotaWindow {
            remaining: 42.0,
            resets_at: None,
            window_minutes: None,
            used: Some(58.0),
            severity: None,
        });
        let pv = crate::tui::state::ProviderView::new(q);
        // Na inicialização, display_ratio deve ser igual ao target (sem animação no 1º frame).
        let expected = 42.0 / 100.0;
        let diff = (pv.display_ratio - expected).abs();
        assert!(
            diff < 1e-10,
            "display_ratio={} mas esperado={expected}",
            pv.display_ratio
        );
    }

    // ---- Config tab tests ----

    fn fake_settings() -> crate::settings::Settings {
        use crate::settings::*;
        use std::collections::BTreeMap;
        Settings {
            version: 2,
            waybar: Waybar {
                providers: vec!["claude".to_string(), "codex".to_string()],
                show_percentage: true,
                separators: SeparatorStyle::Gap,
                provider_order: vec!["claude".to_string(), "codex".to_string()],
                display_mode: DisplayMode::Remaining,
                signal: Some(8),
                interval: 60,
            },
            tooltip: Tooltip {},
            models: BTreeMap::new(),
            window_policy: BTreeMap::new(),
            notify: Notify { enabled: true },
            cache: CacheSettings {
                ttl: BTreeMap::new(),
            },
            glyph_mode: GlyphMode::Box,
            fx_rate: 5.50,
        }
    }

    #[test]
    fn init_config_creates_config_state() {
        let mut state = AppState::new();
        assert!(state.config_state.is_none());

        update(&mut state, Action::InitConfig(fake_settings()));

        assert!(state.config_state.is_some());
        let cs = state.config_state.as_ref().unwrap();
        let diff = (cs.edit_settings.fx_rate - 5.50_f64).abs();
        assert!(
            diff < 1e-10,
            "fx_rate esperado 5.50, obtido {}",
            cs.edit_settings.fx_rate
        );
    }

    #[test]
    fn config_navigate_down_and_up() {
        let mut state = AppState::new();
        update(&mut state, Action::InitConfig(fake_settings()));

        update(&mut state, Action::ConfigDown);
        assert_eq!(state.config_state.as_ref().unwrap().selected_field, 1);

        update(&mut state, Action::ConfigDown);
        assert_eq!(state.config_state.as_ref().unwrap().selected_field, 2);

        update(&mut state, Action::ConfigUp);
        assert_eq!(state.config_state.as_ref().unwrap().selected_field, 1);
    }

    #[test]
    fn config_navigate_clamps_at_bounds() {
        let mut state = AppState::new();
        update(&mut state, Action::InitConfig(fake_settings()));

        // Ja em 0, Up nao deve subtrair
        update(&mut state, Action::ConfigUp);
        assert_eq!(state.config_state.as_ref().unwrap().selected_field, 0);

        // Vai ate o ultimo campo
        let max = crate::tui::state::ConfigField::ALL.len() - 1;
        for _ in 0..max + 5 {
            update(&mut state, Action::ConfigDown);
        }
        assert_eq!(state.config_state.as_ref().unwrap().selected_field, max);
    }

    #[test]
    fn config_enter_edit_sets_input_to_current_value() {
        let mut state = AppState::new();
        update(&mut state, Action::InitConfig(fake_settings()));

        // Seleciona o campo FxRate (index 4)
        state.config_state.as_mut().unwrap().selected_field = 4;
        update(&mut state, Action::ConfigEnterEdit);

        let cs = state.config_state.as_ref().unwrap();
        assert!(cs.editing);
        assert_eq!(cs.input.value(), "5.50");
    }

    #[test]
    fn config_confirm_edit_updates_fx_rate() {
        let mut state = AppState::new();
        update(&mut state, Action::InitConfig(fake_settings()));
        state.config_state.as_mut().unwrap().selected_field = 4; // FxRate
        update(&mut state, Action::ConfigEnterEdit);

        // Simula o usuario digitando "6.25" no buffer
        state.config_state.as_mut().unwrap().input = tui_input::Input::new("6.25".to_string());
        update(&mut state, Action::ConfigConfirmEdit);

        let cs = state.config_state.as_ref().unwrap();
        assert!(!cs.editing, "edicao deve fechar apos confirmacao valida");
        let diff = (cs.edit_settings.fx_rate - 6.25_f64).abs();
        assert!(
            diff < 1e-10,
            "fx_rate deveria ser 6.25, obtido {}",
            cs.edit_settings.fx_rate
        );
    }

    #[test]
    fn config_confirm_edit_invalid_fx_rate_keeps_editing() {
        let mut state = AppState::new();
        update(&mut state, Action::InitConfig(fake_settings()));
        state.config_state.as_mut().unwrap().selected_field = 4; // FxRate
        update(&mut state, Action::ConfigEnterEdit);
        state.config_state.as_mut().unwrap().input = tui_input::Input::new("negativo".to_string());
        update(&mut state, Action::ConfigConfirmEdit);

        let cs = state.config_state.as_ref().unwrap();
        assert!(
            cs.editing,
            "edicao deve permanecer aberta apos valor invalido"
        );
        assert!(
            cs.status_msg
                .as_ref()
                .map(|m| m.starts_with("Erro"))
                .unwrap_or(false),
            "status_msg deve conter 'Erro'"
        );
    }

    #[test]
    fn config_cancel_edit_clears_editing() {
        let mut state = AppState::new();
        update(&mut state, Action::InitConfig(fake_settings()));
        state.config_state.as_mut().unwrap().selected_field = 4; // FxRate
        update(&mut state, Action::ConfigEnterEdit);
        assert!(state.config_state.as_ref().unwrap().editing);

        update(&mut state, Action::ConfigCancelEdit);
        assert!(!state.config_state.as_ref().unwrap().editing);
    }

    #[test]
    fn save_config_sets_pending_save_and_status() {
        let mut state = AppState::new();
        update(&mut state, Action::InitConfig(fake_settings()));

        let follow_ups = update(&mut state, Action::SaveConfig);
        // Nao re-enfileira mais: o event_loop le pending_save no topo do loop.
        assert!(follow_ups.is_empty());
        assert!(state.pending_save, "pending_save deve ser sinalizado");
        // Status msg deve ser "Salvando..."
        let msg = state.config_state.as_ref().unwrap().status_msg.as_deref();
        assert_eq!(msg, Some("Salvando..."));
    }

    // ---- Aba Login ----

    #[test]
    fn login_requested_sets_pending_and_status() {
        let mut state = AppState::new();
        let fu = update(&mut state, Action::LoginRequested("codex".into()));
        assert!(fu.is_empty()); // nao re-enfileira mais: o event_loop le pending_login
        assert_eq!(state.pending_login.as_deref(), Some("codex"));
        assert!(state.login_status.as_deref().unwrap_or("").contains("codex"));
    }

    #[test]
    fn login_finished_success_requests_single_refetch() {
        let mut state = AppState::new();
        let fu = update(&mut state, Action::LoginFinished("codex".into()));
        // O drain intercepta esta action diretamente (spawn_fetch(only=Some(id)))
        // sem re-entrar no update — nao ha guard anti-loop porque o update
        // nunca ve esta action de volta.
        assert!(matches!(fu.as_slice(), [Action::LoginFinished(id)] if id == "codex"));
    }

    // ---- Refresh (tecla [r]) ----

    #[test]
    fn refresh_with_no_pending_fetch_reenqueues_once() {
        let mut state = AppState::new();
        assert!(state.fetch_pending.is_empty());

        let fu = update(&mut state, Action::Refresh);

        assert!(matches!(fu.as_slice(), [Action::Refresh]));
    }

    #[test]
    fn refresh_with_pending_fetch_is_noop() {
        let mut state = AppState::new();
        update(&mut state, Action::FetchStarted(vec!["claude".into()]));
        assert!(!state.fetch_pending.is_empty());

        let fu = update(&mut state, Action::Refresh);

        assert!(
            fu.is_empty(),
            "Refresh deve ser no-op quando ja ha fetch em voo (evita duplicar spawn_fetch)"
        );
    }

    // ---- Mouse (Task 9) ----

    #[test]
    fn click_sidebar_selects_and_activates() {
        let mut state = AppState::new();
        state.providers = vec![ProviderView::new(test_quota("claude", 80.0))];
        update(&mut state, Action::Click(MouseTarget::Sidebar(1)));
        assert_eq!(state.sidebar_selected, 1);
        assert_eq!(state.screen, Screen::Detail); // Provider(0) ativado
    }

    #[test]
    fn click_sidebar_out_of_range_is_noop() {
        // sidebar_items(0) = [Overview, History, Login, Waybar] (4 itens, indices 0..=3).
        let mut state = AppState::new();
        let fu = update(&mut state, Action::Click(MouseTarget::Sidebar(99)));
        assert_eq!(
            state.sidebar_selected, 0,
            "indice fora de faixa e ignorado — cursor nao se move"
        );
        assert!(
            fu.is_empty(),
            "indice fora de sidebar_items() nao deve gerar Activate"
        );
        assert_eq!(state.screen, Screen::Overview, "tela nao muda sem Activate");
    }

    // ---- Aba History (Task 13) ----

    #[test]
    fn toggle_history_range_flips() {
        let mut state = AppState::new();
        assert_eq!(state.history_range, HistoryRange::Week);
        update(&mut state, Action::ToggleHistoryRange);
        assert_eq!(state.history_range, HistoryRange::Day);
        update(&mut state, Action::ToggleHistoryRange);
        assert_eq!(state.history_range, HistoryRange::Week);
    }

    #[test]
    fn click_toggle_range_chip_flips_range() {
        let mut state = AppState::new();
        assert_eq!(state.history_range, HistoryRange::Week);
        update(
            &mut state,
            Action::Click(MouseTarget::Chip(ChipKind::ToggleRange)),
        );
        assert_eq!(state.history_range, HistoryRange::Day);
    }

    #[test]
    fn key_t_on_history_screen_toggles_range() {
        let mut state = AppState::new();
        state.screen = Screen::History;
        update(&mut state, Action::Key(key_event(KeyCode::Char('t'))));
        assert_eq!(state.history_range, HistoryRange::Day);
    }

    #[test]
    fn key_t_outside_history_screen_is_noop() {
        let mut state = AppState::new();
        assert_eq!(state.screen, Screen::Overview);
        let fu = update(&mut state, Action::Key(key_event(KeyCode::Char('t'))));
        assert!(fu.is_empty());
        assert_eq!(
            state.history_range,
            HistoryRange::Week,
            "'t' fora da tela History nao deve alternar o range"
        );
    }

    #[test]
    fn hover_and_scroll_update_state() {
        let mut state = AppState::new();
        update(&mut state, Action::Hover(Some(MouseTarget::Card(0))));
        assert_eq!(state.hover, Some(MouseTarget::Card(0)));
        state.scroll = 2;
        update(&mut state, Action::Scroll(-1));
        assert_eq!(state.scroll, 1);
        update(&mut state, Action::Scroll(-5));
        assert_eq!(state.scroll, 0); // saturating
    }
}
