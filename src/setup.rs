//! Port de `src/setup.ts` — setup interativo leve (terminal-lite, sem @clack).

use std::io::IsTerminal;
use std::os::unix::fs::symlink;
use std::path::{Path, PathBuf};
use std::process::Stdio;

use crate::app_identity::APP_NAME;
use crate::omarchy_integration::{install_omarchy_plugin, run_omarchy_enable_commands};
use crate::settings::Settings;
use crate::waybar_contract::{
    get_default_waybar_asset_paths, install_waybar_assets, WaybarAssetPaths,
};
use crate::waybar_integration::{
    apply_waybar_integration, get_default_waybar_integration_paths, ApplyOptions,
    WaybarIntegrationPaths,
};
use crate::{doctor, term_prompt};

/// Recarrega o Waybar enviando SIGUSR2. Ignora qualquer erro (processo pode não estar rodando).
pub fn reload_waybar() {
    let _ = std::process::Command::new("pkill")
        .args(["-SIGUSR2", "waybar"])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn();
}

/// Waybar presente = binário `waybar` em `path_var`. Sinal para o setup
/// decidir se o fluxo Waybar roda (em Omarchy 4 puro, não há waybar).
pub fn waybar_present(path_var: Option<&std::ffi::OsStr>) -> bool {
    let Some(path_var) = path_var else {
        return false;
    };
    std::env::split_paths(path_var).any(|dir| dir.join("waybar").is_file())
}

/// Cria symlink `~/.local/bin/agent-bar` → binário compilado (`current_exe`).
/// Só chamado em instalações dev (não-system).
pub fn create_symlink(home: &Path) -> std::io::Result<PathBuf> {
    let local_bin = home.join(".local").join("bin");
    std::fs::create_dir_all(&local_bin)?;

    let link = local_bin.join(APP_NAME);

    // Pós-cutover: o symlink dev aponta pro binário compilado que está rodando
    // `setup` (antes apontava pro shim bun `scripts/agent-bar`, removido no cutover).
    let target = std::env::current_exe()?;

    // Se o binário JÁ está no destino do link (ex: `install.sh` instalou direto em
    // ~/.local/bin/agent-bar e roda `setup` dali), NÃO symlinkar: removeríamos o
    // binário e criaríamos um symlink apontando pra si mesmo (dangling). Já está no lugar.
    if let (Ok(t), Ok(l)) = (target.canonicalize(), link.canonicalize()) {
        if t == l {
            return Ok(link);
        }
    }

    // Remove symlink anterior se existir (ignora erro de não-existência)
    let _ = std::fs::remove_file(&link);

    symlink(&target, &link)?;
    Ok(link)
}

/// Instalação do plugin omarchy-shell dentro do setup.
pub struct OmarchySetupOptions {
    /// Destino dos plugins (`~/.config/omarchy/plugins` em produção;
    /// temp dir em teste via `--omarchy-plugins-dir`).
    pub plugins_dir: PathBuf,
    /// Roda `omarchy plugin rescan/enable` + `bar plugin add` após
    /// escrever os arquivos. SEMPRE false em testes.
    pub run_cli: bool,
}

/// Configuração injetável de `run_setup` — seams p/ testes.
pub struct SetupConfig {
    /// Paths de assets (None = usa `get_default_waybar_asset_paths()`).
    pub asset_paths: Option<WaybarAssetPaths>,
    /// Paths de integração (None = usa `get_default_waybar_integration_paths()`).
    pub integration_paths: Option<WaybarIntegrationPaths>,
    /// Raiz do repositório para `install_waybar_assets` (None = detecta automático).
    pub repo_root: Option<PathBuf>,
    /// Home do usuário.
    pub home: PathBuf,
    /// Se true, pula `reload_waybar()` (útil em testes).
    pub skip_reload: bool,
    /// Instalação de sistema (ex: AUR/pacote): não cria symlink em ~/.local/bin.
    /// O `main` (T7) passa `runtime::is_system_install()`; testes passam explicitamente.
    pub system_install: bool,
    /// `Some` = instala o plugin do omarchy-shell.
    pub omarchy: Option<OmarchySetupOptions>,
    /// `true` = pula o fluxo Waybar inteiro (assets + wiring + reload).
    /// Usado quando só o omarchy-shell foi detectado.
    pub skip_waybar: bool,
}

