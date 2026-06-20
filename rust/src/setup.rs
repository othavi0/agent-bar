//! Port de `src/setup.ts` — setup interativo leve (terminal-lite, sem @clack).

use std::io::IsTerminal;
use std::os::unix::fs::symlink;
use std::path::{Path, PathBuf};
use std::process::Stdio;

use crate::app_identity::APP_NAME;
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

/// Cria symlink `~/.local/bin/agent-bar` → `<repo_root>/scripts/agent-bar`.
/// Só chamado em instalações dev (não-system).
pub fn create_symlink(home: &Path) -> std::io::Result<PathBuf> {
    let local_bin = home.join(".local").join("bin");
    std::fs::create_dir_all(&local_bin)?;

    let link = local_bin.join(APP_NAME);

    // repo_root = pai do CARGO_MANIFEST_DIR (i.e., raiz do repo, não `rust/`)
    let repo_root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .map(|p| p.to_path_buf())
        .unwrap_or_else(|| PathBuf::from("."));

    let target = repo_root.join("scripts").join(APP_NAME);

    // Remove symlink anterior se existir (ignora erro de não-existência)
    let _ = std::fs::remove_file(&link);

    symlink(&target, &link)?;
    Ok(link)
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
    term_prompt::note(&format!(
        "{APP_NAME} setup — instalando icons, helper e integração Waybar"
    ));

    // 3. Confirmação interativa
    if confirm {
        let proceed = term_prompt::confirm(&format!("Apply {APP_NAME} setup now?"), true);
        if !proceed {
            term_prompt::status("Cancelado", "Setup não aplicado");
            return Ok(false);
        }
    }

    // 4. Instala assets (icons + terminal helper)
    let asset = cfg
        .asset_paths
        .unwrap_or_else(get_default_waybar_asset_paths);
    let installed = install_waybar_assets(
        &asset.waybar_dir,
        &asset.scripts_dir,
        cfg.repo_root.as_deref(),
    )?;

    // 5. Symlink (somente em instalação dev)
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

    // 8. Status de sucesso + scan de leftovers (best-effort, NUNCA falha o setup)
    term_prompt::status("OK", &format!("{APP_NAME} setup completo"));
    term_prompt::status("Icons", &installed.icons_dir.to_string_lossy());
    term_prompt::status("Helper", &installed.terminal_script.to_string_lossy());

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
        });
        let cfg = SetupConfig {
            asset_paths: Some(asset_paths),
            integration_paths: Some(ipaths),
            repo_root: Some(repo.path().to_path_buf()),
            home: dest.path().to_path_buf(),
            skip_reload: true,
            system_install: true, // força o branch system (sem depender de current_exe)
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
}
