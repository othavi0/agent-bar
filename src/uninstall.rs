//! Port de `src/uninstall.ts` — remoção interativa leve (terminal-lite, sem @clack).

use std::path::Path;

use crate::app_identity::{APP_NAME, OMARCHY_PLUGIN_ID};
use crate::omarchy_integration::remove_omarchy_plugin;
use crate::waybar_contract::get_default_waybar_asset_paths;
use crate::waybar_integration::{remove_waybar_integration, WaybarIntegrationPaths};
use crate::{setup, term_prompt};

/// Remove `path` se existir. Registra em `removed` ou `failed`.
fn remove_path_if_exists(path: &Path, removed: &mut Vec<String>, failed: &mut Vec<String>) {
    if !path.exists() {
        return;
    }

    let display = path.to_string_lossy().into_owned();
    let result = if path.is_dir() {
        std::fs::remove_dir_all(path)
    } else {
        std::fs::remove_file(path)
    };

    match result {
        Ok(()) => removed.push(display),
        Err(_) => failed.push(display),
    }
}

/// Port de `runUninstall` (src/uninstall.ts:47-129), versão terminal-lite.
///
/// - `force=true` → pula confirmação (usado por `remove`).
/// - `title` → label do comando ("agent-bar uninstall" ou "agent-bar remove").
#[allow(clippy::too_many_arguments)]
pub fn run_uninstall(
    settings_dir: &Path,
    cache_dir: &Path,
    home: &Path,
    force: bool,
    title: &str,
    integration_paths: &WaybarIntegrationPaths,
    omarchy_plugins_dir: &Path,
    cli_available_fn: &dyn Fn() -> bool,
    remove_commands_fn: &dyn Fn() -> Vec<String>,
) -> anyhow::Result<()> {
    let asset_paths = get_default_waybar_asset_paths();
    let app_symlink = home.join(".local").join("bin").join(APP_NAME);

    // 1. Nota listando o que será removido
    term_prompt::note(&format!(
        "{title} — paths que serão removidos:\n  • {}\n  • {}\n  • {}\n  • {}\n  • {}\n  • {}\n  • {}\n  • {}\n  • {}\n  • {}",
        integration_paths.waybar_config_path.display(),
        integration_paths.waybar_style_path.display(),
        integration_paths.modules_include_path.display(),
        integration_paths.style_include_path.display(),
        asset_paths.waybar_dir.display(),
        asset_paths.terminal_script.display(),
        settings_dir.display(),
        cache_dir.display(),
        app_symlink.display(),
        omarchy_plugins_dir.join(OMARCHY_PLUGIN_ID).display(),
    ));

    // 2. Confirmação (pula se force=true)
    if !force {
        let proceed = term_prompt::confirm("Continue with uninstall?", false);
        if !proceed {
            term_prompt::status("Cancelado", "Uninstall não aplicado");
            return Ok(());
        }
    }

    // 3. Remove integração Waybar (config + style patches + include files)
    let integration_result = remove_waybar_integration(integration_paths)?;

    // 4. Remove paths individuais
    let mut removed: Vec<String> = Vec::new();
    let mut failed: Vec<String> = Vec::new();

    remove_path_if_exists(&asset_paths.waybar_dir, &mut removed, &mut failed);
    remove_path_if_exists(&asset_paths.terminal_script, &mut removed, &mut failed);
    remove_path_if_exists(settings_dir, &mut removed, &mut failed);
    remove_path_if_exists(cache_dir, &mut removed, &mut failed);
    remove_path_if_exists(&app_symlink, &mut removed, &mut failed);

    // Omarchy-shell: só apaga o diretório do drop-in se os comandos
    // `omarchy ... remove` confirmarem (CLI disponível E sem warnings) —
    // senão fica referência pendurada em `shell.json` apontando pra um
    // dir inexistente, pior do que manter o dir órfão (spec F).
    let plugin_dir = omarchy_plugins_dir.join(OMARCHY_PLUGIN_ID);
    if plugin_dir.exists() {
        let mut remove_ok = cli_available_fn();
        if remove_ok {
            for warning in remove_commands_fn() {
                term_prompt::status("Aviso", &warning);
                remove_ok = false;
            }
        }
        if remove_ok {
            match remove_omarchy_plugin(omarchy_plugins_dir) {
                Ok(true) => removed.push(plugin_dir.to_string_lossy().into_owned()),
                Ok(false) => {}
                Err(_) => failed.push(plugin_dir.to_string_lossy().into_owned()),
            }
        } else {
            term_prompt::status(
                "Aviso",
                &format!(
                    "omarchy remove não confirmado — diretório do plugin mantido em {} (pode restar referência em shell.json)",
                    plugin_dir.display()
                ),
            );
        }
    }

    // 5. Reload Waybar se config/style foram alterados
    if integration_result.config_changed || integration_result.style_changed {
        setup::reload_waybar();
    }

    // 6. Status final
    if !removed.is_empty() {
        term_prompt::status("OK", &format!("{} paths removidos", removed.len()));
    }
    if !failed.is_empty() {
        term_prompt::status("Aviso", &format!("Falha ao remover {} paths", failed.len()));
    }
    term_prompt::status("OK", &format!("{title} completo"));

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    fn integration_paths(dir: &Path) -> WaybarIntegrationPaths {
        WaybarIntegrationPaths {
            waybar_config_path: dir.join("config.jsonc"),
            waybar_style_path: dir.join("style.css"),
            modules_include_path: dir.join("agent-bar").join("modules.jsonc"),
            style_include_path: dir.join("agent-bar").join("style.css"),
        }
    }

    #[test]
    #[serial_test::serial]
    fn uninstall_removes_omarchy_plugin_when_cli_confirms() {
        let home = tempdir().unwrap();
        let prev_home = std::env::var_os("HOME");
        std::env::set_var("HOME", home.path());

        let plugins = tempdir().unwrap();
        crate::omarchy_integration::install_omarchy_plugin(plugins.path()).unwrap();
        let plugin_dir = plugins
            .path()
            .join(crate::app_identity::OMARCHY_PLUGIN_ID);
        assert!(plugin_dir.exists());

        run_uninstall(
            &home.path().join("cfg"),
            &home.path().join("cache"),
            home.path(),
            true, // force: pula confirmação interativa
            "agent-bar uninstall",
            &integration_paths(&home.path().join("waybar-unused")),
            plugins.path(),
            &|| true,
            &Vec::new,
        )
        .unwrap();

        assert!(!plugin_dir.exists());

        match prev_home {
            Some(v) => std::env::set_var("HOME", v),
            None => std::env::remove_var("HOME"),
        }
    }

    #[test]
    #[serial_test::serial]
    fn uninstall_keeps_omarchy_plugin_when_cli_unavailable() {
        let home = tempdir().unwrap();
        let prev_home = std::env::var_os("HOME");
        std::env::set_var("HOME", home.path());

        let plugins = tempdir().unwrap();
        crate::omarchy_integration::install_omarchy_plugin(plugins.path()).unwrap();
        let plugin_dir = plugins
            .path()
            .join(crate::app_identity::OMARCHY_PLUGIN_ID);

        run_uninstall(
            &home.path().join("cfg"),
            &home.path().join("cache"),
            home.path(),
            true,
            "agent-bar uninstall",
            &integration_paths(&home.path().join("waybar-unused")),
            plugins.path(),
            &|| false, // CLI omarchy indisponível
            &Vec::new,
        )
        .unwrap();

        assert!(
            plugin_dir.exists(),
            "diretório do plugin deveria ser mantido"
        );

        match prev_home {
            Some(v) => std::env::set_var("HOME", v),
            None => std::env::remove_var("HOME"),
        }
    }

    #[test]
    #[serial_test::serial]
    fn uninstall_keeps_omarchy_plugin_when_remove_commands_warn() {
        let home = tempdir().unwrap();
        let prev_home = std::env::var_os("HOME");
        std::env::set_var("HOME", home.path());

        let plugins = tempdir().unwrap();
        crate::omarchy_integration::install_omarchy_plugin(plugins.path()).unwrap();
        let plugin_dir = plugins
            .path()
            .join(crate::app_identity::OMARCHY_PLUGIN_ID);

        run_uninstall(
            &home.path().join("cfg"),
            &home.path().join("cache"),
            home.path(),
            true,
            "agent-bar uninstall",
            &integration_paths(&home.path().join("waybar-unused")),
            plugins.path(),
            &|| true,
            &|| vec!["`omarchy bar plugin remove` falhou: boom".to_string()],
        )
        .unwrap();

        assert!(
            plugin_dir.exists(),
            "diretório do plugin deveria ser mantido após falha"
        );

        match prev_home {
            Some(v) => std::env::set_var("HOME", v),
            None => std::env::remove_var("HOME"),
        }
    }
}
