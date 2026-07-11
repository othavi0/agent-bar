//! Port de `src/waybar-contract.ts` — export de módulos/CSS Waybar + resolução de assets.
//! Funções puras; sem I/O direto (exceto `resolve_asset_source_root` que lê o filesystem).

use std::path::{Path, PathBuf};

use indexmap::IndexMap;
use serde::Serialize;

use crate::app_identity::{
    APP_HIDDEN_CLASS, APP_NAME, TERMINAL_HELPER_NAME, WAYBAR_MODULE_PREFIX, WAYBAR_NAMESPACE,
    WAYBAR_SELECTOR_PREFIX,
};
use crate::runtime::is_system_install;
use crate::settings::SeparatorStyle;
use crate::theme::ColorToken;

/// Paleta One Dark overlay — usada como `background-color` nos separadores pill/glass.
/// Equivale a `ONE_DARK.overlay` no TS (`#242a33`).
const SURFACE: &str = "#242a33";

/// Prefixo de assets de sistema (instalação AUR/pacote).
const SYSTEM_ASSET_DIR_PREFIX: &str = "/usr/share/";

/// Providers Waybar na ordem canônica — espelha `WAYBAR_PROVIDERS` do TS.
pub const WAYBAR_PROVIDERS: [&str; 3] = ["claude", "codex", "amp"];

// ---------------------------------------------------------------------------
// WaybarModuleConfig
// ---------------------------------------------------------------------------

/// Configuração de módulo Waybar. Espelha `WaybarModuleConfig` do TS.
#[derive(Debug, Clone, Serialize)]
pub struct WaybarModuleConfig {
    pub exec: String,
    #[serde(rename = "return-type")]
    pub return_type: String,
    pub interval: u32,
    #[serde(rename = "exec-on-event")]
    pub exec_on_event: bool,
    pub tooltip: bool,
    #[serde(rename = "on-click")]
    pub on_click: String,
    #[serde(rename = "on-click-right")]
    pub on_click_right: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub signal: Option<u8>,
}

/// Constrói a definição de módulo para um provider. Espelha `moduleDefinition` do TS.
pub fn module_definition(
    provider: &str,
    app_bin: &str,
    terminal_script: &str,
    signal: Option<u8>,
    interval: u32,
) -> WaybarModuleConfig {
    WaybarModuleConfig {
        exec: format!("{app_bin} --provider {provider}"),
        return_type: "json".to_string(),
        interval,
        exec_on_event: true,
        tooltip: true,
        on_click: format!("{terminal_script} {app_bin} menu"),
        on_click_right: format!("{terminal_script} {app_bin} action-right {provider}"),
        signal,
    }
}

// ---------------------------------------------------------------------------
// WaybarModulesExport
// ---------------------------------------------------------------------------

/// Export de módulos Waybar. Espelha `WaybarModulesExport` do TS.
#[derive(Debug, Serialize)]
pub struct WaybarModulesExport {
    pub providers: Vec<String>,
    pub modules: IndexMap<String, WaybarModuleConfig>,
}

/// Constrói o export de módulos para os providers solicitados.
/// Espelha `exportWaybarModules` do TS.
pub fn export_waybar_modules(
    app_bin: &str,
    terminal_script: &str,
    signal: Option<u8>,
    providers: &[String],
    interval: u32,
) -> WaybarModulesExport {
    let mut modules = IndexMap::new();
    for provider in providers {
        modules.insert(
            format!("{WAYBAR_MODULE_PREFIX}{provider}"),
            module_definition(provider, app_bin, terminal_script, signal, interval),
        );
    }
    WaybarModulesExport {
        providers: providers.to_vec(),
        modules,
    }
}

// ---------------------------------------------------------------------------
// getAllProviderIds
// ---------------------------------------------------------------------------

/// Todos os provider ids conhecidos — built-in + registrados sem duplicatas.
/// Espelha `getAllProviderIds` do TS.
pub fn get_all_provider_ids() -> Vec<String> {
    let mut ids: Vec<String> = WAYBAR_PROVIDERS.iter().map(|s| s.to_string()).collect();
    for id in crate::providers::registered_provider_ids() {
        let id_str = id.to_string();
        if !ids.contains(&id_str) {
            ids.push(id_str);
        }
    }
    ids
}

