#![cfg_attr(not(test), deny(clippy::unwrap_used, clippy::expect_used))]

use std::io::IsTerminal as _;
use std::path::PathBuf;
use std::time::Duration;

use agent_bar::app_identity;
use agent_bar::cache;
use agent_bar::cli::{self, Command, Format};
use agent_bar::config::{self, Paths};
use agent_bar::formatters::clock::Clock;
use agent_bar::formatters::json::to_json_string;
use agent_bar::formatters::terminal::format_for_terminal;
use agent_bar::formatters::waybar::{format_for_waybar, format_provider_for_waybar, WaybarOutput};
use agent_bar::notify;
use agent_bar::providers::types::AllQuotas;
use agent_bar::providers::{
    fetch_all, get_provider, get_quota_for, iso_from_ms, registered_provider_ids, registry, Ctx,
};
use agent_bar::settings::{self, Settings};
use agent_bar::watch;

// ---------------------------------------------------------------------------
// Helpers puros e testáveis
// ---------------------------------------------------------------------------

/// Notify dispara?
/// `settings.notify.enabled && cmd==Waybar && format!=Json && !watch && !stdout_tty`.
fn should_notify(
    settings: &Settings,
    command: Command,
    format: Format,
    watch: bool,
    stdout_is_tty: bool,
) -> bool {
    settings.notify.enabled
        && matches!(command, Command::Waybar)
        && format != Format::Json
        && !watch
        && !stdout_is_tty
}

/// Módulo Waybar oculto?
/// `format != Json && !settings.waybar.providers.contains(provider)`.
fn is_hidden_module(provider: &str, format: Format, settings: &Settings) -> bool {
    format != Format::Json && !settings.waybar.providers.iter().any(|s| s == provider)
}

/// Escreve o payload Waybar em stdout (stdout-limpo; serialização não falha).
fn print_waybar(o: &WaybarOutput) {
    println!("{}", serde_json::to_string(o).unwrap_or_default());
}

// ---------------------------------------------------------------------------
// Entry-point
// ---------------------------------------------------------------------------