/// Port de `runSetup` (src/setup.ts:49-177), versão terminal-lite.
/// Retorna `Ok(true)` se setup foi executado, `Ok(false)` se cancelado pelo usuário.
pub fn run_setup(
    settings: &Settings,
    cfg: SetupConfig,
    confirm: bool,
    clear_screen: bool,
) -> anyhow::Result<bool> {
    // 1. Clear screen (no-op em não-TTY p/ não quebrar testes)
    if clear_screen && std::io::stdout().is_terminal() {
        print!("\x1b[2J\x1b[H");
    }

    // 2. Nota descrevendo as ações
    let target = match (&cfg.omarchy, cfg.skip_waybar) {
        (Some(_), true) => "integração omarchy-shell",
        (Some(_), false) => "integração Waybar + omarchy-shell",
        (None, _) => "integração Waybar",
    };
    term_prompt::note(&format!(
        "{APP_NAME} setup — instalando icons, helper e {target}"
    ));

    // 3. Confirmação interativa
    if confirm {
        let proceed = term_prompt::confirm(&format!("Apply {APP_NAME} setup now?"), true);
        if !proceed {
            term_prompt::status("Cancelado", "Setup não aplicado");
            return Ok(false);
        }
    }

    // 5. Symlink (somente em instalação dev) — fora do gate Waybar: a
    // instalação dev precisa do binário no PATH também para o QML omarchy.
    if !cfg.system_install {
        create_symlink(&cfg.home).map_err(|e| anyhow::anyhow!("Falha ao criar symlink: {e}"))?;

        // Aviso de PATH ausente (port de setup.ts:142-152): em instalação dev o
        // symlink fica em ~/.local/bin, que pode não estar no $PATH.
        let local_bin = cfg.home.join(".local").join("bin");
        let on_path = std::env::var("PATH")
            .is_ok_and(|path| std::env::split_paths(&path).any(|dir| dir == local_bin));
        if !on_path {
            term_prompt::note(&format!(
                "{} não está no seu PATH. Adicione ao perfil do shell:\n  export PATH=\"$HOME/.local/bin:$PATH\"",
                local_bin.display()
            ));
        }
    }

    if !cfg.skip_waybar {
        // 4. Instala assets (icons + terminal helper)
        let asset = cfg
            .asset_paths
            .unwrap_or_else(get_default_waybar_asset_paths);
        let installed = install_waybar_assets(
            &asset.waybar_dir,
            &asset.scripts_dir,
            cfg.repo_root.as_deref(),
        )?;

        // 6. Wiring Waybar config e style
        let ipaths = cfg
            .integration_paths
            .unwrap_or_else(get_default_waybar_integration_paths);
        apply_waybar_integration(
            settings,
            ApplyOptions {
                paths: ipaths,
                icons_dir: Some(installed.icons_dir.clone()),
                app_bin: Some(asset.app_bin.clone()),
                terminal_script: Some(installed.terminal_script.clone()),
            },
        )?;

        // 7. Reload Waybar
        if !cfg.skip_reload {
            reload_waybar();
        }

        term_prompt::status("Icons", &installed.icons_dir.to_string_lossy());
        term_prompt::status("Helper", &installed.terminal_script.to_string_lossy());
    }

    // Omarchy-shell: escreve o drop-in e (fora de testes) ativa via CLI.
    if let Some(om) = &cfg.omarchy {
        let installed = install_omarchy_plugin(&om.plugins_dir)?;
        term_prompt::status("Omarchy", &installed.plugin_dir.to_string_lossy());
        if om.run_cli {
            for warning in run_omarchy_enable_commands() {
                term_prompt::status("Aviso", &warning);
            }
        }
    }

    // 8. Status de sucesso + scan de leftovers (best-effort, NUNCA falha o setup)
    term_prompt::status("OK", &format!("{APP_NAME} setup completo"));

    // Scan best-effort: ignora qualquer erro
    let findings = doctor::scan(&cfg.home);
    let has_leftovers = findings.package_json_orphan
        || findings.package_json_mixed
        || findings.node_modules_dir.is_some()
        || !findings.lockfiles.is_empty();
    if has_leftovers {
        term_prompt::status(
            "Aviso",
            &format!(
                "Detectada instalação residual em $HOME. Execute `{APP_NAME} doctor` para limpar."
            ),
        );
    }

    // 9. Retorna true
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Paths;
    use crate::settings::load;
    use crate::waybar_contract::WaybarAssetPaths;
    use crate::waybar_integration::WaybarIntegrationPaths;
    use std::path::PathBuf;
    use tempfile::tempdir;

    #[test]
    #[serial_test::serial]
    fn setup_system_install_skips_symlink_uses_path_appbin() {
        let repo = tempdir().unwrap(); // fixture de assets
        std::fs::create_dir_all(repo.path().join("icons")).unwrap();
        std::fs::write(repo.path().join("icons").join("a.png"), b"x").unwrap();
        std::fs::create_dir_all(repo.path().join("scripts")).unwrap();
        std::fs::write(
            repo.path().join("scripts").join("agent-bar-open-terminal"),
            b"#!/bin/sh\n",
        )
        .unwrap();

        let dest = tempdir().unwrap();
        let asset_paths = WaybarAssetPaths {
            waybar_dir: dest.path().join("agent-bar"),
            scripts_dir: dest.path().join("scripts"),
            icons_dir: dest.path().join("agent-bar").join("icons"),
            terminal_script: dest.path().join("scripts").join("agent-bar-open-terminal"),
            app_bin: "agent-bar".to_string(), // system
        };
        let ipaths = WaybarIntegrationPaths {
            waybar_config_path: dest.path().join("config.jsonc"),
            waybar_style_path: dest.path().join("style.css"),
            modules_include_path: dest.path().join("agent-bar").join("modules.jsonc"),
            style_include_path: dest.path().join("agent-bar").join("style.css"),
        };
        let s = load(&Paths {
            cache_dir: dest.path().join("c"),
            config_dir: dest.path().join("cfg"),
            claude_credentials: PathBuf::new(),
            codex_auth: PathBuf::new(),
            codex_sessions: PathBuf::new(),
            amp_settings: PathBuf::new(),
            amp_threads: PathBuf::new(),
            grok_home: PathBuf::new(),
            grok_auth: PathBuf::new(),
        });
        let cfg = SetupConfig {
            asset_paths: Some(asset_paths),
            integration_paths: Some(ipaths),
            repo_root: Some(repo.path().to_path_buf()),
            home: dest.path().to_path_buf(),
            skip_reload: true,
            system_install: true, // força o branch system (sem depender de current_exe)
            omarchy: None,
            skip_waybar: false,
        };
        let ok = run_setup(&s, cfg, false, false).unwrap();
        assert!(ok);
        // não criou symlink em <home>/.local/bin/agent-bar
        assert!(!dest
            .path()
            .join(".local")
            .join("bin")
            .join("agent-bar")
            .exists());
    }

    #[test]
    fn waybar_present_checks_path() {
        let bin = tempdir().unwrap();
        let path_var = std::ffi::OsString::from(bin.path());
        assert!(!waybar_present(Some(&path_var)));
        std::fs::write(bin.path().join("waybar"), b"").unwrap();
        assert!(waybar_present(Some(&path_var)));
        assert!(!waybar_present(None));
    }

    #[test]
    #[serial_test::serial]
    fn setup_omarchy_only_installs_plugin_and_skips_waybar() {
        let dest = tempdir().unwrap();
        let plugins = tempdir().unwrap();
        let s = load(&Paths {
            cache_dir: dest.path().join("c"),
            config_dir: dest.path().join("cfg"),
            claude_credentials: PathBuf::new(),
            codex_auth: PathBuf::new(),
            codex_sessions: PathBuf::new(),
            amp_settings: PathBuf::new(),
            amp_threads: PathBuf::new(),
            grok_home: PathBuf::new(),
            grok_auth: PathBuf::new(),
        });
        let cfg = SetupConfig {
            asset_paths: None,
            integration_paths: None,
            repo_root: None,
            home: dest.path().to_path_buf(),
            skip_reload: true,
            system_install: true,
            omarchy: Some(OmarchySetupOptions {
                plugins_dir: plugins.path().to_path_buf(),
                run_cli: false, // NUNCA roda `omarchy` real em teste
            }),
            skip_waybar: true,
        };
        let ok = run_setup(&s, cfg, false, false).unwrap();
        assert!(ok);
        let plugin_dir = plugins.path().join(crate::app_identity::OMARCHY_PLUGIN_ID);
        assert!(plugin_dir.join("manifest.json").exists());
        assert!(plugin_dir.join("Widget.qml").exists());
        // fluxo waybar não rodou: nenhum config.jsonc/style.css criado
        assert!(!dest.path().join("config.jsonc").exists());
    }
}