// ---------------------------------------------------------------------------
// CSS export
// ---------------------------------------------------------------------------

/// CSS do bloco de separadores. Espelha `separatorCss` do TS (linhas 144-216).
/// Cada estilo termina com uma linha em branco (`''` final no array TS → `\n` no join).
fn separator_css(providers: &[String], style: SeparatorStyle) -> String {
    if providers.is_empty() {
        return String::new();
    }

    let selector_block = providers
        .iter()
        .map(|p| format!("{WAYBAR_SELECTOR_PREFIX}{p}"))
        .collect::<Vec<_>>()
        .join(",\n");

    match style {
        SeparatorStyle::Pill => [
            format!("/* {APP_NAME} separators: pill */"),
            format!("{selector_block} {{"),
            format!("  background-color: {SURFACE};"),
            "  border-radius: 4px;".to_string(),
            "}".to_string(),
            String::new(),
        ]
        .join("\n"),

        SeparatorStyle::Gap => [
            format!("/* {APP_NAME} separators: gap */"),
            format!("{selector_block} {{"),
            "  border-color: transparent;".to_string(),
            "}".to_string(),
            String::new(),
        ]
        .join("\n"),

        SeparatorStyle::Bare => [
            format!("/* {APP_NAME} separators: bare */"),
            format!("{selector_block} {{"),
            "  border-color: transparent;".to_string(),
            "  background-color: transparent;".to_string(),
            "}".to_string(),
            format!("{selector_block}:hover {{"),
            "  background-color: transparent;".to_string(),
            "  border-color: transparent;".to_string(),
            "}".to_string(),
            String::new(),
        ]
        .join("\n"),

        SeparatorStyle::Glass => [
            format!("/* {APP_NAME} separators: glass */"),
            format!("{selector_block} {{"),
            "  background-color: rgba(192, 201, 212, 0.04);".to_string(),
            "  border-color: transparent;".to_string(),
            "  border-radius: 4px;".to_string(),
            "}".to_string(),
            String::new(),
        ]
        .join("\n"),

        SeparatorStyle::Shadow => [
            format!("/* {APP_NAME} separators: shadow */"),
            format!("{selector_block} {{"),
            "  border-color: transparent;".to_string(),
            "  border-radius: 4px;".to_string(),
            "  box-shadow: 0 1px 3px rgba(0, 0, 0, 0.3);".to_string(),
            "}".to_string(),
            String::new(),
        ]
        .join("\n"),

        SeparatorStyle::None => [
            format!("/* {APP_NAME} separators: none */"),
            format!("{selector_block} {{"),
            "  border-color: transparent;".to_string(),
            "  margin: 0;".to_string(),
            "}".to_string(),
            String::new(),
        ]
        .join("\n"),
    }
}

