//! Closed v10 CLI command types.

use std::path::PathBuf;

/// Supported provider identifiers (closed catalog).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ProviderId {
    Claude,
    Codex,
    Amp,
    Grok,
}

impl ProviderId {
    pub const ALL: [ProviderId; 4] = [
        ProviderId::Claude,
        ProviderId::Codex,
        ProviderId::Amp,
        ProviderId::Grok,
    ];

    pub fn as_str(self) -> &'static str {
        match self {
            ProviderId::Claude => "claude",
            ProviderId::Codex => "codex",
            ProviderId::Amp => "amp",
            ProviderId::Grok => "grok",
        }
    }

    pub fn parse_word(word: &str) -> Option<Self> {
        match word {
            "claude" => Some(ProviderId::Claude),
            "codex" => Some(ProviderId::Codex),
            "amp" => Some(ProviderId::Amp),
            "grok" => Some(ProviderId::Grok),
            _ => None,
        }
    }
}

impl std::fmt::Display for ProviderId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Status stdout format.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StatusFormat {
    Human,
    Json,
}

/// Cache mode for status collection.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CacheMode {
    Use,
    Bypass,
}

/// Notification evaluation mode for status.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NotificationMode {
    Evaluate,
    Skip,
}

/// Options for `status` (and bare `agent-bar`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StatusOptions {
    pub format: StatusFormat,
    pub provider: Option<ProviderId>,
    pub cache: CacheMode,
    pub notifications: NotificationMode,
}

impl Default for StatusOptions {
    fn default() -> Self {
        Self {
            format: StatusFormat::Human,
            provider: None,
            cache: CacheMode::Use,
            notifications: NotificationMode::Skip,
        }
    }
}

/// Config subcommands backed by the canonical settings store (schema v1).
///
/// `show` is read-only. `apply` accepts exactly one complete document, validates
/// before taking the shared maintenance lock, and returns the stored JSON.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConfigCommand {
    Show,
    Apply(ConfigInput),
}

/// Input source for `config apply` (complete-document only).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConfigInput {
    Stdin,
    File(PathBuf),
    Json(String),
}

/// Setup modes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SetupOptions {
    Production,
    PluginsDir(PathBuf),
}

/// Update subcommands. `Apply` takes no argument: `update apply` is an
/// unconditional detached delegation to `omarchy plugin update
/// agent-bar.usage --yes` (git-plugin-distribution Task 2), not a
/// version-gated apply of a specific release.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UpdateCommand {
    Interactive,
    Check,
    Apply,
}

/// Doctor subcommands.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DoctorCommand {
    Scan,
    Clean,
}

/// Accepted `help <topic>` values.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HelpTopic {
    Status,
    Login,
    Config,
    Setup,
    Update,
    Uninstall,
    Doctor,
    Help,
    Version,
}

impl HelpTopic {
    pub const ALL: [HelpTopic; 9] = [
        HelpTopic::Status,
        HelpTopic::Login,
        HelpTopic::Config,
        HelpTopic::Setup,
        HelpTopic::Update,
        HelpTopic::Uninstall,
        HelpTopic::Doctor,
        HelpTopic::Help,
        HelpTopic::Version,
    ];

    pub fn as_str(self) -> &'static str {
        match self {
            HelpTopic::Status => "status",
            HelpTopic::Login => "login",
            HelpTopic::Config => "config",
            HelpTopic::Setup => "setup",
            HelpTopic::Update => "update",
            HelpTopic::Uninstall => "uninstall",
            HelpTopic::Doctor => "doctor",
            HelpTopic::Help => "help",
            HelpTopic::Version => "version",
        }
    }

    pub fn parse_word(word: &str) -> Option<Self> {
        match word {
            "status" => Some(HelpTopic::Status),
            "login" => Some(HelpTopic::Login),
            "config" => Some(HelpTopic::Config),
            "setup" => Some(HelpTopic::Setup),
            "update" => Some(HelpTopic::Update),
            "uninstall" => Some(HelpTopic::Uninstall),
            "doctor" => Some(HelpTopic::Doctor),
            "help" => Some(HelpTopic::Help),
            "version" => Some(HelpTopic::Version),
            _ => None,
        }
    }
}

/// Top-level parsed command.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Command {
    Status(StatusOptions),
    Login(ProviderId),
    Config(ConfigCommand),
    Setup(SetupOptions),
    Update(UpdateCommand),
    Uninstall { purge: bool },
    Doctor(DoctorCommand),
    Help(Option<HelpTopic>),
    Version,
}
