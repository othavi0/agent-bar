//! Port de `src/uninstall.ts` — remoção interativa leve (terminal-lite, sem @clack).

use std::path::Path;

use crate::app_identity::APP_NAME;
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
pub fn run_uninstall(
    settings_dir: &Path,
    cache_dir: &Path,
    home: &Path,
    force: bool,
    title: &str,
    integration_paths: &WaybarIntegrationPaths,
) -> anyhow::Result<()> {
    let asset_paths = get_default_waybar_asset_paths();
    let app_symlink = home.join(".local").join("bin").join(APP_NAME);

    // 1. Nota listando o que será removido
    term_prompt::note(&format!(
        "{title} — paths que serão removidos:\n  • {}\n  • {}\n  • {}\n  • {}\n  • {}\n  • {}\n  • {}\n  • {}",
        integration_paths.waybar_config_path.display(),
        integration_paths.waybar_style_path.display(),
        integration_paths.modules_include_path.display(),
        integration_paths.style_include_path.display(),
        asset_paths.waybar_dir.display(),
        asset_paths.terminal_script.display(),
        settings_dir.display(),
        cache_dir.display(),
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
