#![cfg_attr(not(test), deny(clippy::unwrap_used, clippy::expect_used))]

use std::io::IsTerminal as _;
use std::path::{Path, PathBuf};
use std::time::Duration;

use agent_bar::app_identity::{self, APP_NAME};
use agent_bar::cache;
use agent_bar::cli::{self, Command, Format};
use agent_bar::config::{self, Paths};
use agent_bar::formatters::clock::Clock;
use agent_bar::formatters::json::to_json_string;
use agent_bar::formatters::terminal::format_for_terminal;
use agent_bar::formatters::waybar::{
    format_for_waybar, format_provider_for_waybar, waybar_stdout_line, WaybarOutput,
};
use agent_bar::notify;
use agent_bar::providers::types::AllQuotas;
use agent_bar::providers::{
    fetch_all, get_provider, get_quota_for, iso_from_ms, registered_provider_ids, registry, Ctx,
};
use agent_bar::settings::{self, Settings};
use agent_bar::tui;
use agent_bar::watch;
use agent_bar::{
    doctor, install, omarchy_integration, runtime, setup, term_prompt, uninstall, update,
    waybar_contract, waybar_integration,
};

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

/// Escreve o payload Waybar em stdout (stdout-limpo; nunca vazio em falha de serde).
fn print_waybar(o: &WaybarOutput) {
    let mut err = std::io::stderr();
    println!("{}", waybar_stdout_line(o, &mut err));
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
        // ----------------------------------------------------------------
        // Comandos de instalação — não precisam de Ctx HTTP.
        // ----------------------------------------------------------------
        Command::AssetsInstall => {
            let defaults = waybar_contract::get_default_waybar_asset_paths();
            let waybar_dir = opts
                .waybar_dir
                .as_deref()
                .map(PathBuf::from)
                .unwrap_or(defaults.waybar_dir);
            let scripts_dir = opts
                .scripts_dir
                .as_deref()
                .map(PathBuf::from)
                .unwrap_or(defaults.scripts_dir);
            match waybar_contract::install_waybar_assets(&waybar_dir, &scripts_dir, None) {
                Ok(r) => {
                    println!("{}", serde_json::to_string(&r).unwrap_or_default());
                    std::process::exit(0);
                }
                Err(e) => {
                    log::error!("{e}");
                    std::process::exit(1);
                }
            }
        }

        Command::ExportWaybarModules => {
            let paths = match Paths::from_env() {
                Ok(p) => p,
                Err(e) => {
                    log::error!("{e}");
                    std::process::exit(1);
                }
            };
            let settings = settings::load(&paths);
            let defaults = waybar_contract::get_default_waybar_asset_paths();
            let app_bin = opts.app_bin.clone().unwrap_or(defaults.app_bin);
            let terminal_script = opts
                .terminal_script
                .as_deref()
                .map(PathBuf::from)
                .unwrap_or(defaults.terminal_script);
            let term_str = terminal_script.to_string_lossy().to_string();
            let export = waybar_contract::export_waybar_modules(
                &app_bin,
                &term_str,
                settings.waybar.signal,
                &settings.waybar.provider_order,
                settings.waybar.interval,
            );
            println!(
                "{}",
                serde_json::to_string_pretty(&export).unwrap_or_default()
            );
            std::process::exit(0);
        }

        Command::ExportWaybarCss => {
            let paths = match Paths::from_env() {
                Ok(p) => p,
                Err(e) => {
                    log::error!("{e}");
                    std::process::exit(1);
                }
            };
            let settings = settings::load(&paths);
            let defaults = waybar_contract::get_default_waybar_asset_paths();
            let icons_dir = opts
                .icons_dir
                .as_deref()
                .map(PathBuf::from)
                .unwrap_or(defaults.icons_dir);
            let css = waybar_contract::export_waybar_css(
                &icons_dir.to_string_lossy(),
                &settings.waybar.provider_order,
                settings.waybar.separators,
            );
            let wrapped = serde_json::json!({ "css": css });
            println!(
                "{}",
                serde_json::to_string_pretty(&wrapped).unwrap_or_default()
            );
            std::process::exit(0);
        }

        Command::Setup => {
            let paths = match Paths::from_env() {
                Ok(p) => p,
                Err(e) => {
                    log::error!("{e}");
                    std::process::exit(1);
                }
            };
            let settings = settings::load(&paths);
            let asset_paths = waybar_contract::get_default_waybar_asset_paths();
            let ipaths = waybar_integration::get_default_waybar_integration_paths();
            let home = std::env::var_os("HOME")
                .map(PathBuf::from)
                .unwrap_or_default();
            let omarchy_forced = opts.omarchy_plugins_dir.as_ref().map(PathBuf::from);
            let omarchy_detected = omarchy_integration::detect_omarchy_shell();
            let omarchy = match (omarchy_forced, omarchy_detected) {
                (Some(dir), _) => Some(setup::OmarchySetupOptions {
                    plugins_dir: dir,
                    run_cli: false, // dir injetado = teste/CI: não toca o shell vivo
                }),
                (None, true) => Some(setup::OmarchySetupOptions {
                    plugins_dir: omarchy_integration::default_omarchy_plugins_dir(&home),
                    run_cli: true,
                }),
                (None, false) => None,
            };
            let skip_waybar =
                omarchy.is_some() && !setup::waybar_present(std::env::var_os("PATH").as_deref());
            let cfg = setup::SetupConfig {
                asset_paths: Some(asset_paths),
                integration_paths: Some(ipaths),
                repo_root: None,
                home,
                skip_reload: false,
                system_install: runtime::is_system_install(),
                omarchy,
                skip_waybar,
            };
            match setup::run_setup(&settings, cfg, true, true) {
                Ok(_) => std::process::exit(0),
                Err(e) => {
                    log::error!("{e}");
                    std::process::exit(1);
                }
            }
        }

        // Comando interno oculto — usado só pelo helper Bash para saber a
        // fonte do menu. stdout LIMPO: exatamente 1 linha TSV (contrato do
        // helper).
        Command::MenuFont => {
            let paths = match Paths::from_env() {
                Ok(p) => p,
                Err(e) => {
                    log::error!("{e}");
                    std::process::exit(1);
                }
            };
            let settings = settings::load(&paths);
            println!("{}\t{}", settings.menu.font_family, settings.menu.font_size);
            std::process::exit(0);
        }

        Command::Doctor => {
            let home = std::env::var_os("HOME")
                .map(PathBuf::from)
                .unwrap_or_default();
            let dry_run = opts.dry_run;
            let yes = opts.yes;
            let confirm = |f: &doctor::DoctorFindings| {
                let items: Vec<String> = [
                    f.package_json_path
                        .as_ref()
                        .filter(|_| f.package_json_orphan)
                        .map(|p| p.to_string_lossy().into_owned()),
                    f.node_modules_dir
                        .as_ref()
                        .map(|p| p.to_string_lossy().into_owned()),
                ]
                .into_iter()
                .flatten()
                .chain(f.lockfiles.iter().map(|p| p.to_string_lossy().into_owned()))
                .collect();
                if !items.is_empty() {
                    term_prompt::note(&format!(
                        "Leftovers encontrados:\n  • {}",
                        items.join("\n  • ")
                    ));
                }
                let msg = if dry_run {
                    "Show what would be removed?"
                } else {
                    "Remove the leftovers above?"
                };
                term_prompt::confirm(msg, true)
            };
            let result = doctor::run_doctor(doctor::DoctorOptions {
                home: &home,
                dry_run,
                yes,
                confirm: &confirm,
            });
            match result.status {
                doctor::DoctorStatus::Clean => {
                    term_prompt::status("OK", "Nothing to clean");
                }
                doctor::DoctorStatus::Cleaned => {
                    term_prompt::status("OK", &format!("{} paths removidos", result.removed.len()));
                }
                doctor::DoctorStatus::MixedOnly => {
                    term_prompt::status(
                        "Info",
                        "package.json tem outras dependências além do agent-bar; remoção pulada",
                    );
                }
                doctor::DoctorStatus::Cancelled => {
                    term_prompt::status("Cancelado", "Doctor não aplicou mudanças");
                }
            }
            std::process::exit(0);
        }

        Command::Update => {
            let paths = match Paths::from_env() {
                Ok(p) => p,
                Err(e) => {
                    log::error!("{e}");
                    std::process::exit(1);
                }
            };
            let settings = settings::load(&paths);
            let home = std::env::var_os("HOME")
                .map(PathBuf::from)
                .unwrap_or_default();
            let install_root = home.join(format!(".{APP_NAME}"));

            // O binário novo traz QML novo — o drop-in do omarchy-shell só
            // atualiza via setup (o update não toca nele; ver setup::SetupConfig.omarchy).
            let omarchy_setup_hint = |home: &Path| {
                let plugin_dir = omarchy_integration::default_omarchy_plugins_dir(home)
                    .join(app_identity::OMARCHY_PLUGIN_ID);
                if plugin_dir.exists() {
                    term_prompt::note(&format!(
                        "Plugin omarchy-shell detectado. Rode `{} setup` para atualizá-lo.",
                        app_identity::APP_NAME
                    ));
                }
            };

            // Detecção pelo binário real, não por CARGO_MANIFEST_DIR (compile-time —
            // aponta pro runner do CI, não pra máquina do usuário; hotfix 7.0.1).
            let current_exe = match std::env::current_exe().and_then(|p| p.canonicalize()) {
                Ok(p) => p,
                Err(e) => {
                    log::error!("Failed to resolve current executable: {e}");
                    std::process::exit(1);
                }
            };
            let repo_root = update::find_repo_root(&current_exe);

            fn run_real_command(cmd: &str, args: &[String], cwd: &Path) -> update::CommandResult {
                match std::process::Command::new(cmd)
                    .args(args)
                    .current_dir(cwd)
                    .output()
                {
                    Ok(out) => {
                        let mut combined = String::from_utf8_lossy(&out.stdout).into_owned();
                        let stderr = String::from_utf8_lossy(&out.stderr);
                        if !stderr.is_empty() {
                            if !combined.is_empty() {
                                combined.push('\n');
                            }
                            combined.push_str(&stderr);
                        }
                        update::CommandResult {
                            ok: out.status.success(),
                            output: combined,
                        }
                    }
                    Err(e) => update::CommandResult {
                        ok: false,
                        output: e.to_string(),
                    },
                }
            }

            match update::detect_install_kind(repo_root.as_deref(), &install_root) {
                update::InstallKind::System => {
                    term_prompt::status(
                        "Info",
                        "Instalação de sistema detectada. Use o AUR helper (ex: yay -Syu agent-bar) para atualizar.",
                    );
                    std::process::exit(0);
                }
                update::InstallKind::DevGit => {
                    if let Some(root) = &repo_root {
                        if let Ok(v) = update::read_cargo_version(root) {
                            log::debug!(
                                "Dev checkout detectado: {} (Cargo.toml v{v})",
                                root.display()
                            );
                        }
                    }
                    log::error!(
                        "Dev checkout detectado. Use `git pull` na raiz do repositório para atualizar."
                    );
                    std::process::exit(1);
                }
                update::InstallKind::ManagedGit => {
                    let Some(root) = repo_root.clone() else {
                        log::error!("Internal error: ManagedGit sem repo_root resolvido.");
                        std::process::exit(1);
                    };
                    if let Ok(v) = update::read_cargo_version(&root) {
                        log::debug!("Managed checkout: {} (Cargo.toml v{v})", root.display());
                    }
                    let settings_for_setup = settings.clone();
                    let repo_root_for_setup = root.clone();
                    let home_for_setup = home.clone();
                    let run_setup = || {
                        let asset_paths = waybar_contract::get_default_waybar_asset_paths();
                        let ipaths = waybar_integration::get_default_waybar_integration_paths();
                        let cfg = setup::SetupConfig {
                            asset_paths: Some(asset_paths),
                            integration_paths: Some(ipaths),
                            repo_root: Some(repo_root_for_setup.clone()),
                            home: home_for_setup.clone(),
                            skip_reload: false,
                            system_install: runtime::is_system_install(),
                            omarchy: None,
                            skip_waybar: false,
                        };
                        if let Err(e) = setup::run_setup(&settings_for_setup, cfg, false, false) {
                            log::error!("Setup falhou após update: {e}");
                        }
                    };
                    let confirm_managed = |summary: &update::UpdateSummary| {
                        if !summary.commits.is_empty() {
                            term_prompt::note(&format!(
                                "Commits disponíveis:\n  {}",
                                summary.commits.join("\n  ")
                            ));
                        }
                        term_prompt::confirm("Aplicar update?", true)
                    };
                    match update::run_managed_update(update::ManagedUpdateOptions {
                        repo_root: &root,
                        install_root: &install_root,
                        run_command: &run_real_command,
                        run_setup: &run_setup,
                        confirm: &confirm_managed,
                    }) {
                        Ok(r) => {
                            match r.status {
                                update::ManagedUpdateStatus::Updated => {
                                    term_prompt::status("OK", "Update aplicado");
                                    omarchy_setup_hint(&home);
                                }
                                update::ManagedUpdateStatus::UpToDate => {
                                    term_prompt::status("OK", "Já na versão mais recente");
                                }
                                update::ManagedUpdateStatus::Cancelled => {
                                    term_prompt::status("Cancelado", "Update não aplicado");
                                }
                                update::ManagedUpdateStatus::WrongRoot => {
                                    log::error!("Diretório de instalação não corresponde ao repo.");
                                    std::process::exit(1);
                                }
                            }
                            std::process::exit(0);
                        }
                        Err(e) => {
                            log::error!("{e}");
                            std::process::exit(1);
                        }
                    }
                }
                update::InstallKind::Standalone => {
                    let http = match reqwest::Client::builder()
                        .use_rustls_tls()
                        .user_agent(update::SELFUPDATE_USER_AGENT)
                        .timeout(Duration::from_secs(60))
                        .build()
                    {
                        Ok(c) => c,
                        Err(e) => {
                            log::error!("Failed to build HTTP client: {e}");
                            std::process::exit(1);
                        }
                    };
                    let data_dir = update::default_data_dir(&home);
                    let waybar_paths = waybar_contract::get_default_waybar_asset_paths();
                    let opts = update::StandaloneUpdateOptions {
                        current_version: app_identity::VERSION,
                        exe_path: &current_exe,
                        data_dir: &data_dir,
                        waybar_dir: &waybar_paths.waybar_dir,
                        scripts_dir: &waybar_paths.scripts_dir,
                        run_command: &run_real_command,
                        http: &http,
                        releases_api_url: format!(
                            "https://api.github.com/repos/{}/releases/latest",
                            update::GITHUB_REPO
                        ),
                        download_base_url: format!(
                            "https://github.com/{}/releases/download",
                            update::GITHUB_REPO
                        ),
                        has_sha256sum: &|| install::has_cmd("sha256sum"),
                        has_tar: &|| install::has_cmd("tar"),
                    };
                    match update::run_standalone_update(opts).await {
                        Ok(update::StandaloneUpdateStatus::UpToDate { version }) => {
                            term_prompt::status(
                                "OK",
                                &format!("Já está na última versão (v{version})"),
                            );
                            std::process::exit(0);
                        }
                        Ok(update::StandaloneUpdateStatus::Updated {
                            old_version,
                            new_version,
                        }) => {
                            term_prompt::status(
                                "OK",
                                &format!(
                                    "agent-bar atualizado: v{old_version} -> v{new_version}. Icons e helper da Waybar atualizados."
                                ),
                            );
                            omarchy_setup_hint(&home);
                            std::process::exit(0);
                        }
                        Err(e) => {
                            log::error!("{e}");
                            std::process::exit(1);
                        }
                    }
                }
            }
        }

        Command::Uninstall => {
            let paths = match Paths::from_env() {
                Ok(p) => p,
                Err(e) => {
                    log::error!("{e}");
                    std::process::exit(1);
                }
            };
            let home = std::env::var_os("HOME")
                .map(PathBuf::from)
                .unwrap_or_default();
            let settings_dir = home.join(".config").join(APP_NAME);
            let ipaths = waybar_integration::get_default_waybar_integration_paths();
            match uninstall::run_uninstall(
                &settings_dir,
                &paths.cache_dir,
                &home,
                false,
                &format!("{APP_NAME} uninstall"),
                &ipaths,
                &omarchy_integration::default_omarchy_plugins_dir(&home),
            ) {
                Ok(()) => std::process::exit(0),
                Err(e) => {
                    log::error!("{e}");
                    std::process::exit(1);
                }
            }
        }

        Command::Remove => {
            let paths = match Paths::from_env() {
                Ok(p) => p,
                Err(e) => {
                    log::error!("{e}");
                    std::process::exit(1);
                }
            };
            let home = std::env::var_os("HOME")
                .map(PathBuf::from)
                .unwrap_or_default();
            let settings_dir = home.join(".config").join(APP_NAME);
            let ipaths = waybar_integration::get_default_waybar_integration_paths();
            match uninstall::run_uninstall(
                &settings_dir,
                &paths.cache_dir,
                &home,
                true,
                &format!("{APP_NAME} remove"),
                &ipaths,
                &omarchy_integration::default_omarchy_plugins_dir(&home),
            ) {
                Ok(()) => std::process::exit(0),
                Err(e) => {
                    log::error!("{e}");
                    std::process::exit(1);
                }
            }
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

    // 6. Menu → abre a TUI.
    if matches!(opts.command, Command::Menu) {
        if let Err(e) = tui::run_tui(&ctx, None).await {
            log::error!("TUI encerrou com erro: {e}");
            std::process::exit(1);
        }
        std::process::exit(0);
    }

    // 6b. ActionRight → resolve foco e abre a TUI focada.
    if matches!(opts.command, Command::ActionRight) {
        let provider = opts.provider.as_deref().unwrap_or("");
        match agent_bar::action_right::action_right_focus(provider, &ctx).await {
            Some(focus) => {
                if let Err(e) = tui::run_tui(&ctx, Some(focus)).await {
                    log::error!("TUI encerrou com erro: {e}");
                    std::process::exit(1);
                }
            }
            None => std::process::exit(1),
        }
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
            // Waybar (default) — ou interativo sem args → TUI.
            let stdout_tty = std::io::stdout().is_terminal();
            if stdout_tty && raw.is_empty() {
                if let Err(e) = tui::run_tui(&ctx, None).await {
                    log::error!("TUI encerrou com erro: {e}");
                    std::process::exit(1);
                }
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
            grok_home: PathBuf::new(),
            grok_auth: PathBuf::new(),
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
