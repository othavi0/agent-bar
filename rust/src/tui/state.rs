use crate::providers::types::ProviderQuota;
use crate::usage::UsageSummary;

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
        }
    }
}

impl Default for AppState {
    fn default() -> Self {
        Self::new()
    }
}
