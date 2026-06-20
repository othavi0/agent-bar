//! Patcher cirúrgico do Waybar config/style (JSONC). NÃO usa crate de JSONC —
//! comentários e ordem do arquivo do usuário precisam sobreviver. Premissa: a
//! ESTRUTURA do JSONC é ASCII (chaves/colchetes/aspas); valores não-ASCII ficam
//! dentro de strings, puladas por `skip_string`. Port de `src/waybar-integration.ts`.

use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use regex::Regex;
use serde::Serialize;

use crate::app_identity::{APP_NAME, BACKUP_SUFFIX, WAYBAR_MODULE_PREFIX, WAYBAR_NAMESPACE};
use crate::settings::Settings;

/// Avança além de um literal de string JSON; `i` aponta à aspa de abertura.
/// Retorna o índice logo após a aspa de fechamento.
pub(crate) fn skip_string(content: &[u8], mut i: usize) -> usize {
    i += 1;
    while i < content.len() {
        match content[i] {
            b'\\' => {
                i += 2;
                continue;
            }
            b'"' => return i + 1,
            _ => i += 1,
        }
    }
    i
}

/// Acha o `]` que fecha o `[` em `open_idx`, honrando colchetes aninhados,
/// strings e comentários JSONC. `None` se desbalanceado.
pub(crate) fn find_matching_bracket(content: &[u8], open_idx: usize) -> Option<usize> {
    let mut depth: i32 = 0;
    let mut i = open_idx;
    while i < content.len() {
        let c = content[i];
        if c == b'"' {
            i = skip_string(content, i);
            continue;
        }
        if c == b'/' && content.get(i + 1) == Some(&b'/') {
            // TS: i = nl === -1 ? content.length : nl (sem pular o \n — loop externo avança)
            match content[i..].iter().position(|&b| b == b'\n') {
                Some(rel) => {
                    i += rel;
                }
                None => {
                    i = content.len();
                    continue;
                }
            }
            continue;
        }
        if c == b'/' && content.get(i + 1) == Some(&b'*') {
            let rest = &content[i + 2..];
            match rest.windows(2).position(|w| w == b"*/") {
                Some(rel) => {
                    i = i + 2 + rel + 2;
                }
                None => {
                    i = content.len();
                    continue;
                }
            }
            continue;
        }
        if c == b'[' {
            depth += 1;
        } else if c == b']' {
            depth -= 1;
            if depth == 0 {
                return Some(i);
            }
        }
        i += 1;
    }
    None
}