/// Gera o CSS completo do Waybar. Retorna a string CSS diretamente.
/// O caller (T7) embrulha em `{"css": ...}`. Espelha `exportWaybarCss` do TS (linhas 273-325).
///
/// # Nota sobre `file://`
/// `pathToFileURL` do Node percent-encoda o path; para paths ASCII simples sem espaços
/// o resultado é `file://` + path absoluto, o que é o que fazemos aqui. Simplificação
/// fiel ao uso real (paths de icons no Waybar não têm espaços).
pub fn export_waybar_css(
    icons_dir: &str,
    provider_order: &[String],
    separators: SeparatorStyle,
) -> String {
    let icon_ref = |name: &str| -> String {
        let p = format!("{icons_dir}/{name}");
        if p.starts_with('/') {
            format!("file://{p}")
        } else {
            p
        }
    };

    let effective_order: Vec<String> = if provider_order.is_empty() {
        WAYBAR_PROVIDERS.iter().map(|s| s.to_string()).collect()
    } else {
        provider_order.to_vec()
    };

    let all_provider_selectors = WAYBAR_PROVIDERS
        .iter()
        .map(|p| format!("{WAYBAR_SELECTOR_PREFIX}{p}"))
        .collect::<Vec<_>>()
        .join(",\n");

    let state_selectors = |state: &str| -> String {
        WAYBAR_PROVIDERS
            .iter()
            .map(|p| format!("{WAYBAR_SELECTOR_PREFIX}{p}.{state}"))
            .collect::<Vec<_>>()
            .join(", ")
    };

    let sep_css = separator_css(&effective_order, separators);

    [
        format!("/* {APP_NAME} waybar stylesheet */"),
        format!("{all_provider_selectors} {{"),
        "  padding-left: 26px;".to_string(),
        "  padding-right: 10px;".to_string(),
        "  background-size: 14px 14px;".to_string(),
        "  background-repeat: no-repeat;".to_string(),
        "  background-position: 6px center;".to_string(),
        "  border-left: 1px solid #434d5d;".to_string(),
        format!("  color: {};", ColorToken::Text.hex()),
        "  transition: color 120ms ease, background-color 120ms ease;".to_string(),
        "}".to_string(),
        String::new(),
        format!("{all_provider_selectors}:hover {{"),
        "  background-color: rgba(192, 201, 212, 0.04);".to_string(),
        "  border-color: #3c4656;".to_string(),
        format!("  color: {};", ColorToken::TextBright.hex()),
        "}".to_string(),
        String::new(),
        format!(
            "{WAYBAR_SELECTOR_PREFIX}claude {{ background-image: url(\"{}\"); }}",
            icon_ref("claude-code-icon.png")
        ),
        format!(
            "{WAYBAR_SELECTOR_PREFIX}codex {{ background-image: url(\"{}\"); }}",
            icon_ref("codex-icon.png")
        ),
        format!(
            "{WAYBAR_SELECTOR_PREFIX}amp {{ background-image: url(\"{}\"); }}",
            icon_ref("amp-icon.svg")
        ),
        String::new(),
        format!(
            "{} {{ color: {}; }}",
            state_selectors("ok"),
            ColorToken::Green.hex()
        ),
        format!(
            "{} {{ color: {}; }}",
            state_selectors("low"),
            ColorToken::Yellow.hex()
        ),
        format!(
            "{} {{ color: {}; }}",
            state_selectors("warn"),
            ColorToken::Orange.hex()
        ),
        format!(
            "{} {{ color: {}; }}",
            state_selectors("critical"),
            ColorToken::Red.hex()
        ),
        format!(
            "{} {{ color: {}; }}",
            state_selectors("disconnected"),
            ColorToken::Red.hex()
        ),
        format!("{} {{", state_selectors(APP_HIDDEN_CLASS)),
        "  min-width: 0;".to_string(),
        "  padding: 0;".to_string(),
        "  margin: 0;".to_string(),
        "  border: 0;".to_string(),
        "  background-image: none;".to_string(),
        "}".to_string(),
        String::new(),
        sep_css,
    ]
    .join("\n")
}

// ---------------------------------------------------------------------------
// install_waybar_assets
// ---------------------------------------------------------------------------

/// Assets copiados para o destino do Waybar. Espelha o retorno de `installWaybarAssets` do TS.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InstalledAssets {
    pub icons_dir: PathBuf,
    pub terminal_script: PathBuf,
}

fn copy_dir(src: &Path, dest: &Path) -> std::io::Result<()> {
    std::fs::create_dir_all(dest)?;
    for entry in std::fs::read_dir(src)? {
        let entry = entry?;
        let src_path = entry.path();
        let dest_path = dest.join(entry.file_name());
        if entry.file_type()?.is_dir() {
            copy_dir(&src_path, &dest_path)?;
        } else {
            std::fs::copy(&src_path, &dest_path)?;
        }
    }
    Ok(())
}

