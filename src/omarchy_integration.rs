//! Integração com o omarchy-shell (Omarchy 4+): escreve o plugin bar-widget
//! `agent-bar.usage` como drop-in em `<plugins_dir>/agent-bar.usage/`.
//!
//! Os arquivos do plugin são EMBUTIDOS no binário (include_str!/include_bytes!)
//! de propósito: o QML fica version-locked com o schema de `--format json`
//! do mesmo binário. Contrato: docs/omarchy-shell.md.

use std::ffi::OsStr;
use std::path::{Path, PathBuf};
use std::process::Command;

use crate::app_identity::{OMARCHY_PLUGIN_ID, OMARCHY_SHELL_DIR, TERMINAL_HELPER_NAME, VERSION};

const MANIFEST_TEMPLATE: &str = include_str!("../assets/omarchy/manifest.json");
const WIDGET_QML: &str = include_str!("../assets/omarchy/Widget.qml");
const TERMINAL_HELPER: &str = include_str!("../scripts/agent-bar-open-terminal");
const ICON_CLAUDE: &[u8] = include_bytes!("../icons/claude-code-icon.png");
const ICON_CODEX: &[u8] = include_bytes!("../icons/codex-icon.png");
const ICON_AMP: &[u8] = include_bytes!("../icons/amp-icon.svg");
const ICON_GROK: &[u8] = include_bytes!("../icons/grok-icon.svg");

/// Placeholder do manifest substituído por `VERSION` na instalação.
pub const VERSION_PLACEHOLDER: &str = "__AGENT_BAR_VERSION__";

/// `${XDG_CONFIG_HOME:-<home>/.config}/omarchy/plugins`.
pub fn default_omarchy_plugins_dir(home: &Path) -> PathBuf {
    let config_root = std::env::var_os("XDG_CONFIG_HOME")
        .filter(|v| !v.is_empty())
        .map(PathBuf::from)
        .unwrap_or_else(|| home.join(".config"));
    config_root.join("omarchy").join("plugins")
}

/// `${XDG_CONFIG_HOME:-<home>/.config}/omarchy/shell.json` — arquivo do
/// omarchy-shell (schema não é nosso; usado só como leitura pelo `doctor`).
/// NUNCA escrito por este binário (ADR-0002).
pub fn default_omarchy_shell_json_path(home: &Path) -> PathBuf {
    let config_root = std::env::var_os("XDG_CONFIG_HOME")
        .filter(|v| !v.is_empty())
        .map(PathBuf::from)
        .unwrap_or_else(|| home.join(".config"));
    config_root.join("omarchy").join("shell.json")
}

/// Sinal de omarchy-shell: raiz QML instalada E CLI `omarchy` no PATH.
/// Ambos exigidos — só o dir pode ser resíduo de pacote; só a CLI pode
/// ser um Omarchy < 4 sem shell.
pub fn omarchy_shell_present(shell_dir: &Path, path_var: Option<&OsStr>) -> bool {
    shell_dir.is_dir() && cli_on_path(path_var)
}

pub fn detect_omarchy_shell() -> bool {
    omarchy_shell_present(
        Path::new(OMARCHY_SHELL_DIR),
        std::env::var_os("PATH").as_deref(),
    )
}

/// Scan único de `path_var` pela CLI `omarchy` — compartilhado pela detecção
/// do shell e pelo check do uninstall.
fn cli_on_path(path_var: Option<&OsStr>) -> bool {
    path_var.is_some_and(|p| std::env::split_paths(p).any(|dir| dir.join("omarchy").is_file()))
}

/// Só a CLI (usado pelo uninstall best-effort, que não exige o shell dir).
pub fn omarchy_cli_available() -> bool {
    cli_on_path(std::env::var_os("PATH").as_deref())
}

/// Manifest com a versão do binário injetada.
pub fn rendered_manifest() -> String {
    MANIFEST_TEMPLATE.replace(VERSION_PLACEHOLDER, VERSION)
}

pub struct InstalledOmarchyPlugin {
    pub plugin_dir: PathBuf,
}

