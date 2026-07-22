use std::collections::BTreeSet;

use crate::providers::types::ProviderQuota;
use crate::settings::{GlyphMode, Settings};
use crate::usage::{UsageRecord, UsageSummary};

use super::mouse::MouseTarget;

/// Animação C: estado do throbber braille (índice do frame).
/// Avança via `AnimTick` no `update`.
#[derive(Debug, Clone, Default)]
pub struct ThrobberAnim {
    pub index: i8,
}

impl ThrobberAnim {
    pub fn advance(&mut self) {
        // BRAILLE_SIX tem 6 símbolos (índice 0..5); wraps modulo 6.
        const LEN: i8 = 6;
        self.index = self.index.checked_add(1).unwrap_or(0) % LEN;
    }
}

/// Janela temporal do chart da aba History (T13). `Day` = últimas 24h (24
/// buckets horários); `Week` = últimos 7 dias (24*7=168 buckets horários).
/// Alterna via tecla `t` (`Action::ToggleHistoryRange`) — SÓ o chart
/// respeita este campo; a tabela e o rodapé "Total 7d" sempre cobrem os
/// 7 dias inteiros de `state.history` (a fonte já é records_since(7d)).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HistoryRange {
    Day,
    Week,
}

/// Tela atual da TUI. Substitui `Tab` + `Mode`: cada tela e um estado
/// distinto navegado via sidebar (sem abas). `Overview` morreu na Task 11 —
/// `Detail` agora é a tela default do boot (foco resolvido por ID, ver
/// `AppState::pending_focus` e `update.rs::Action::ProviderFetched`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Screen {
    Detail,
    History,
    Login,
    Waybar,
}

/// Evento de efeito visual (T16): `update` empurra puro (`fx_queue`); o
/// event_loop drena a fila a cada frame e traduz em efeitos tachyonfx
/// (`crate::tui::effects::Effects::on_event`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FxEvent {
    /// `FetchCompleted` chegou — dispara sweep (T16).
    FetchLanded,
}

/// Item da sidebar unica. `Provider(i)` indexa `AppState.providers`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SidebarItem {
    Provider(usize),
    History,
    Login,
    Waybar,
}

/// Constroi a lista de itens da sidebar na ordem de exibicao: 1 entrada por
/// provider, History, Login, Waybar. `Overview` morreu na Task 11 — não há
/// mais item de lista/resumo antes dos providers.
pub fn sidebar_items(n_providers: usize) -> Vec<SidebarItem> {
    let mut v: Vec<SidebarItem> = (0..n_providers).map(SidebarItem::Provider).collect();
    v.extend([
        SidebarItem::History,
        SidebarItem::Login,
        SidebarItem::Waybar,
    ]);
    v
}

/// Campo da aba Waybar config (ordem de exibicao = ordem dos enum variants).
/// FxRate fica por ultimo (Task 14): as secoes WAYBAR (Providers..Interval)
/// e TUI (FxRate — afeta so este menu, nao o Waybar) ficam contiguas no
/// render. So ordem de display/navegacao (j/k); nenhuma persistencia
/// depende dela.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConfigField {
    Providers,
    ProviderOrder,
    Separators,
    DisplayMode,
    Signal,
    Interval,
    FxRate,
}

impl ConfigField {
    pub const ALL: [ConfigField; 7] = [
        ConfigField::Providers,
        ConfigField::ProviderOrder,
        ConfigField::Separators,
        ConfigField::DisplayMode,
        ConfigField::Signal,
        ConfigField::Interval,
        ConfigField::FxRate,
    ];

    /// Rótulo humano na lista/título do painel Config (trilha C).
    /// Não é a chave do settings.json — só UI.
    pub fn label(self) -> &'static str {
        match self {
            ConfigField::Providers => "Provedores",
            ConfigField::ProviderOrder => "Ordem",
            ConfigField::Separators => "Separadores",
            ConfigField::DisplayMode => "Exibição",
            ConfigField::Signal => "Sinal",
            ConfigField::Interval => "Intervalo",
            ConfigField::FxRate => "Câmbio R$",
        }
    }
}

