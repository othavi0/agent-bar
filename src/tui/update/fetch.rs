//! Handlers de fetch de providers, refresh e pending_focus.

use crate::providers::types::ProviderQuota;
use crate::tui::action::Action;
use crate::tui::state::{AppState, FetchStatus, FxEvent, ProviderView, Screen};

pub(super) fn refresh(state: &mut AppState) -> Vec<Action> {
    // Evita fetch duplicado se ja tem um em voo; senao, re-enfileira
    // Refresh UMA vez para o event_loop interceptar (mesmo padrao de
    // ReloadUsage/SaveConfig — o drain NAO re-entra no update com ele).
    if state.fetch_pending.is_empty() {
        vec![Action::Refresh]
    } else {
        vec![]
    }
}

pub(super) fn fetch_started(state: &mut AppState, ids: Vec<String>) -> Vec<Action> {
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

pub(super) fn provider_fetched(state: &mut AppState, q: Box<ProviderQuota>) -> Vec<Action> {
    state.fetch_pending.retain(|id| id != &q.provider);
    let old_len = state.providers.len();
    let provider_id = q.provider.clone();
    match state
        .providers
        .iter_mut()
        .find(|pv| pv.quota.provider == q.provider)
    {
        Some(pv) => pv.quota = *q,
        None => {
            state.providers.push(ProviderView::new(*q));
            // Provider novo insere Provider(old_len) na posicao
            // old_len da sidebar (Task 11: sem Overview, os
            // providers começam direto no índice 0 — sem o
            // deslocamento de +1 que a Overview antes exigia). Um
            // cursor que já apontava pra History/Login/Waybar
            // (índices >= old_len) passaria a apontar pro item
            // anterior sem isso — desloca 1 posição pra manter o
            // cursor no MESMO item lógico.
            if state.sidebar_selected >= old_len {
                state.sidebar_selected += 1;
            }
        }
    }
    // Boot/foco pendente (Task 11): resolve por ID, lazy, nunca por
    // índice fixo — o fetch de OUTRO provider não pode roubar o
    // foco (só entra aqui quando o provider ALVO chega). Provider(i)
    // é o i-ésimo item da sidebar (sem offset de Overview).
    if state.pending_focus.as_deref() == Some(provider_id.as_str()) {
        if let Some(idx) = state
            .providers
            .iter()
            .position(|p| p.quota.provider == provider_id)
        {
            state.selected = idx;
            state.screen = Screen::Detail;
            state.sidebar_selected = idx;
            state.pending_focus = None;
        }
    }
    vec![]
}

pub(super) fn fetch_completed(
    state: &mut AppState,
    fetched_at: String,
    silent: bool,
) -> Vec<Action> {
    // Efeito sweep (T16, fix pós-review): dispara quando uma onda
    // de fetch termina, mesmo se outra onda sobreposta ainda
    // estiver em voo (gatilho é a action chegando, não o status
    // final agregado) — MAS só para ondas pedidas pelo usuário
    // (load inicial, `r`/chip Refresh, LoginFinished). O poll
    // silencioso de 60s (`silent=true`) NUNCA dispara o sweep
    // (spec §8: efeito é feedback de ação, não deve repetir a cada
    // minuto sem o usuário fazer nada). O resto do bookkeeping
    // abaixo (last_update monotônico, status, ReloadUsage) é
    // agnóstico à origem da onda — count-up do header pode
    // continuar re-lerpando num poll silencioso, é sutil.
    if !silent {
        state.fx_queue.push(FxEvent::FetchLanded);
    }
    // NAO limpa fetch_pending incondicionalmente: cada ProviderFetched
    // ja remove o proprio id. Se sobrar algo aqui, e porque outra onda
    // (Refresh/tick de 60s) ainda esta em voo — mantem Loading em vez
    // de regredir pra Loaded (a onda que sobrar completa depois).
    //
    // Mesmo parse de timestamp usado pelo antigo DataFetched, mas
    // nunca regride: fica o mais recente entre o atual e o parseado
    // (ondas sobrepostas podem terminar fora de ordem).
    let parsed =
        time::OffsetDateTime::parse(&fetched_at, &time::format_description::well_known::Rfc3339)
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

pub(super) fn reload_usage() -> Vec<Action> {
    // interceptada no event_loop; no update e no-op
    vec![]
}

pub(super) fn fetch_failed(state: &mut AppState, msg: String) -> Vec<Action> {
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

pub(super) fn usage_computed(
    state: &mut AppState,
    summary: crate::usage::UsageSummary,
) -> Vec<Action> {
    // 1º load (usage ainda None): pinta display_cost = alvo direto,
    // sem animar a partir de zero. Loads seguintes deixam o
    // AnimTick fazer o count-up até o novo alvo.
    if state.usage.is_none() {
        state.display_cost = summary.total_cost.usd;
    }
    state.usage = Some(summary);
    vec![]
}