/// Escreve o drop-in completo. Idempotente: sobrescreve arquivos existentes
/// (é assim que `setup` re-executado atualiza o plugin após update).
pub fn install_omarchy_plugin(plugins_dir: &Path) -> anyhow::Result<InstalledOmarchyPlugin> {
    let plugin_dir = plugins_dir.join(OMARCHY_PLUGIN_ID);
    let icons_dir = plugin_dir.join("icons");
    let scripts_dir = plugin_dir.join("scripts");
    std::fs::create_dir_all(&icons_dir)?;
    std::fs::create_dir_all(&scripts_dir)?;

    std::fs::write(plugin_dir.join("manifest.json"), rendered_manifest())?;
    std::fs::write(plugin_dir.join("Widget.qml"), WIDGET_QML)?;
    std::fs::write(icons_dir.join("claude-code-icon.png"), ICON_CLAUDE)?;
    std::fs::write(icons_dir.join("codex-icon.png"), ICON_CODEX)?;
    std::fs::write(icons_dir.join("amp-icon.svg"), ICON_AMP)?;
    std::fs::write(icons_dir.join("grok-icon.svg"), ICON_GROK)?;

    let helper = scripts_dir.join(TERMINAL_HELPER_NAME);
    std::fs::write(&helper, TERMINAL_HELPER)?;
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(&helper, std::fs::Permissions::from_mode(0o755))?;

    Ok(InstalledOmarchyPlugin { plugin_dir })
}

/// Remove o drop-in. `Ok(true)` se existia e foi removido.
pub fn remove_omarchy_plugin(plugins_dir: &Path) -> std::io::Result<bool> {
    let plugin_dir = plugins_dir.join(OMARCHY_PLUGIN_ID);
    if !plugin_dir.exists() {
        return Ok(false);
    }
    std::fs::remove_dir_all(&plugin_dir)?;
    Ok(true)
}

/// Roda um comando `omarchy ...` best-effort; retorna aviso em falha.
fn run_omarchy(args: &[&str]) -> Option<String> {
    match Command::new("omarchy").args(args).output() {
        Ok(out) if out.status.success() => None,
        Ok(out) => Some(format!(
            "`omarchy {}` falhou: {}",
            args.join(" "),
            String::from_utf8_lossy(&out.stderr).trim()
        )),
        Err(e) => Some(format!("`omarchy {}` não executou: {e}", args.join(" "))),
    }
}

/// Ativa o plugin no shell (rescan + enable + bar add). Best-effort:
/// retorna a lista de avisos — o setup imprime e segue (o usuário pode
/// rodar os comandos manualmente).
pub fn run_omarchy_enable_commands() -> Vec<String> {
    [
        // Sem `--yes`: enable/add com argumento já são não-interativos e o
        // CLI do Omarchy 4.0.0.alpha rejeita a flag ("unknown option" —
        // verificado ao vivo em 2026-07-21; `--yes` só existe em add/update
        // de repo git).
        vec!["plugin", "rescan"],
        vec!["plugin", "enable", OMARCHY_PLUGIN_ID],
        vec!["bar", "plugin", "add", OMARCHY_PLUGIN_ID],
    ]
    .iter()
    .filter_map(|args| run_omarchy(args))
    .collect()
}

/// Desativa/remove no shell (bar remove + plugin remove). Best-effort.
pub fn run_omarchy_remove_commands() -> Vec<String> {
    [
        vec!["bar", "plugin", "remove", OMARCHY_PLUGIN_ID],
        vec!["plugin", "remove", OMARCHY_PLUGIN_ID],
    ]
    .iter()
    .filter_map(|args| run_omarchy(args))
    .collect()
}

/// `true` se `OMARCHY_PLUGIN_ID` aparecer como valor de string em qualquer
/// lugar da árvore JSON de `shell_json_path` — tolerante ao shape exato do
/// `shell.json` (schema do omarchy-shell, não nosso). `false` se o arquivo
/// não existir ou não parsear (silencioso — é só um sinal pro `doctor`).
pub fn shell_json_has_plugin_entry(shell_json_path: &Path) -> bool {
    let Ok(content) = std::fs::read_to_string(shell_json_path) else {
        return false;
    };
    let Ok(value) = serde_json::from_str::<serde_json::Value>(&content) else {
        return false;
    };
    json_contains_string(&value, OMARCHY_PLUGIN_ID)
}

