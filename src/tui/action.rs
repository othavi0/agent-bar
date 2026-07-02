use ratatui::crossterm::event::KeyEvent;

use crate::providers::types::ProviderQuota;
use crate::settings::Settings;
use crate::usage::{UsageRecord, UsageSummary};

use super::state::Tab;

#[derive(Debug)]
pub enum Action {
    Key(KeyEvent),
    Tick,
    AnimTick,
    /// Fetch iniciou para estes provider ids (spinner/progresso).
    FetchStarted(Vec<String>),
    /// Um provider terminou (merge incremental — a tela atualiza aos poucos).
    /// Boxed: `ProviderQuota` é ~500 bytes inline, muito maior que as demais
    /// variantes do enum (clippy::large_enum_variant).
    ProviderFetched(Box<ProviderQuota>),
    /// Todos terminaram. `fetched_at` ISO (mesmo formato do AllQuotas).
    FetchCompleted { fetched_at: String },
    /// Pede ao event_loop para redisparar o parse de usage (interceptada).
    ReloadUsage,
    FetchFailed(String),
    /// Engine de custo calculou UsageSummary; armazenar em AppState.usage.
    UsageComputed(UsageSummary),
    Up,
    Down,
    OpenDetail,
    Back,
    SwitchTab(Tab),
    Refresh,
    Quit,
    // --- Aba Waybar Config ---
    /// Inicializa o ConfigState com as settings atuais (lazy, ao entrar na aba).
    InitConfig(Settings),
    /// Navega entre campos da config.
    ConfigUp,
    ConfigDown,
    /// Entra em modo de edicao do campo selecionado.
    ConfigEnterEdit,
    /// Cancela a edicao sem salvar.
    ConfigCancelEdit,
    /// Confirma a edicao do campo atual (aplica o valor ao edit_settings).
    ConfigConfirmEdit,
    /// Salva as settings editadas (sinaliza para o event_loop; nao faz IO).
    SaveConfig,
    /// Feedback de resultado do save (exibido na status_msg da aba).
    ConfigSaveResult(Result<(), String>),
    // --- Aba History ---
    /// Records carregados via records_since (7d). IO acontece no event_loop.
    HistoryLoaded(Vec<UsageRecord>),
    // --- Aba Login ---
    /// Navega para cima na lista de providers da aba Login.
    LoginUp,
    /// Navega para baixo na lista de providers da aba Login.
    LoginDown,
    /// Sinaliza ao event_loop que deve lancar o login do provider indicado.
    /// O update e puro (nao spawna); o event_loop intercepta e chama RealLogin.
    LoginRequested(String),
    /// Feedback do resultado do login (exibido como status na aba).
    LoginResult(Result<(), String>),
    /// Login terminou (IO ja concluido pelo event_loop). Interceptada no
    /// drain para disparar refetch do provider, sem re-entrar no update.
    LoginFinished(String),
    /// Abre/fecha o overlay de ajuda (atalhos de teclado).
    ToggleHelp,
}