/// Copia `icons/` e o terminal helper para o destino do Waybar.
/// `repo_root=None` resolve via `resolve_asset_source_root`.
/// Espelha `installWaybarAssets` do TS (linhas 327-358).
pub fn install_waybar_assets(
    waybar_dir: &Path,
    scripts_dir: &Path,
    repo_root: Option<&Path>,
) -> anyhow::Result<InstalledAssets> {
    let repo_root: PathBuf = match repo_root {
        Some(r) => r.to_path_buf(),
        None => resolve_asset_source_root()?,
    };
    let icons_source = repo_root.join("icons");
    let icons_dest = waybar_dir.join("icons");
    let script_source = repo_root
        .join("scripts")
        .join(crate::app_identity::TERMINAL_HELPER_NAME);
    let script_dest = scripts_dir.join(crate::app_identity::TERMINAL_HELPER_NAME);

    if !icons_source.exists() {
        anyhow::bail!("Icons folder not found: {}", icons_source.display());
    }
    if !script_source.exists() {
        anyhow::bail!("Terminal helper not found: {}", script_source.display());
    }

    let _ = std::fs::remove_dir_all(&icons_dest); // rmSync recursive+force (ignora ausência)
    std::fs::create_dir_all(waybar_dir)?;
    copy_dir(&icons_source, &icons_dest)?;

    std::fs::create_dir_all(scripts_dir)?;
    std::fs::copy(&script_source, &script_dest)?;
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(&script_dest, std::fs::Permissions::from_mode(0o755))?;

    Ok(InstalledAssets {
        icons_dir: icons_dest,
        terminal_script: script_dest,
    })
}

// ---------------------------------------------------------------------------
// WaybarAssetPaths
// ---------------------------------------------------------------------------

/// Paths de assets Waybar. Espelha o retorno de `getDefaultWaybarAssetPaths` do TS.
pub struct WaybarAssetPaths {
    pub waybar_dir: PathBuf,
    pub scripts_dir: PathBuf,
    pub icons_dir: PathBuf,
    pub terminal_script: PathBuf,
    /// Literal (pode conter `$HOME`) — não é um PathBuf real.
    pub app_bin: String,
}

/// Paths defaults para uma instalação típica. Espelha `getDefaultWaybarAssetPaths` do TS.
pub fn get_default_waybar_asset_paths() -> WaybarAssetPaths {
    let home = std::env::var("HOME").unwrap_or_else(|_| "/root".to_string());
    let waybar_root = PathBuf::from(&home).join(".config").join("waybar");
    let waybar_dir = waybar_root.join(WAYBAR_NAMESPACE);
    let scripts_dir = waybar_root.join("scripts");
    let icons_dir = waybar_dir.join("icons");
    let terminal_script = scripts_dir.join(TERMINAL_HELPER_NAME);
    let app_bin = if is_system_install() {
        APP_NAME.to_string()
    } else {
        format!("$HOME/.local/bin/{APP_NAME}")
    };

    WaybarAssetPaths {
        waybar_dir,
        scripts_dir,
        icons_dir,
        terminal_script,
        app_bin,
    }
}

// ---------------------------------------------------------------------------
// resolve_asset_source_root
// ---------------------------------------------------------------------------

/// Resolve o diretório de dados de uma instalação standalone (curl|bash).
/// A decisão `XDG_DATA_HOME` vs `<home>/.local/share` (a parte que causava
/// split-brain com `update::default_data_dir` — hotfix 7.0.1) é delegada pra
/// `config::agent_bar_data_dir`, canônica pros dois. O short-circuit de
/// `AGENT_BAR_DATA` fica duplicado aqui de propósito: não precisa de `HOME`
/// pra resolver, então checamos antes de exigi-lo (`None` só quando nem
/// `AGENT_BAR_DATA` nem `HOME` estão disponíveis).
fn standalone_data_asset_dir() -> Option<PathBuf> {
    if let Some(v) = std::env::var_os("AGENT_BAR_DATA").filter(|v| !v.is_empty()) {
        return Some(PathBuf::from(v));
    }
    let home = std::env::var_os("HOME").filter(|v| !v.is_empty())?;
    Some(crate::config::agent_bar_data_dir(&PathBuf::from(home)))
}