fn json_contains_string(value: &serde_json::Value, needle: &str) -> bool {
    match value {
        serde_json::Value::String(s) => s == needle,
        serde_json::Value::Array(items) => items.iter().any(|v| json_contains_string(v, needle)),
        serde_json::Value::Object(map) => map.values().any(|v| json_contains_string(v, needle)),
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::PermissionsExt;
    use tempfile::tempdir;

    #[test]
    #[serial_test::serial]
    fn default_plugins_dir_respects_xdg_config_home() {
        let prev = std::env::var_os("XDG_CONFIG_HOME");
        std::env::set_var("XDG_CONFIG_HOME", "/tmp/xdg-test");
        let dir = default_omarchy_plugins_dir(std::path::Path::new("/home/u"));
        assert_eq!(
            dir,
            std::path::PathBuf::from("/tmp/xdg-test/omarchy/plugins")
        );
        std::env::remove_var("XDG_CONFIG_HOME");
        let dir = default_omarchy_plugins_dir(std::path::Path::new("/home/u"));
        assert_eq!(
            dir,
            std::path::PathBuf::from("/home/u/.config/omarchy/plugins")
        );
        match prev {
            Some(v) => std::env::set_var("XDG_CONFIG_HOME", v),
            None => std::env::remove_var("XDG_CONFIG_HOME"),
        }
    }

    #[test]
    fn shell_json_has_plugin_entry_finds_nested_id() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("shell.json");
        std::fs::write(&path, r#"{"bar":{"plugins":[{"id":"agent-bar.usage"}]}}"#).unwrap();
        assert!(shell_json_has_plugin_entry(&path));
    }

    #[test]
    fn shell_json_has_plugin_entry_false_when_absent_or_missing() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("shell.json");
        assert!(!shell_json_has_plugin_entry(&path)); // arquivo nao existe

        std::fs::write(&path, r#"{"bar":{"plugins":[]}}"#).unwrap();
        assert!(!shell_json_has_plugin_entry(&path));
    }

    #[test]
    #[serial_test::serial]
    fn default_shell_json_path_respects_xdg_config_home() {
        let prev = std::env::var_os("XDG_CONFIG_HOME");
        std::env::set_var("XDG_CONFIG_HOME", "/tmp/xdg-test-shell-json");
        let path = default_omarchy_shell_json_path(std::path::Path::new("/home/u"));
        assert_eq!(
            path,
            std::path::PathBuf::from("/tmp/xdg-test-shell-json/omarchy/shell.json")
        );
        std::env::remove_var("XDG_CONFIG_HOME");
        let path = default_omarchy_shell_json_path(std::path::Path::new("/home/u"));
        assert_eq!(
            path,
            std::path::PathBuf::from("/home/u/.config/omarchy/shell.json")
        );
        match prev {
            Some(v) => std::env::set_var("XDG_CONFIG_HOME", v),
            None => std::env::remove_var("XDG_CONFIG_HOME"),
        }
    }

    #[test]
    fn shell_present_requires_dir_and_cli() {
        let bin = tempdir().unwrap();
        let shell = tempdir().unwrap();
        // sem CLI no PATH → false
        let empty = std::ffi::OsString::from(bin.path());
        assert!(!omarchy_shell_present(shell.path(), Some(&empty)));
        // com CLI fake no PATH → true
        std::fs::write(bin.path().join("omarchy"), "#!/bin/sh\n").unwrap();
        assert!(omarchy_shell_present(shell.path(), Some(&empty)));
        // dir inexistente → false mesmo com CLI
        assert!(!omarchy_shell_present(
            &shell.path().join("nope"),
            Some(&empty)
        ));
    }

    #[test]
    fn install_writes_plugin_files_with_version() {
        let dest = tempdir().unwrap();
        let installed = install_omarchy_plugin(dest.path()).unwrap();
        let dir = installed.plugin_dir;
        assert_eq!(
            dir,
            dest.path().join(crate::app_identity::OMARCHY_PLUGIN_ID)
        );
        let manifest = std::fs::read_to_string(dir.join("manifest.json")).unwrap();
        assert!(manifest.contains(crate::app_identity::VERSION));
        assert!(!manifest.contains(VERSION_PLACEHOLDER));
        assert!(manifest.contains("\"id\": \"agent-bar.usage\""));
        assert!(dir.join("Widget.qml").exists());
        assert!(dir.join("icons").join("claude-code-icon.png").exists());
        assert!(dir.join("icons").join("codex-icon.png").exists());
        assert!(dir.join("icons").join("amp-icon.svg").exists());
        assert!(dir.join("icons").join("grok-icon.svg").exists());
        let helper = dir
            .join("scripts")
            .join(crate::app_identity::TERMINAL_HELPER_NAME);
        assert!(helper.exists());
        let mode = std::fs::metadata(&helper).unwrap().permissions().mode();
        assert_eq!(mode & 0o111, 0o111, "helper deve ser executável");
    }

    #[test]
    fn install_is_idempotent() {
        let dest = tempdir().unwrap();
        install_omarchy_plugin(dest.path()).unwrap();
        install_omarchy_plugin(dest.path()).unwrap(); // re-run não falha
    }

    #[test]
    fn remove_reports_presence() {
        let dest = tempdir().unwrap();
        assert!(!remove_omarchy_plugin(dest.path()).unwrap());
        install_omarchy_plugin(dest.path()).unwrap();
        assert!(remove_omarchy_plugin(dest.path()).unwrap());
        assert!(!dest
            .path()
            .join(crate::app_identity::OMARCHY_PLUGIN_ID)
            .exists());
    }

    #[test]
    fn manifest_snapshot() {
        let rendered = rendered_manifest().replace(crate::app_identity::VERSION, "0.0.0-test");
        insta::assert_snapshot!("omarchy_manifest", rendered);
    }
}
