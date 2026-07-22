//! Handlers da tela Waybar/config e helpers de edição de campos.

use crate::tui::action::Action;
use crate::tui::state::{AppState, ConfigField, ConfigState};

/// Retorna a representacao em string de um campo das edit_settings.
pub(super) fn field_value_string(field: ConfigField, cs: &ConfigState) -> String {
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
pub(super) fn apply_field_edit(
    field: ConfigField,
    value: &str,
    cs: &mut ConfigState,
) -> Result<(), String> {
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
                        "separador inválido: '{other}' (pill/gap/bare/glass/shadow/none)"
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
                other => return Err(format!("modo inválido: '{other}' (remaining/used)")),
            };
            s.waybar.display_mode = mode;
            Ok(())
        }
        ConfigField::FxRate => {
            let rate: f64 = value
                .trim()
                .parse()
                .map_err(|_| format!("fxRate inválido: '{value}' (número esperado)"))?;
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
                .map_err(|_| format!("signal inválido: '{value}' (1-30 ou none)"))?;
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
                .map_err(|_| format!("interval inválido: '{value}' (inteiro positivo)"))?;
            if n == 0 {
                return Err(format!("interval deve ser > 0: '{value}'"));
            }
            s.waybar.interval = n;
            Ok(())
        }
    }
}

pub(super) fn init_config(
    state: &mut AppState,
    settings: crate::settings::Settings,
) -> Vec<Action> {
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

pub(super) fn config_up(state: &mut AppState) -> Vec<Action> {
    if let Some(cs) = state.config_state.as_mut() {
        if !cs.editing && cs.selected_field > 0 {
            cs.selected_field -= 1;
        }
    }
    vec![]
}

pub(super) fn config_down(state: &mut AppState) -> Vec<Action> {
    let max = ConfigField::visible(state.platform).len().saturating_sub(1);
    if let Some(cs) = state.config_state.as_mut() {
        if !cs.editing && cs.selected_field < max {
            cs.selected_field += 1;
        }
    }
    vec![]
}

pub(super) fn config_enter_edit(state: &mut AppState) -> Vec<Action> {
    let visible = ConfigField::visible(state.platform);
    if let Some(cs) = state.config_state.as_mut() {
        if !cs.editing {
            let field = visible[cs.selected_field];
            let current = field_value_string(field, cs);
            cs.input = tui_input::Input::new(current);
            cs.editing = true;
            cs.status_msg = None;
        }
    }
    vec![]
}

pub(super) fn config_cancel_edit(state: &mut AppState) -> Vec<Action> {
    if let Some(cs) = state.config_state.as_mut() {
        cs.editing = false;
        cs.input = tui_input::Input::default();
    }
    vec![]
}

pub(super) fn config_confirm_edit(state: &mut AppState) -> Vec<Action> {
    let visible = ConfigField::visible(state.platform);
    if let Some(cs) = state.config_state.as_mut() {
        if cs.editing {
            let field = visible[cs.selected_field];
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

pub(super) fn save_config(state: &mut AppState) -> Vec<Action> {
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

pub(super) fn config_save_result(state: &mut AppState, result: Result<(), String>) -> Vec<Action> {
    if let Some(cs) = state.config_state.as_mut() {
        cs.status_msg = Some(match result {
            Ok(()) => "Configuracao salva e Waybar recarregado.".to_string(),
            Err(e) => format!("Erro ao salvar: {e}"),
        });
    }
    vec![]
}