/// Resolve o diretório raiz que contém `icons/` e `scripts/` de origem.
/// Espelha `resolveAssetSourceRoot` do TS (linhas 74-94).
///
/// Prioridade:
/// 1. `AGENT_BAR_ASSET_DIR` env — deve ser absoluto e conter `icons/`.
/// 2. Instalação de sistema: `/usr/share/agent-bar`.
/// 3. Instalação standalone: `AGENT_BAR_DATA` env, senão
///    `${XDG_DATA_HOME:-~/.local/share}/agent-bar` (mesma resolução do
///    `install.sh` — hotfix 7.0.1: sem isso, `agent-bar setup` avulso após
///    um install standalone caía direto no fallback dev/`CARGO_MANIFEST_DIR`
///    fantasma e falhava).
/// 4. Dev/checkout: o `CARGO_MANIFEST_DIR` (raiz do repo) — só aceito se
///    `has_icons` também passar aqui.
pub fn resolve_asset_source_root() -> anyhow::Result<PathBuf> {
    let has_icons = |d: &Path| d.join("icons").exists();

    if let Some(env_val) = std::env::var_os("AGENT_BAR_ASSET_DIR") {
        let env_dir = PathBuf::from(&env_val);
        let env_str = env_val.to_string_lossy();
        if !env_dir.is_absolute() || !has_icons(&env_dir) {
            anyhow::bail!(
                "AGENT_BAR_ASSET_DIR must be an absolute path containing icons/ (got: {env_str})."
            );
        }
        return Ok(env_dir);
    }

    if is_system_install() {
        let system_dir = PathBuf::from(format!("{SYSTEM_ASSET_DIR_PREFIX}{APP_NAME}"));
        if has_icons(&system_dir) {
            return Ok(system_dir);
        }
        anyhow::bail!(
            "Asset directory not found at {}. Reinstall the package, or set AGENT_BAR_ASSET_DIR.",
            system_dir.display()
        );
    }

    if let Some(data_dir) = standalone_data_asset_dir() {
        if has_icons(&data_dir) {
            return Ok(data_dir);
        }
    }

    // Pós-cutover: o crate É a raiz do repo; CARGO_MANIFEST_DIR já aponta pra raiz.
    let repo_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));

    if has_icons(&repo_root) {
        return Ok(repo_root);
    }

    anyhow::bail!(
        "Asset directory not found. Reinstall via install.sh, run `agent-bar setup` from a checkout, or set AGENT_BAR_ASSET_DIR."
    );
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::settings::SeparatorStyle;

    fn s(v: &[&str]) -> Vec<String> {
        v.iter().map(|x| x.to_string()).collect()
    }

    #[test]
    fn modules_wire_click_handlers_through_terminal_helper() {
        let e = export_waybar_modules(
            "$HOME/.local/bin/agent-bar",
            "$HOME/.config/waybar/scripts/agent-bar-open-terminal",
            None,
            &s(&["claude", "codex", "amp"]),
            120,
        );
        let claude = &e.modules["custom/agent-bar-claude"];
        assert_eq!(
            claude.on_click,
            "$HOME/.config/waybar/scripts/agent-bar-open-terminal $HOME/.local/bin/agent-bar menu"
        );
        let codex = &e.modules["custom/agent-bar-codex"];
        assert!(codex.exec_on_event);
        assert_eq!(codex.exec, "$HOME/.local/bin/agent-bar --provider codex");
        let amp = &e.modules["custom/agent-bar-amp"];
        assert_eq!(
            amp.on_click_right,
            "$HOME/.config/waybar/scripts/agent-bar-open-terminal $HOME/.local/bin/agent-bar action-right amp"
        );
    }

    #[test]
    fn modules_only_for_requested_providers() {
        let e = export_waybar_modules(
            "/usr/bin/agent-bar",
            "/usr/bin/open-terminal",
            None,
            &s(&["claude"]),
            120,
        );
        assert_eq!(e.modules.len(), 1);
        assert!(e.modules.contains_key("custom/agent-bar-claude"));
        assert!(!e.modules.contains_key("custom/agent-bar-codex"));
    }

    #[test]
    fn signal_present_when_provided_absent_otherwise() {
        let with = export_waybar_modules("bin", "term", Some(8), &s(&["claude", "codex"]), 120);
        assert_eq!(with.modules["custom/agent-bar-claude"].signal, Some(8));
        let without = export_waybar_modules("bin", "term", None, &s(&["claude"]), 120);
        assert_eq!(without.modules["custom/agent-bar-claude"].signal, None);
    }

    #[test]
    fn module_definition_uses_settings_interval() {
        let m = module_definition("claude", "agent-bar", "term.sh", Some(8), 60);
        assert_eq!(m.interval, 60);
        let m2 = module_definition("claude", "agent-bar", "term.sh", Some(8), 120);
        assert_eq!(m2.interval, 120);
    }

    #[test]
    fn css_has_base_styles_icons_states() {
        let css = export_waybar_css(
            "/home/user/.config/waybar/agent-bar/icons",
            &s(&["claude", "codex", "amp"]),
            SeparatorStyle::Gap,
        );
        for sel in [
            "#custom-agent-bar-claude",
            "#custom-agent-bar-codex",
            "#custom-agent-bar-amp",
        ] {
            assert!(css.contains(sel), "missing {sel}");
        }
        for icon in ["claude-code-icon.png", "codex-icon.png", "amp-icon.svg"] {
            assert!(css.contains(icon), "missing {icon}");
        }
        for st in [".ok", ".low", ".warn", ".critical", ".disconnected"] {
            assert!(css.contains(st), "missing {st}");
        }
    }

    #[test]
    fn css_separator_styles_have_marker_and_distinct_props() {
        for st in [
            SeparatorStyle::Pill,
            SeparatorStyle::Gap,
            SeparatorStyle::Bare,
            SeparatorStyle::Glass,
            SeparatorStyle::Shadow,
            SeparatorStyle::None,
        ] {
            let css = export_waybar_css("/icons", &s(&["claude"]), st);
            assert!(css.len() > 100);
        }
        assert!(
            export_waybar_css("/i", &s(&["claude"]), SeparatorStyle::Pill)
                .contains("border-radius")
        );
        assert!(
            export_waybar_css("/i", &s(&["claude"]), SeparatorStyle::Bare)
                .contains("border-color: transparent")
        );
        assert!(export_waybar_css("/i", &s(&["claude"]), SeparatorStyle::Glass).contains("rgba("));
        assert!(
            export_waybar_css("/i", &s(&["claude"]), SeparatorStyle::Shadow).contains("box-shadow")
        );
        let none = export_waybar_css("/i", &s(&["claude"]), SeparatorStyle::None);
        assert!(none.contains("border-color: transparent") && none.contains("margin: 0"));
    }

    #[test]
    #[serial_test::serial]
    fn asset_root_honors_absolute_env_with_icons() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join("icons")).unwrap();
        temp_env::with_var("AGENT_BAR_ASSET_DIR", Some(dir.path().as_os_str()), || {
            assert_eq!(resolve_asset_source_root().unwrap(), dir.path());
        });
    }

    #[test]
    #[serial_test::serial]
    fn asset_root_throws_under_system_when_absent() {
        temp_env::with_vars(
            [
                ("AGENT_BAR_FORCE_COMPILED", Some("1")),
                ("AGENT_BAR_ASSET_DIR", None),
            ],
            || {
                let err = resolve_asset_source_root().unwrap_err().to_string();
                assert!(err.contains("Asset directory not found"), "got: {err}");
            },
        );
    }

    #[test]
    #[serial_test::serial]
    fn asset_root_throws_on_invalid_env() {
        temp_env::with_var("AGENT_BAR_ASSET_DIR", Some("/nonexistent-xyz"), || {
            assert!(resolve_asset_source_root()
                .unwrap_err()
                .to_string()
                .contains("AGENT_BAR_ASSET_DIR must be"));
        });
        temp_env::with_var("AGENT_BAR_ASSET_DIR", Some("relative/path"), || {
            assert!(resolve_asset_source_root()
                .unwrap_err()
                .to_string()
                .contains("AGENT_BAR_ASSET_DIR must be"));
        });
    }

    // -----------------------------------------------------------------------
    // Fix 3 (hotfix 7.0.1): candidato standalone em resolve_asset_source_root
    // -----------------------------------------------------------------------

    #[test]
    #[serial_test::serial]
    fn asset_root_honors_standalone_data_dir_without_asset_dir_env() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join("icons")).unwrap();
        temp_env::with_vars(
            [
                ("AGENT_BAR_ASSET_DIR", None),
                ("AGENT_BAR_FORCE_COMPILED", None),
                ("AGENT_BAR_DATA", Some(dir.path().as_os_str())),
            ],
            || {
                let resolved = resolve_asset_source_root().unwrap();
                assert_eq!(resolved, dir.path());
                // Prioridade respeitada: não caiu no fallback dev (a raiz do
                // repo, que também tem icons/ — ver install.sh/assets).
                assert_ne!(resolved, PathBuf::from(env!("CARGO_MANIFEST_DIR")));
            },
        );
    }

    #[test]
    #[serial_test::serial]
    fn asset_root_honors_xdg_data_home_fallback() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join(APP_NAME).join("icons")).unwrap();
        temp_env::with_vars(
            [
                ("AGENT_BAR_ASSET_DIR", None),
                ("AGENT_BAR_FORCE_COMPILED", None),
                ("AGENT_BAR_DATA", None),
                ("XDG_DATA_HOME", Some(dir.path().as_os_str())),
            ],
            || {
                let resolved = resolve_asset_source_root().unwrap();
                assert_eq!(resolved, dir.path().join(APP_NAME));
            },
        );
    }

    #[test]
    #[serial_test::serial]
    fn asset_root_falls_through_to_dev_when_standalone_has_no_icons() {
        // Diretório existe (candidato construível) mas sem icons/ — inválido;
        // o fallback dev (CARGO_MANIFEST_DIR, raiz do repo — tem icons/) só é
        // aceito porque o standalone falhou o guard `has_icons`.
        let dir = tempfile::tempdir().unwrap();
        temp_env::with_vars(
            [
                ("AGENT_BAR_ASSET_DIR", None),
                ("AGENT_BAR_FORCE_COMPILED", None),
                ("AGENT_BAR_DATA", Some(dir.path().as_os_str())),
            ],
            || {
                let resolved = resolve_asset_source_root().unwrap();
                assert_eq!(resolved, PathBuf::from(env!("CARGO_MANIFEST_DIR")));
            },
        );
    }

    #[test]
    #[serial_test::serial]
    fn asset_root_system_forced_errors_even_with_standalone_data_present() {
        // Sem nenhum candidato viável: System é forçado (sem `/usr/share/agent-bar`
        // real nesta máquina de teste) e o candidato standalone, mesmo com
        // icons/ presente, não deve ser consultado — System tem precedência e
        // falha antes de chegar lá. Mensagem final cita AGENT_BAR_ASSET_DIR.
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join("icons")).unwrap();
        let dir_str = dir.path().to_str().expect("tempdir path is valid UTF-8");
        temp_env::with_vars(
            [
                ("AGENT_BAR_FORCE_COMPILED", Some("1")),
                ("AGENT_BAR_ASSET_DIR", None),
                ("AGENT_BAR_DATA", Some(dir_str)),
            ],
            || {
                let err = resolve_asset_source_root().unwrap_err().to_string();
                assert!(err.contains("Asset directory not found"), "got: {err}");
                assert!(err.contains("AGENT_BAR_ASSET_DIR"), "got: {err}");
            },
        );
    }

    #[test]
    #[serial_test::serial]
    fn standalone_data_asset_dir_prefers_agent_bar_data_over_xdg() {
        temp_env::with_vars(
            [
                ("AGENT_BAR_DATA", Some("/custom/data")),
                ("XDG_DATA_HOME", Some("/xdg/data")),
            ],
            || {
                assert_eq!(
                    standalone_data_asset_dir(),
                    Some(PathBuf::from("/custom/data"))
                );
            },
        );
    }

    #[test]
    #[serial_test::serial]
    fn standalone_data_asset_dir_falls_back_to_home_local_share() {
        temp_env::with_vars(
            [
                ("AGENT_BAR_DATA", None::<&str>),
                ("XDG_DATA_HOME", None::<&str>),
            ],
            || {
                let home = std::env::var_os("HOME").expect("HOME must be set in test env");
                let expected = PathBuf::from(home)
                    .join(".local")
                    .join("share")
                    .join(APP_NAME);
                assert_eq!(standalone_data_asset_dir(), Some(expected));
            },
        );
    }

    #[test]
    #[serial_test::serial]
    fn standalone_data_dir_matches_update_default_data_dir_with_xdg_set() {
        // Hotfix 7.0.1 (fix(paths)): standalone_data_asset_dir (setup) e
        // update::default_data_dir (self-update) tinham que resolver pro
        // MESMO diretório — antes divergiam porque só um respeitava
        // XDG_DATA_HOME. Não mexe em $HOME real (evita flake em testes
        // não-serial de outros módulos que leem HOME concorrentemente);
        // como XDG_DATA_HOME é setado aqui, ele domina de qualquer forma.
        let real_home = std::env::var_os("HOME").expect("HOME must be set in test env");
        let home_path = PathBuf::from(&real_home);
        temp_env::with_vars(
            [
                ("AGENT_BAR_DATA", None::<&str>),
                ("XDG_DATA_HOME", Some("/xdg/data")),
            ],
            || {
                let from_setup = standalone_data_asset_dir().unwrap();
                let from_update = crate::update::default_data_dir(&home_path);
                assert_eq!(from_setup, from_update);
                assert_eq!(from_setup, PathBuf::from("/xdg/data/agent-bar"));
            },
        );
    }

    #[test]
    #[serial_test::serial]
    fn default_app_bin_system_vs_local() {
        temp_env::with_var("AGENT_BAR_FORCE_COMPILED", Some("1"), || {
            assert_eq!(get_default_waybar_asset_paths().app_bin, "agent-bar");
        });
        temp_env::with_var("AGENT_BAR_FORCE_COMPILED", None::<&str>, || {
            assert_eq!(
                get_default_waybar_asset_paths().app_bin,
                "$HOME/.local/bin/agent-bar"
            );
        });
    }

    #[test]
    fn install_copies_icons_and_helper_with_exec_perm() {
        use std::os::unix::fs::PermissionsExt;
        let src = tempfile::tempdir().unwrap();
        // fixture: src/icons/a.png + src/scripts/agent-bar-open-terminal
        std::fs::create_dir_all(src.path().join("icons")).unwrap();
        std::fs::write(src.path().join("icons").join("a.png"), b"png").unwrap();
        std::fs::create_dir_all(src.path().join("scripts")).unwrap();
        std::fs::write(
            src.path().join("scripts").join("agent-bar-open-terminal"),
            b"#!/bin/sh\n",
        )
        .unwrap();

        let dest = tempfile::tempdir().unwrap();
        let waybar_dir = dest.path().join("agent-bar");
        let scripts_dir = dest.path().join("scripts");

        let r = install_waybar_assets(&waybar_dir, &scripts_dir, Some(src.path())).unwrap();
        assert!(r.icons_dir.join("a.png").exists());
        assert!(r.terminal_script.exists());
        let mode = std::fs::metadata(&r.terminal_script)
            .unwrap()
            .permissions()
            .mode();
        assert_eq!(mode & 0o777, 0o755);
    }

    #[test]
    fn install_errors_when_icons_source_missing() {
        let src = tempfile::tempdir().unwrap(); // sem icons/
        std::fs::create_dir_all(src.path().join("scripts")).unwrap();
        std::fs::write(
            src.path().join("scripts").join("agent-bar-open-terminal"),
            b"x",
        )
        .unwrap();
        let dest = tempfile::tempdir().unwrap();
        let err = install_waybar_assets(
            &dest.path().join("a"),
            &dest.path().join("s"),
            Some(src.path()),
        )
        .unwrap_err()
        .to_string();
        assert!(err.contains("Icons folder not found"), "got: {err}");
    }
}
