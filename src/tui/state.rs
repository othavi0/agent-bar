use crate::providers::types::ProviderQuota;
use crate::settings::Settings;
use crate::usage::{UsageRecord, UsageSummary};

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

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Tab {
    Dashboard,
    Waybar,
    History,
    Login,
}

impl Tab {
    /// Tabs in display order.
    pub const ALL: [Tab; 4] = [Tab::Dashboard, Tab::Waybar, Tab::History, Tab::Login];

    pub fn index(&self) -> usize {
        match self {
            Tab::Dashboard => 0,
            Tab::Waybar => 1,
            Tab::History => 2,
            Tab::Login => 3,
        }
    }

    pub fn from_index(i: usize) -> Self {
        match i % 4 {
            0 => Tab::Dashboard,
            1 => Tab::Waybar,
            2 => Tab::History,
            _ => Tab::Login,
        }
    }
}

/// Campo da aba Waybar config (ordem de exibicao = ordem dos enum variants).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConfigField {
    Providers,
    ProviderOrder,
    Separators,
    DisplayMode,
    FxRate,
    Signal,
    Interval,
}

impl ConfigField {
    pub const ALL: [ConfigField; 7] = [
        ConfigField::Providers,
        ConfigField::ProviderOrder,
        ConfigField::Separators,
        ConfigField::DisplayMode,
        ConfigField::FxRate,
        ConfigField::Signal,
        ConfigField::Interval,
    ];

    pub fn label(self) -> &'static str {
        match self {
            ConfigField::Providers => "providers",
            ConfigField::ProviderOrder => "providerOrder",
            ConfigField::Separators => "separators",
            ConfigField::DisplayMode => "displayMode",
            ConfigField::FxRate => "fxRate",
            ConfigField::Signal => "signal",
            ConfigField::Interval => "interval",
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
pub enum Panel {
    Sidebar,
    Content,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Mode {
    List,
    Detail,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FetchStatus {
    Idle,
    Loading,
    Loaded,
    Failed(String),
}

#[derive(Debug, Clone)]
pub struct ProviderView {
    pub quota: ProviderQuota,
    /// Animação A (gauge lerp): valor exibido do gauge, persegue o target via lerp.
    /// Inicializado = target no 1º load para não animar a partir de zero.
    pub display_ratio: f64,
}

impl ProviderView {
    pub fn new(quota: ProviderQuota) -> Self {
        let target = quota
            .primary
            .as_ref()
            .map(|w| w.remaining / 100.0)
            .unwrap_or(0.0)
            .clamp(0.0, 1.0);
        Self {
            quota,
            display_ratio: target,
        }
    }

    /// Percentual restante alvo (0.0-1.0) vindo do dado bruto do provider.
    pub fn target_ratio(&self) -> f64 {
        self.quota
            .primary
            .as_ref()
            .map(|w| w.remaining / 100.0)
            .unwrap_or(0.0)
            .clamp(0.0, 1.0)
    }
}

#[derive(Debug)]
pub struct AppState {
    pub tab: Tab,
    pub providers: Vec<ProviderView>,
    pub selected: usize,
    pub mode: Mode,
    pub focus: Panel,
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
    /// Indice selecionado na aba Login (0=claude, 1=codex, 2=amp).
    pub login_selected: usize,
    /// Mensagem de status da aba Login (feedback de erro ou instrucao).
    pub login_status: Option<String>,
    /// Records da aba History (ultimos 7 dias). Carregado via HistoryLoaded.
    pub history: Option<Vec<UsageRecord>>,
    /// Overlay de ajuda visivel (toggle via `?`, fecha com Esc ou `?`).
    pub show_help: bool,
    /// Ids de provider com fetch em voo (Task 5). Populado por `FetchStarted`,
    /// esvaziado incrementalmente por `ProviderFetched`/`FetchCompleted`.
    pub fetch_pending: Vec<String>,
}

impl AppState {
    pub fn new() -> Self {
        Self {
            tab: Tab::Dashboard,
            providers: Vec::new(),
            selected: 0,
            mode: Mode::List,
            focus: Panel::Sidebar,
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
            show_help: false,
            fetch_pending: Vec::new(),
        }
    }
}

impl Default for AppState {
    fn default() -> Self {
        Self::new()
    }
}