/// Estado da aba Waybar config.
#[derive(Debug, Clone)]
pub struct ConfigState {
    /// Campo selecionado atualmente.
    pub selected_field: usize,
    /// Indica se o campo selecionado esta em modo de edicao.
    pub editing: bool,
    /// Buffer de edicao do campo atual (tui-input).
    pub input: tui_input::Input,
    /// Settings editadas (clone do original; salvas por SaveConfig).
    pub edit_settings: Settings,
    /// Mensagem de status (feedback de save).
    pub status_msg: Option<String>,
}

impl ConfigState {
    pub fn new(settings: &Settings) -> Self {
        Self {
            selected_field: 0,
            editing: false,
            input: tui_input::Input::default(),
            edit_settings: settings.clone(),
            status_msg: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FetchStatus {
    Idle,
    Loading,
    Loaded,
    Failed(String),
}

// A "Animação A" (gauge lerp via `display_ratio`/`target_ratio`) foi
// removida na varredura de legado 7.1.x: nenhum render lia o valor — os
// gauges renderizam `w.remaining` direto; só os testes exercitavam o lerp.
#[derive(Debug, Clone)]
pub struct ProviderView {
    pub quota: ProviderQuota,
}

impl ProviderView {
    pub fn new(quota: ProviderQuota) -> Self {
        Self { quota }
    }
}

#[derive(Debug)]
pub struct AppState {
    /// Tela atual (navegacao via sidebar, sem abas).
    pub screen: Screen,
    pub providers: Vec<ProviderView>,
    /// Indice do provider em foco na tela Detail.
    pub selected: usize,
    /// Indice selecionado na sidebar (indexa `sidebar_items(providers.len())`).
    pub sidebar_selected: usize,
    /// Posicao de scroll do painel de conteudo (usado por telas com overflow).
    pub scroll: u16,
    pub status: FetchStatus,
    pub last_update: Option<time::OffsetDateTime>,
    pub should_quit: bool,
    /// Resultado do engine de usage/custo (T5). None enquanto nao calculado.
    pub usage: Option<UsageSummary>,
    /// Animação C (throbber) e D (pulse): contador de frames de animação.
    /// Incrementado a cada AnimTick (~30ms). Usado para blink e throbber.
    pub anim_frame: u64,
    /// Animação C: estado do throbber braille.
    pub throbber: ThrobberAnim,
    /// Estado da aba Waybar config. None ate a aba ser aberta pela 1a vez.
    pub config_state: Option<ConfigState>,
    /// Indice selecionado na aba Login (0=claude, 1=codex, 2=amp, 3=grok).
    pub login_selected: usize,
    /// Mensagem de status da aba Login (feedback de erro ou instrucao).
    pub login_status: Option<String>,
    /// Records da aba History (ultimos 7 dias). Carregado via HistoryLoaded.
    pub history: Option<Vec<UsageRecord>>,
    /// Janela temporal exibida no chart da aba History (T13). Default Week;
    /// alterna com Day via tecla `t`.
    pub history_range: HistoryRange,
    /// Índice do dia selecionado na lista de dias expansível da aba History
    /// (T20) — indexa `sessions_by_day(state.history, state.local_offset)`,
    /// desc (0 = dia mais recente). Navegado via `HistoryUp`/`HistoryDown`
    /// (j/k/setas, escopado à tela History).
    pub history_selected: usize,
    /// Dias expandidos (mostrando as sessões individuais) na lista de dias
    /// da aba History (T20). Chave = `DaySessions.date` (data LOCAL, mesmo
    /// fuso de `sessions_by_day`). Toggle via `HistoryToggleDay` (Enter).
    pub history_expanded: BTreeSet<time::Date>,
    /// Overlay de ajuda visivel (toggle via `?`, fecha com Esc ou `?`).
    pub show_help: bool,
    /// Ids de provider com fetch em voo (Task 5). Populado por `FetchStarted`,
    /// esvaziado incrementalmente por `ProviderFetched`/`FetchCompleted`.
    pub fetch_pending: Vec<String>,
    /// Provider id aguardando foco (Task 11): setado no boot (menu default =
    /// 1º provider habilitado nas settings; action-right = provider
    /// específico via `InitialFocus::Provider`) e resolvido LAZY em
    /// `Action::ProviderFetched` — por ID, nunca por índice fixo (o fetch de
    /// OUTRO provider não pode roubar o foco). `None` quando não há foco
    /// pendente (já resolvido, ou boot direto numa tela sem provider, ex.
    /// `InitialFocus::Login`).
    pub pending_focus: Option<String>,
    /// Login pendente: o event_loop desenha 1 frame com o status e entao
    /// suspende o terminal para o CLI de login.
    pub pending_login: Option<String>,
    /// Save pendente: mesmo padrao (frame "Salvando..." antes do IO).
    pub pending_save: bool,
    /// Alvo do HitMap sob o cursor do mouse (Task 9). None fora de qualquer zona.
    pub hover: Option<MouseTarget>,
    /// Offset local do relógio (T12 fix): usado pra converter timestamps
    /// (ex. pico do sparkline de 24h) antes de extrair a hora exibida —
    /// NUNCA assuma que um `OffsetDateTime` já carrega o offset certo.
    /// Default `UtcOffset::UTC` (mantém testes/snapshots determinísticos);
    /// `event_loop::run` sobrescreve com `octx.local_offset` no boot real.
    pub local_offset: time::UtcOffset,
    /// Modo de glyph dos ícones semânticos da TUI (`tui::widgets::icons`).
    /// Default `GlyphMode::Box` (mantém testes/snapshots determinísticos
    /// com glyphs universais); `event_loop::run` sobrescreve com
    /// `octx.settings.glyph_mode` no boot real.
    pub glyph_mode: GlyphMode,
    /// Fila de eventos de efeito visual (T16). `update` empurra puro; o
    /// event_loop drena (`.drain(..)`) a cada iteração do loop e nunca deve
    /// deixá-la crescer sem limite entre frames.
    pub fx_queue: Vec<FxEvent>,
    /// Custo exibido no header (T16): persegue `usage.total_cost.usd` via
    /// lerp (fator 0.12/tick de ~30ms, snap quando a diferença < 0.01) —
    /// count-up visual. Com `animations=false`, `AnimTick` snapa direto pro
    /// alvo (sem lerp). No 1º load (`Action::UsageComputed` com
    /// `usage` ainda `None`) já nasce igual ao alvo — para não animar a
    /// partir de zero no primeiro paint.
    pub display_cost: f64,
    /// Gate de animações (`settings.menu.animations`, Task 15/16): controla
    /// o count-up de `display_cost` (o pulse crítico dos gauges morreu em
    /// v8 — spec §6; gauge agora é sólido, sem modulação de brilho).
    /// Default `true` (paridade com `MenuSettings::animations`);
    /// `event_loop::run` sobrescreve com `octx.settings.menu.animations`
    /// no boot real — mesmo padrão de `glyph_mode`/`local_offset`. NÃO
    /// gate os efeitos tachyonfx (esses são gate por
    /// `Effects::new(enabled)`, construído direto do settings no
    /// event_loop).
    pub animations: bool,
}

impl AppState {
    pub fn new() -> Self {
        Self {
            screen: Screen::Detail,
            providers: Vec::new(),
            selected: 0,
            sidebar_selected: 0,
            scroll: 0,
            status: FetchStatus::Idle,
            last_update: None,
            should_quit: false,
            usage: None,
            anim_frame: 0,
            throbber: ThrobberAnim::default(),
            config_state: None,
            login_selected: 0,
            login_status: None,
            history: None,
            history_range: HistoryRange::Week,
            history_selected: 0,
            history_expanded: BTreeSet::new(),
            show_help: false,
            fetch_pending: Vec::new(),
            pending_focus: None,
            pending_login: None,
            pending_save: false,
            hover: None,
            local_offset: time::UtcOffset::UTC,
            glyph_mode: GlyphMode::Box,
            fx_queue: Vec::new(),
            display_cost: 0.0,
            animations: true,
        }
    }
}

impl Default for AppState {
    fn default() -> Self {
        Self::new()
    }
}