/// Extrai os valores de string de um corpo `"a", "b", ...` (JSON-unescape via serde).
pub(crate) fn parse_quoted_strings(block: &str) -> Vec<String> {
    static RE: OnceLock<Option<Regex>> = OnceLock::new();
    let re = match RE.get_or_init(|| Regex::new(r#""((?:\\.|[^"\\])*)""#).ok()) {
        Some(r) => r,
        None => return Vec::new(),
    };
    let mut out = Vec::new();
    for cap in re.captures_iter(block) {
        let inner = &cap[1];
        if let Ok(s) = serde_json::from_str::<String>(&format!("\"{inner}\"")) {
            out.push(s);
        }
    }
    out
}

pub(crate) fn format_string_array(values: &[String], indent: &str) -> String {
    if values.is_empty() {
        return "[]".to_string();
    }
    let item_indent = format!("{indent}  ");
    let lines = values
        .iter()
        .map(|v| {
            format!(
                "{item_indent}{}",
                serde_json::to_string(v).unwrap_or_default()
            )
        })
        .collect::<Vec<_>>()
        .join(",\n");
    format!("[\n{lines}\n{indent}]")
}

pub(crate) struct RewriteArrayResult {
    pub(crate) content: String,
    pub(crate) found: bool,
    pub(crate) changed: bool,
}

pub(crate) fn rewrite_string_array_property(
    content: &str,
    property: &str,
    transform: impl Fn(Vec<String>) -> Vec<String>,
) -> RewriteArrayResult {
    let pattern = format!(r#""{}"\s*:\s*\["#, regex::escape(property));
    let re = match Regex::new(&pattern) {
        Ok(r) => r,
        Err(_) => {
            return RewriteArrayResult {
                content: content.to_string(),
                found: false,
                changed: false,
            }
        }
    };
    let bytes = content.as_bytes();
    for m in re.find_iter(content) {
        let match_start = m.start();
        let line_start = content[..match_start]
            .rfind('\n')
            .map(|p| p + 1)
            .unwrap_or(0);
        let line_prefix = &content[line_start..match_start];
        if line_prefix.contains("//") {
            continue;
        }
        let open_idx = m.end() - 1; // índice do '['
        let close_idx = match find_matching_bracket(bytes, open_idx) {
            Some(c) => c,
            None => continue,
        };
        let body = &content[open_idx + 1..close_idx];
        let current = parse_quoted_strings(body);
        let next = transform(current.clone());

        let indent: String = line_prefix
            .chars()
            .take_while(|c| c.is_whitespace())
            .collect();

        if current == next {
            return RewriteArrayResult {
                content: content.to_string(),
                found: true,
                changed: false,
            };
        }
        let prefix = &content[match_start..open_idx]; // `"prop"\s*:\s*`
        let rewritten = format!(
            "{}{}{}{}",
            &content[..match_start],
            prefix,
            format_string_array(&next, &indent),
            &content[close_idx + 1..]
        );
        return RewriteArrayResult {
            content: rewritten,
            found: true,
            changed: true,
        };
    }
    RewriteArrayResult {
        content: content.to_string(),
        found: false,
        changed: false,
    }
}

// ---------------------------------------------------------------------------
// Public API types
// ---------------------------------------------------------------------------

/// Paths de integração Waybar (config + namespaced includes).
#[derive(Debug, Clone)]
pub struct WaybarIntegrationPaths {
    pub waybar_config_path: PathBuf,
    pub waybar_style_path: PathBuf,
    pub modules_include_path: PathBuf,
    pub style_include_path: PathBuf,
}

/// Paths defaults: `~/.config/waybar/…`.
pub fn get_default_waybar_integration_paths() -> WaybarIntegrationPaths {
    let home = std::env::var("HOME").unwrap_or_else(|_| "/root".to_string());
    let waybar_root = PathBuf::from(&home).join(".config").join("waybar");
    WaybarIntegrationPaths {
        waybar_config_path: waybar_root.join("config.jsonc"),
        waybar_style_path: waybar_root.join("style.css"),
        modules_include_path: waybar_root.join(WAYBAR_NAMESPACE).join("modules.jsonc"),
        style_include_path: waybar_root.join(WAYBAR_NAMESPACE).join("style.css"),
    }
}

/// Retorna os IDs de módulo Waybar para a ordem de providers dada.
pub fn get_app_module_ids(order: &[String]) -> Vec<String> {
    order
        .iter()
        .map(|p| format!("{WAYBAR_MODULE_PREFIX}{p}"))
        .collect()
}

/// Opções para `apply_waybar_integration`.
pub struct ApplyOptions {
    pub paths: WaybarIntegrationPaths,
    pub icons_dir: Option<PathBuf>,
    pub app_bin: Option<String>,
    pub terminal_script: Option<PathBuf>,
}

/// Resultado de `apply_waybar_integration`.
pub struct ApplyResult {
    pub config_changed: bool,
    pub style_changed: bool,
    pub module_ids: Vec<String>,
    pub modules_include_path: PathBuf,
    pub style_include_path: PathBuf,
}

/// Resultado de `remove_waybar_integration`.
pub struct RemoveResult {
    pub config_changed: bool,
    pub style_changed: bool,
    pub removed_includes: Vec<PathBuf>,
}

// ---------------------------------------------------------------------------
// Private helpers
// ---------------------------------------------------------------------------

/// Lê um arquivo como texto; retorna None se não existe.
fn read_text(path: &Path) -> Option<String> {
    if !path.exists() {
        return None;
    }
    std::fs::read_to_string(path).ok()
}

/// Cria o diretório pai e escreve `content`, garantindo `\n` final.
fn write_text(path: &Path, content: &str) -> anyhow::Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let final_content = if content.ends_with('\n') {
        content.to_string()
    } else {
        format!("{content}\n")
    };
    std::fs::write(path, final_content)?;
    Ok(())
}

/// Copia `path` para `path + BACKUP_SUFFIX` se o backup ainda não existe.
fn backup_if_needed(path: &Path) -> anyhow::Result<()> {
    let backup = format!("{}{}", path.display(), BACKUP_SUFFIX);
    let backup_path = Path::new(&backup);
    if !backup_path.exists() && path.exists() {
        std::fs::copy(path, backup_path)?;
    }
    Ok(())
}

/// Insere `property_text` como primeira propriedade do primeiro objeto `{`.
/// Port de `insertPropertyIntoFirstObject` (TS :218-232).
fn insert_property_into_first_object(content: &str, property_text: &str) -> anyhow::Result<String> {
    let brace_idx = content.find('{').ok_or_else(|| {
        anyhow::anyhow!("Waybar config must contain an object to insert {APP_NAME} integration.")
    })?;

    let after_brace = &content[brace_idx + 1..];

    // Detecta indentação via regex \n(\s*)"
    static INDENT_RE: OnceLock<Option<Regex>> = OnceLock::new();
    let indent = INDENT_RE
        .get_or_init(|| Regex::new(r#"\n(\s*)""#).ok())
        .as_ref()
        .and_then(|re| re.captures(after_brace))
        .and_then(|cap| cap.get(1))
        .map(|m| m.as_str().to_string())
        .unwrap_or_else(|| "  ".to_string());

    let first_token = after_brace.trim_start();
    let object_is_empty = first_token.starts_with('}');
    let insertion = if object_is_empty {
        format!("\n{indent}{property_text}\n")
    } else {
        format!("\n{indent}{property_text},")
    };

    Ok(format!(
        "{}{insertion}{after_brace}",
        &content[..brace_idx + 1]
    ))
}

/// Retorna true se o módulo é gerenciado (prefixo known).
fn is_managed_module(value: &str) -> bool {
    value.starts_with(WAYBAR_MODULE_PREFIX)
}

/// Remove os imports gerenciados do CSS. Port de `stripManagedStyleImports` (TS :238-246).
/// Três regexes sequenciais: bloco de comentário, linha @import, linha em branco inicial.
fn strip_managed_style_imports(content: &str) -> String {
    let ns = regex::escape(WAYBAR_NAMESPACE);
    let app = regex::escape(APP_NAME);

    // Pattern 1: (?m)^\s*/\*\s*agent-bar managed import\s*\*/\n?
    let pat1 = [r"(?m)^\s*/\*\s*", app.as_str(), r" managed import\s*\*/\n?"].concat();

    // Pattern 2: (?m)^\s*@import\s+url\((?:"\./<ns>/style\.css"|'\./<ns>/style\.css')\);?\n?
    // Sem backreference (regex crate não suporta \1): as DUAS alternativas completas
    // ficam dentro do grupo `(?:...)`, com o `\)` do url() fora dele.
    let pat2 = [
        r#"(?m)^\s*@import\s+url\((?:"\./"#,
        ns.as_str(),
        r#"/style\.css"|'\./"#,
        ns.as_str(),
        r#"/style\.css')\);?\n?"#,
    ]
    .concat();

    let step1 = Regex::new(&pat1)
        .ok()
        .map(|re| re.replace_all(content, "").into_owned())
        .unwrap_or_else(|| content.to_string());

    let step2 = Regex::new(&pat2)
        .ok()
        .map(|re| re.replace_all(&step1, "").into_owned())
        .unwrap_or(step1);

    // Pattern 3: ^\s*\n — remove primeira linha em branco (sem (?m))
    static RE_BLANK: OnceLock<Option<Regex>> = OnceLock::new();
    RE_BLANK
        .get_or_init(|| Regex::new(r"^\s*\n").ok())
        .as_ref()
        .map(|re| re.replace(&step2, "").into_owned())
        .unwrap_or(step2)
}

/// Garante que `include_path` esteja no array `"include"` do config.
fn ensure_include_path(content: &str, include_path: &str) -> anyhow::Result<(String, bool)> {
    let include_path_owned = include_path.to_string();
    let result =
        rewrite_string_array_property(content, "include", move |mut values: Vec<String>| {
            if !values.contains(&include_path_owned) {
                values.push(include_path_owned.clone());
            }
            values
        });

    if result.found {
        return Ok((result.content, result.changed));
    }

    // Não encontrou "include" — inserir via insert_property_into_first_object
    let include_property = format!(
        "\"include\": {}",
        format_string_array(&[include_path.to_string()], "  ")
    );
    let next = insert_property_into_first_object(content, &include_property)?;
    Ok((next, true))
}

/// Remove os `include_paths` do array `"include"` do config.
fn remove_include_paths(content: &str, include_paths: &[&str]) -> (String, bool) {
    let set: std::collections::HashSet<&str> = include_paths.iter().copied().collect();
    let result = rewrite_string_array_property(content, "include", |values: Vec<String>| {
        values
            .into_iter()
            .filter(|v| !set.contains(v.as_str()))
            .collect()
    });
    (result.content, result.changed)
}

/// Reconcilia managed modules: substitui os existentes pelos novos, mantém não-managed,
/// anexa restantes. Port de `reconcileManagedModules` (TS :277-299).
fn reconcile_managed_modules(values: Vec<String>, module_ids: &[String]) -> Vec<String> {
    let mut next: Vec<String> = Vec::new();
    let mut module_index = 0usize;

    for value in &values {
        if is_managed_module(value) {
            if module_index < module_ids.len() {
                next.push(module_ids[module_index].clone());
                module_index += 1;
            }
            continue;
        }
        next.push(value.clone());
    }

    while module_index < module_ids.len() {
        next.push(module_ids[module_index].clone());
        module_index += 1;
    }

    next
}

/// Garante que os module IDs estejam em `"modules-right"`.
fn ensure_modules_right(content: &str, module_ids: &[String]) -> anyhow::Result<(String, bool)> {
    let ids = module_ids.to_vec();
    let result = rewrite_string_array_property(content, "modules-right", move |values| {
        reconcile_managed_modules(values, &ids)
    });

    if result.found {
        return Ok((result.content, result.changed));
    }

    let modules_property = format!(
        "\"modules-right\": {}",
        format_string_array(module_ids, "  ")
    );
    let next = insert_property_into_first_object(content, &modules_property)?;
    Ok((next, true))
}

/// Remove managed modules de `"modules-right"`.
fn remove_modules_right(content: &str) -> (String, bool) {
    let result = rewrite_string_array_property(content, "modules-right", |values| {
        values
            .into_iter()
            .filter(|v| !is_managed_module(v))
            .collect()
    });
    (result.content, result.changed)
}

/// Garante o import do CSS gerenciado no style.css.
fn ensure_style_import(content: &str) -> (String, bool) {
    let app_style_import = format!("@import url(\"./{WAYBAR_NAMESPACE}/style.css\");");
    let stripped = strip_managed_style_imports(content);
    let next = if !stripped.is_empty() {
        format!("/* {APP_NAME} managed import */\n{app_style_import}\n\n{stripped}")
    } else {
        format!("/* {APP_NAME} managed import */\n{app_style_import}\n")
    };
    let changed = next != content;
    (next, changed)
}

/// Remove o import do CSS gerenciado.
fn remove_style_import(content: &str) -> (String, bool) {
    let next = strip_managed_style_imports(content);
    let changed = next != content;
    (next, changed)
}

/// Struct para serialização do bootstrap config com ordem de chaves garantida.
#[derive(Serialize)]
struct BootstrapConfig<'a> {
    layer: &'a str,
    position: &'a str,
    #[serde(rename = "modules-left")]
    modules_left: &'a [String],
    #[serde(rename = "modules-center")]
    modules_center: &'a [String],
    #[serde(rename = "modules-right")]
    modules_right: &'a [String],
    include: &'a [String],
}

/// Constrói um config Waybar mínimo com os módulos e o include path.
/// Port de `buildBootstrapConfig` (TS :340-353).
fn build_bootstrap_config(module_ids: &[String], include_path: &str) -> String {
    let config = BootstrapConfig {
        layer: "top",
        position: "top",
        modules_left: &[],
        modules_center: &[],
        modules_right: module_ids,
        include: &[include_path.to_string()],
    };
    serde_json::to_string_pretty(&config).unwrap_or_default()
}

/// Deriva a ordem de providers a partir das settings.
/// Port de `resolveProviderOrder` (TS :355-364).
fn resolve_provider_order(settings: &Settings) -> Vec<String> {
    let (providers, provider_order) = crate::settings::normalize_provider_selection(
        &settings.waybar.providers,
        &settings.waybar.provider_order,
    );
    if !provider_order.is_empty() {
        provider_order
    } else {
        providers
    }
}

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/// Aplica a integração Waybar: escreve os include files e patcha config/style.
/// Port de `applyWaybarIntegration` (TS :380-436).
pub fn apply_waybar_integration(
    settings: &Settings,
    opts: ApplyOptions,
) -> anyhow::Result<ApplyResult> {
    let paths = opts.paths;
    let defaults = crate::waybar_contract::get_default_waybar_asset_paths();

    let provider_order = resolve_provider_order(settings);
    let module_ids = get_app_module_ids(&provider_order);

    // Exporta e grava o modules.jsonc (só o map `modules`, não o export inteiro)
    let app_bin = opts
        .app_bin
        .as_deref()
        .unwrap_or(&defaults.app_bin)
        .to_string();
    let terminal_script = opts
        .terminal_script
        .as_ref()
        .map(|p| p.to_string_lossy().into_owned())
        .unwrap_or_else(|| defaults.terminal_script.to_string_lossy().into_owned());

    let export = crate::waybar_contract::export_waybar_modules(
        &app_bin,
        &terminal_script,
        settings.waybar.signal,
        &provider_order,
    );
    let modules_json = serde_json::to_string_pretty(&export.modules)?;
    write_text(&paths.modules_include_path, &modules_json)?;

    // Exporta e grava o style.css
    let icons_dir = opts
        .icons_dir
        .as_ref()
        .map(|p| p.to_string_lossy().into_owned())
        .unwrap_or_else(|| defaults.icons_dir.to_string_lossy().into_owned());
    let css = crate::waybar_contract::export_waybar_css(
        &icons_dir,
        &provider_order,
        settings.waybar.separators,
    );
    write_text(&paths.style_include_path, &css)?;

    // Patcha (ou cria) o config do Waybar
    let include_path_str = paths
        .modules_include_path
        .to_str()
        .map(|s| s.to_string())
        .unwrap_or_else(|| paths.modules_include_path.to_string_lossy().into_owned());

    let current_config = read_text(&paths.waybar_config_path);
    let next_config = match &current_config {
        None => build_bootstrap_config(&module_ids, &include_path_str),
        Some(existing) => {
            let (with_include, _) = ensure_include_path(existing, &include_path_str)?;
            let (with_modules, _) = ensure_modules_right(&with_include, &module_ids)?;
            with_modules
        }
    };

    let config_changed = current_config.as_deref() != Some(&next_config);
    if config_changed {
        backup_if_needed(&paths.waybar_config_path)?;
        write_text(&paths.waybar_config_path, &next_config)?;
    }

    // Patcha o style do Waybar
    let current_style = read_text(&paths.waybar_style_path);
    let (next_style, style_changed_flag) =
        ensure_style_import(current_style.as_deref().unwrap_or(""));
    let style_changed = style_changed_flag || current_style.is_none();
    if style_changed {
        backup_if_needed(&paths.waybar_style_path)?;
        write_text(&paths.waybar_style_path, &next_style)?;
    }

    Ok(ApplyResult {
        config_changed,
        style_changed,
        module_ids,
        modules_include_path: paths.modules_include_path,
        style_include_path: paths.style_include_path,
    })
}

/// Remove a integração Waybar do config/style e apaga os include files gerados.
/// Port de `removeWaybarIntegration` (TS :438-481).
pub fn remove_waybar_integration(paths: &WaybarIntegrationPaths) -> anyhow::Result<RemoveResult> {
    // Patcha config
    let current_config = read_text(&paths.waybar_config_path);
    let mut config_changed = false;
    if let Some(existing) = current_config {
        let include_str = paths
            .modules_include_path
            .to_str()
            .map(|s| s.to_string())
            .unwrap_or_else(|| paths.modules_include_path.to_string_lossy().into_owned());
        let (after_include, inc_changed) = remove_include_paths(&existing, &[&include_str]);
        let (after_modules, mod_changed) = remove_modules_right(&after_include);
        config_changed = inc_changed || mod_changed;
        if config_changed {
            backup_if_needed(&paths.waybar_config_path)?;
            write_text(&paths.waybar_config_path, &after_modules)?;
        }
    }

    // Patcha style
    let current_style = read_text(&paths.waybar_style_path);
    let mut style_changed = false;
    if let Some(existing) = current_style {
        let (next_style, changed) = remove_style_import(&existing);
        style_changed = changed;
        if style_changed {
            backup_if_needed(&paths.waybar_style_path)?;
            write_text(&paths.waybar_style_path, &next_style)?;
        }
    }

    // Remove include files gerados
    let mut removed_includes: Vec<PathBuf> = Vec::new();
    for path in [&paths.modules_include_path, &paths.style_include_path] {
        if path.exists() {
            std::fs::remove_file(path)?;
            removed_includes.push(path.clone());
        }
    }

    Ok(RemoveResult {
        config_changed,
        style_changed,
        removed_includes,
    })
}

// ---------------------------------------------------------------------------
// Tests T3a
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn find_bracket_balances_nested_and_strings() {
        // [ "a", ["b","c"], "]" ]  → fecha no último ]
        let s = r#"[ "a", ["b","c"], "]" ]"#;
        let b = s.as_bytes();
        let open = 0;
        let close = find_matching_bracket(b, open).unwrap();
        assert_eq!(&s[open..=close], s); // o array inteiro
    }

    #[test]
    fn find_bracket_skips_line_and_block_comments() {
        let s = "[ \"a\", // ] not this\n \"b\" /* ] */ ]";
        let close = find_matching_bracket(s.as_bytes(), 0).unwrap();
        assert_eq!(s.as_bytes()[close], b']');
        // o ] final, não os comentados
        assert_eq!(close, s.len() - 1);
    }

    #[test]
    fn find_bracket_unbalanced_returns_none() {
        assert_eq!(find_matching_bracket(b"[ \"a\"", 0), None);
    }

    #[test]
    fn strip_managed_import_removes_both_quote_styles() {
        let ns = WAYBAR_NAMESPACE;
        // Aspas duplas (o que o APP_STYLE_IMPORT gera) + comentário gerenciado.
        let dq = format!(
            "/* {APP_NAME} managed import */\n@import url(\"./{ns}/style.css\");\n\nwindow {{ color: red; }}\n"
        );
        let out_dq = strip_managed_style_imports(&dq);
        assert!(
            !out_dq.contains("@import"),
            "double-quote import não removido: {out_dq}"
        );
        assert!(out_dq.contains("window { color: red; }"));

        // Aspas simples (tolerância herdada do TS via `\1` backref).
        let sq = format!("@import url('./{ns}/style.css');\nwindow {{ color: blue; }}\n");
        let out_sq = strip_managed_style_imports(&sq);
        assert!(
            !out_sq.contains("@import"),
            "single-quote import não removido: {out_sq}"
        );
        assert!(out_sq.contains("window { color: blue; }"));
    }

    #[test]
    fn parse_quoted_unescapes_via_json() {
        let v = parse_quoted_strings(r#""a", "b\"c", "d\\e""#);
        assert_eq!(
            v,
            vec!["a".to_string(), "b\"c".to_string(), "d\\e".to_string()]
        );
    }

    #[test]
    fn format_array_empty_and_nonempty() {
        assert_eq!(format_string_array(&[], "  "), "[]");
        assert_eq!(
            format_string_array(&["x".into(), "y".into()], "  "),
            "[\n    \"x\",\n    \"y\"\n  ]"
        );
    }

    #[test]
    fn rewrite_appends_when_found() {
        let content = "{\n  \"include\": [\"a\"]\n}";
        let r = rewrite_string_array_property(content, "include", |mut v| {
            v.push("b".into());
            v
        });
        assert!(r.found && r.changed);
        assert!(r.content.contains("\"a\""));
        assert!(r.content.contains("\"b\""));
    }

    #[test]
    fn rewrite_skips_commented_line_and_reports_not_found() {
        let content = "{\n  // \"include\": [\"old\"],\n}";
        let r = rewrite_string_array_property(content, "include", |v| v);
        assert!(!r.found);
        assert_eq!(r.content, content);
    }

    #[test]
    fn rewrite_no_change_when_transform_identity() {
        let content = "{\n  \"include\": [\"a\"]\n}";
        let r = rewrite_string_array_property(content, "include", |v| v);
        assert!(r.found && !r.changed);
    }
}

// ---------------------------------------------------------------------------
// Tests T3b (orchestration)
// ---------------------------------------------------------------------------

#[cfg(test)]
mod orchestration_tests {
    use super::*;
    use crate::config::Paths;
    use crate::settings::load;
    use std::path::PathBuf;
    use tempfile::tempdir;

    fn strip_jsonc(s: &str) -> String {
        // remove /* */ e // ... para validar via serde_json
        let block = regex::Regex::new(r"(?s)/\*.*?\*/")
            .unwrap()
            .replace_all(s, "");
        regex::Regex::new(r"(?m)^\s*//.*$")
            .unwrap()
            .replace_all(&block, "")
            .to_string()
    }

    fn test_paths(dir: &std::path::Path) -> WaybarIntegrationPaths {
        WaybarIntegrationPaths {
            waybar_config_path: dir.join("config.jsonc"),
            waybar_style_path: dir.join("style.css"),
            modules_include_path: dir.join("agent-bar").join("modules.jsonc"),
            style_include_path: dir.join("agent-bar").join("style.css"),
        }
    }

    fn default_settings(dir: &std::path::Path) -> crate::settings::Settings {
        load(&Paths {
            cache_dir: dir.join("cache"),
            config_dir: dir.join("config"),
            claude_credentials: PathBuf::new(),
            codex_auth: PathBuf::new(),
            codex_sessions: PathBuf::new(),
            amp_settings: PathBuf::new(),
            amp_threads: PathBuf::new(),
        })
    }

    fn apply_opts(p: &WaybarIntegrationPaths) -> ApplyOptions {
        ApplyOptions {
            paths: p.clone(),
            icons_dir: Some(PathBuf::from("/icons")),
            app_bin: Some("/bin/agent-bar".to_string()),
            terminal_script: Some(PathBuf::from("/bin/term")),
        }
    }

    #[test]
    fn adds_managed_modules_preserving_existing() {
        let dir = tempdir().unwrap();
        let p = test_paths(dir.path());
        std::fs::write(
            &p.waybar_config_path,
            "{\n  \"modules-right\": [\"clock\", \"battery\"],\n  \"include\": [\"/existing/include.jsonc\"]\n}",
        )
        .unwrap();
        let s = default_settings(dir.path());
        let r = apply_waybar_integration(&s, apply_opts(&p)).unwrap();
        assert!(r.config_changed);
        let patched = std::fs::read_to_string(&p.waybar_config_path).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&strip_jsonc(&patched)).unwrap();
        let mr = parsed["modules-right"].as_array().unwrap();
        assert!(mr.iter().any(|v| v == "clock"));
        assert!(mr.iter().any(|v| v == "battery"));
        for id in get_app_module_ids(&["claude".into(), "codex".into(), "amp".into()]) {
            assert!(mr.iter().any(|v| v.as_str() == Some(&id)), "missing {id}");
        }
        let inc = parsed["include"].as_array().unwrap();
        assert!(inc.iter().any(|v| v == "/existing/include.jsonc"));
        assert!(inc
            .iter()
            .any(|v| v.as_str() == Some(p.modules_include_path.to_str().unwrap())));
    }

    #[test]
    fn does_not_corrupt_nested_array() {
        let dir = tempdir().unwrap();
        let p = test_paths(dir.path());
        std::fs::write(
            &p.waybar_config_path,
            "{\n  \"modules-right\": [\"clock\", {\"name\": \"x\", \"items\": [\"a\", \"b\"]}],\n  \"include\": []\n}",
        )
        .unwrap();
        let s = default_settings(dir.path());
        apply_waybar_integration(&s, apply_opts(&p)).unwrap();
        let patched = std::fs::read_to_string(&p.waybar_config_path).unwrap();
        assert!(serde_json::from_str::<serde_json::Value>(&strip_jsonc(&patched)).is_ok());
    }

    #[test]
    fn leaves_commented_modules_right_untouched() {
        let dir = tempdir().unwrap();
        let p = test_paths(dir.path());
        std::fs::write(
            &p.waybar_config_path,
            "{\n  // \"modules-right\": [\"old-module\"],\n  \"modules-right\": [\"clock\"],\n  \"include\": []\n}",
        )
        .unwrap();
        let s = default_settings(dir.path());
        apply_waybar_integration(&s, apply_opts(&p)).unwrap();
        let patched = std::fs::read_to_string(&p.waybar_config_path).unwrap();
        assert!(patched.contains("// \"modules-right\": [\"old-module\"],"));
        let parsed: serde_json::Value = serde_json::from_str(&strip_jsonc(&patched)).unwrap();
        let mr = parsed["modules-right"].as_array().unwrap();
        assert!(mr.iter().any(|v| v == "clock"));
        assert!(mr.iter().any(|v| v == "custom/agent-bar-claude"));
    }

    #[test]
    fn round_trip_remove_reverses_apply_with_backup() {
        let dir = tempdir().unwrap();
        let p = test_paths(dir.path());
        std::fs::write(
            &p.waybar_config_path,
            "{\n  \"modules-right\": [\"clock\"],\n  \"include\": []\n}",
        )
        .unwrap();
        std::fs::write(&p.waybar_style_path, "window { color: red; }\n").unwrap();
        let s = default_settings(dir.path());
        apply_waybar_integration(&s, apply_opts(&p)).unwrap();
        let rr = remove_waybar_integration(&p).unwrap();
        assert!(rr.config_changed);
        let final_cfg: serde_json::Value = serde_json::from_str(&strip_jsonc(
            &std::fs::read_to_string(&p.waybar_config_path).unwrap(),
        ))
        .unwrap();
        let mr = final_cfg["modules-right"].as_array().unwrap();
        for id in get_app_module_ids(&["claude".into(), "codex".into(), "amp".into()]) {
            assert!(!mr.iter().any(|v| v.as_str() == Some(&id)));
        }
        assert!(mr.iter().any(|v| v == "clock"));
        let backup = format!(
            "{}{}",
            p.waybar_style_path.display(),
            crate::app_identity::BACKUP_SUFFIX
        );
        assert!(std::fs::read_to_string(&backup)
            .unwrap()
            .contains("window { color: red; }"));
    }
}
