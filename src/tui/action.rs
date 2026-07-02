use ratatui::crossterm::event::KeyEvent;

use crate::providers::types::ProviderQuota;
use crate::settings::Settings;
use crate::usage::{UsageRecord, UsageSummary};

use super::mouse::MouseTarget;
use super::state::SidebarItem;

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
    /// `silent`: onda disparada pelo `data_tick` de 60s (poll de fundo) vs
    /// onda pedida pelo usuário (load inicial, `r`/chip Refresh,
    /// LoginFinished) — `update` só empurra `FxEvent::FetchLanded` (sweep,
    /// T16) quando `!silent` (spec §8: sweep é feedback de ação do
    /// usuário, não deve repetir a cada poll silencioso).
    FetchCompleted {
        fetched_at: String,
        silent: bool,
    },
    /// Pede ao event_loop para redisparar o parse de usage (interceptada).
    ReloadUsage,
    FetchFailed(String),
    /// Engine de custo calculou UsageSummary; armazenar em AppState.usage.
    UsageComputed(UsageSummary),
    Up,
    Down,
    OpenDetail,
    Back,
    /// Move a selecao da sidebar diretamente para o indice dado.
    SelectSidebar(usize),
    /// Ativa o item selecionado da sidebar (navega para a tela correspondente).
    Activate(SidebarItem),
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
    /// Alterna o range do chart (24h/7d) — tecla `t` na tela History.
    ToggleHistoryRange,
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
    // --- Mouse (Task 9) ---
    /// Clique esquerdo num alvo do HitMap (sidebar/card/chip).
    Click(MouseTarget),
    /// Mouse moveu; alvo do HitMap sob o cursor (None se fora de qualquer zona).
    Hover(Option<MouseTarget>),
    /// Roda do mouse: delta positivo desce, negativo sobe. Satura em 0 no update.
    Scroll(i32),
}