#[tokio::main(flavor = "current_thread")]
async fn main() {
    let raw: Vec<String> = std::env::args().skip(1).collect();

    // 1. Parse de CLI — erros fatais → stderr + exit(1).
    let opts = match cli::parse_args(&raw) {
        Ok(o) => o,
        Err(e) => {
            eprintln!("{}", e.message);
            std::process::exit(1);
        }
    };

    // Avisos não-fatais → stderr.
    for w in &opts.warnings {
        eprintln!("{w}");
    }

    // 2. Logger.
    agent_bar::logger::init(opts.verbose);

    // 3. NO_COLOR.
    let no_color = std::env::var_os("NO_COLOR").is_some();

    // 4. Short-circuits cedo (ordem do TS).
    match opts.command {
        Command::Help => {
            cli::show_help(no_color);
            std::process::exit(0);
        }
        Command::Version => {
            println!("{}", app_identity::VERSION);
            std::process::exit(0);
        }
        // Stubs Plano 6 — comandos de instalação/TUI ainda não portados.
        Command::Menu => {
            log::error!("'menu' ainda não implementado na reescrita Rust (Plano 6).");
            std::process::exit(1);
        }
        Command::Setup => {
            log::error!("'setup' ainda não implementado na reescrita Rust (Plano 6).");
            std::process::exit(1);
        }
        Command::AssetsInstall => {
            log::error!("'assets install' ainda não implementado na reescrita Rust (Plano 6).");
            std::process::exit(1);
        }
        Command::ExportWaybarModules => {
            log::error!(
                "'export waybar-modules' ainda não implementado na reescrita Rust (Plano 6)."
            );
            std::process::exit(1);
        }
        Command::ExportWaybarCss => {
            log::error!("'export waybar-css' ainda não implementado na reescrita Rust (Plano 6).");
            std::process::exit(1);
        }
        Command::Update => {
            log::error!("'update' ainda não implementado na reescrita Rust (Plano 6).");
            std::process::exit(1);
        }
        Command::Uninstall => {
            log::error!("'uninstall' ainda não implementado na reescrita Rust (Plano 6).");
            std::process::exit(1);
        }
        Command::Remove => {
            log::error!("'remove' ainda não implementado na reescrita Rust (Plano 6).");
            std::process::exit(1);
        }
        Command::Doctor => {
            log::error!("'doctor' ainda não implementado na reescrita Rust (Plano 6).");
            std::process::exit(1);
        }
        // Outros comandos prosseguem abaixo.
        _ => {}
    }

    // 5. Construir Ctx.
    let paths = match Paths::from_env() {
        Ok(p) => p,
        Err(e) => {
            log::error!("{e}");
            std::process::exit(1);
        }
    };
    let settings = settings::load(&paths);
    let clock = Clock::from_env();
    let ctx = Ctx {
        client: agent_bar::http::client(),
        paths: &paths,
        settings: &settings,
        now_ms: config::now_ms(),
        local_offset: clock.local_offset,
        claude_usage_url: config::CLAUDE_USAGE_URL.to_string(),
        version: app_identity::VERSION,
        home: std::env::var_os("HOME")
            .map(PathBuf::from)
            .unwrap_or_default(),
    };

    // 6. ActionRight.
    if matches!(opts.command, Command::ActionRight) {
        agent_bar::action_right::handle_action_right(
            opts.provider.as_deref().unwrap_or(""),
            &ctx,
            &clock,
            no_color,
        )
        .await;
        std::process::exit(0);
    }

    // 7. Refresh (invalidar cache antes do fetch).
    if opts.refresh {
        let ids: Vec<&str> = match opts.provider.as_deref() {
            Some(p) => vec![p],
            None => registered_provider_ids(),
        };
        for id in ids {
            if let Some(prov) = get_provider(id) {
                cache::invalidate(&paths.cache_dir, prov.cache_key());
            }
        }
        log::info!("Cache invalidated");
    }

    // 8. Watch — antes do fetch.
    if opts.watch {
        if let Err(e) = watch::start_watch(
            opts.provider.as_deref(),
            Duration::from_secs(u64::from(opts.interval_seconds)),
            &ctx,
        )
        .await
        {
            log::error!("{e}");
            std::process::exit(1);
        }
        return;
    }

    // 9. Fetch de quotas.
    let quotas: AllQuotas = if let Some(prov) = &opts.provider {
        // Hidden-module short-circuit (Waybar com provider desabilitado nas settings).
        if is_hidden_module(prov, opts.format, &settings) {
            print_waybar(&WaybarOutput {
                text: String::new(),
                tooltip: String::new(),
                class: app_identity::APP_HIDDEN_CLASS.to_string(),
                alt: None,
                percentage: None,
            });
            std::process::exit(0);
        }

        let quota = match get_quota_for(prov, &ctx).await {
            Some(q) => q,
            None => {
                log::error!("Unknown provider: {prov}");
                std::process::exit(1);
            }
        };
        AllQuotas {
            providers: vec![quota],
            fetched_at: iso_from_ms(config::now_ms()),
        }
    } else {
        let mut all = fetch_all(&registry(), &ctx).await;
        // Waybar: filtra pelos providers habilitados nas settings (exceto JSON).
        if matches!(opts.command, Command::Waybar) && opts.format != Format::Json {
            all.providers
                .retain(|p| settings.waybar.providers.iter().any(|s| s == &p.provider));
        }
        all
    };

    // 10. JSON — sai antes do dispatch de UI.
    if opts.format == Format::Json {
        println!("{}", to_json_string(&quotas).unwrap_or_default());
        std::process::exit(0);
    }

    // 11. Dispatch final de UI.
    let mode = settings.waybar.display_mode;

    match opts.command {
        Command::Terminal | Command::Status => {
            println!(
                "{}",
                format_for_terminal(&clock, &quotas, &settings, mode, no_color)
            );
        }
        _ => {
            // Waybar (default) — ou interativo sem args → ajuda.
            let stdout_tty = std::io::stdout().is_terminal();
            if stdout_tty && raw.is_empty() {
                cli::show_help(no_color);
            } else {
                if opts.provider.is_some() && quotas.providers.len() == 1 {
                    print_waybar(&format_provider_for_waybar(
                        &clock,
                        &quotas.providers[0],
                        &settings,
                        mode,
                    ));
                } else {
                    print_waybar(&format_for_waybar(&clock, &quotas, &settings, mode));
                }

                if should_notify(&settings, opts.command, opts.format, opts.watch, stdout_tty) {
                    notify::check_and_notify(&quotas, &paths.cache_dir).await;
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Testes dos helpers puros
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use agent_bar::config::Paths;
    use agent_bar::settings::{load, Notify, Settings};
    use std::path::PathBuf;
    use tempfile::tempdir;

    // -----------------------------------------------------------------------
    // Helpers de setup
    // -----------------------------------------------------------------------

    fn make_paths(dir: &std::path::Path) -> Paths {
        Paths {
            cache_dir: dir.join("cache"),
            config_dir: dir.join("config"),
            claude_credentials: PathBuf::new(),
            codex_auth: PathBuf::new(),
            codex_sessions: PathBuf::new(),
            amp_settings: PathBuf::new(),
            amp_threads: PathBuf::new(),
        }
    }

    fn default_settings() -> Settings {
        let dir = tempdir().unwrap();
        load(&make_paths(dir.path()))
    }

    fn settings_with_notify(enabled: bool) -> Settings {
        let mut s = default_settings();
        s.notify = Notify { enabled };
        s
    }

    fn settings_without_provider(provider: &str) -> Settings {
        let mut s = default_settings();
        s.waybar.providers.retain(|p| p != provider);
        s
    }

    // -----------------------------------------------------------------------
    // should_notify
    // -----------------------------------------------------------------------

    #[test]
    fn should_notify_true_when_all_conditions_met() {
        let s = settings_with_notify(true);
        assert!(should_notify(
            &s,
            Command::Waybar,
            Format::Waybar,
            false,
            false
        ));
    }

    #[test]
    fn should_notify_false_when_stdout_is_tty() {
        let s = settings_with_notify(true);
        assert!(!should_notify(
            &s,
            Command::Waybar,
            Format::Waybar,
            false,
            true
        ));
    }

    #[test]
    fn should_notify_false_when_format_is_json() {
        let s = settings_with_notify(true);
        assert!(!should_notify(
            &s,
            Command::Waybar,
            Format::Json,
            false,
            false
        ));
    }

    #[test]
    fn should_notify_false_when_watch_is_true() {
        let s = settings_with_notify(true);
        assert!(!should_notify(
            &s,
            Command::Waybar,
            Format::Waybar,
            true,
            false
        ));
    }

    #[test]
    fn should_notify_false_when_command_is_terminal() {
        let s = settings_with_notify(true);
        assert!(!should_notify(
            &s,
            Command::Terminal,
            Format::Waybar,
            false,
            false
        ));
    }

    #[test]
    fn should_notify_false_when_notify_disabled() {
        let s = settings_with_notify(false);
        assert!(!should_notify(
            &s,
            Command::Waybar,
            Format::Waybar,
            false,
            false
        ));
    }

    // -----------------------------------------------------------------------
    // is_hidden_module
    // -----------------------------------------------------------------------

    #[test]
    fn is_hidden_when_provider_not_in_settings() {
        // Remove "amp" dos providers para forçar hidden.
        let s = settings_without_provider("amp");
        assert!(is_hidden_module("amp", Format::Waybar, &s));
    }

    #[test]
    fn not_hidden_when_provider_is_in_settings() {
        let s = default_settings();
        // "claude" está nos defaults.
        assert!(!is_hidden_module("claude", Format::Waybar, &s));
    }

    #[test]
    fn not_hidden_when_format_is_json_even_if_provider_absent() {
        let s = settings_without_provider("amp");
        // JSON bypass: nunca hidden.
        assert!(!is_hidden_module("amp", Format::Json, &s));
    }

    // -----------------------------------------------------------------------
    // Sanidade: settings de test_support têm os defaults esperados
    // -----------------------------------------------------------------------

    #[test]
    fn default_settings_have_known_providers() {
        let s = default_settings();
        assert!(s.waybar.providers.contains(&"claude".to_string()));
        assert!(s.notify.enabled);
    }

    // -----------------------------------------------------------------------
    // print_waybar — garante que serializa sem panic (sem stdout real)
    // -----------------------------------------------------------------------

    #[test]
    fn waybar_output_serializes_without_panic() {
        let o = WaybarOutput {
            text: "test".to_string(),
            tooltip: "tip".to_string(),
            class: "cls".to_string(),
            alt: None,
            percentage: None,
        };
        let json = serde_json::to_string(&o).unwrap_or_default();
        assert!(json.contains("\"text\":\"test\""));
        assert!(json.contains("\"tooltip\":\"tip\""));
        assert!(!json.contains("alt")); // skip_serializing_if None
    }

    #[test]
    fn waybar_hidden_output_has_hidden_class() {
        let o = WaybarOutput {
            text: String::new(),
            tooltip: String::new(),
            class: app_identity::APP_HIDDEN_CLASS.to_string(),
            alt: None,
            percentage: None,
        };
        let json = serde_json::to_string(&o).unwrap_or_default();
        assert!(json.contains(app_identity::APP_HIDDEN_CLASS));
    }
}
