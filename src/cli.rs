//! Parsing de argumentos da CLI — port fiel de `src/cli.ts:118-337`.
//!
//! Design consciente: `parseArgs` do TS chama `process.exit` direto; aqui modelamos
//! como `Result<CliOptions, CliError>` + `warnings: Vec<String>` para que o parse seja
//! 100% testável sem mockar exit. Erros fatais → `CliError`; avisos não-fatais → `warnings`.
//! O `main` (Task 6) imprime e sai com 1.

use crate::app_identity::APP_NAME;
use crate::config::DEFAULT_INTERVAL_SECS;

/// Comando principal a executar (mapeia os valores de `command` do TS 1:1).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Command {
    Waybar,
    Terminal,
    Menu,
    Status,
    Help,
    Version,
    ActionRight,
    Setup,
    AssetsInstall,
    ExportWaybarModules,
    ExportWaybarCss,
    Update,
    Uninstall,
    Remove,
    Doctor,
    /// Comando interno oculto — imprime `{font_family}\t{font_size}\n` de
    /// `settings.menu` p/ o helper `scripts/agent-bar-open-terminal`. Não
    /// aparece no help nem em `KNOWN_COMMANDS` (não é sugerido em typos).
    MenuFont,
}

/// Formato de saída do módulo Waybar.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Format {
    Waybar,
    Json,
}

/// Opções resultantes do parse — equivale ao `CliOptions` do TS.
#[derive(Debug, Clone, PartialEq)]
pub struct CliOptions {
    pub command: Command,
    pub refresh: bool,
    pub provider: Option<String>,
    pub verbose: bool,
    pub format: Format,
    pub watch: bool,
    pub interval_seconds: u32,
    pub waybar_dir: Option<String>,
    pub omarchy_plugins_dir: Option<String>,
    pub scripts_dir: Option<String>,
    pub icons_dir: Option<String>,
    pub app_bin: Option<String>,
    pub terminal_script: Option<String>,
    pub dry_run: bool,
    pub yes: bool,
    /// Avisos não-fatais coletados durante o parse — o caller imprime em stderr.
    pub warnings: Vec<String>,
}

impl Default for CliOptions {
    fn default() -> Self {
        Self {
            command: Command::Waybar,
            refresh: false,
            provider: None,
            verbose: false,
            format: Format::Waybar,
            watch: false,
            interval_seconds: DEFAULT_INTERVAL_SECS,
            waybar_dir: None,
            omarchy_plugins_dir: None,
            scripts_dir: None,
            icons_dir: None,
            app_bin: None,
            terminal_script: None,
            dry_run: false,
            yes: false,
            warnings: Vec::new(),
        }
    }
}

/// Erro fatal de parsing — o caller imprime `message` em stderr e sai com 1.
#[derive(Debug, Clone, PartialEq)]
pub struct CliError {
    pub message: String,
}

/// Comandos conhecidos para sugestão de typo (verbatim do TS:126).
const KNOWN_COMMANDS: &[&str] = &[
    "menu",
    "status",
    "setup",
    "assets",
    "export",
    "update",
    "uninstall",
    "remove",
    "action-right",
    "doctor",
    "help",
];

/// Distância de Levenshtein clássica (DP) — port fiel do TS:140-152.
fn levenshtein(a: &str, b: &str) -> usize {
    let a: Vec<char> = a.chars().collect();
    let b: Vec<char> = b.chars().collect();
    let m = a.len();
    let n = b.len();

    // dp[i][j] = distância entre a[..i] e b[..j]
    let mut dp = vec![vec![0usize; n + 1]; m + 1];

    // Inicialização: distâncias das strings vazias
    for (i, row) in dp.iter_mut().enumerate() {
        row[0] = i;
    }
    for (j, cell) in dp[0].iter_mut().enumerate() {
        *cell = j;
    }

    for i in 1..=m {
        for j in 1..=n {
            dp[i][j] = if a[i - 1] == b[j - 1] {
                dp[i - 1][j - 1]
            } else {
                1 + dp[i - 1][j].min(dp[i][j - 1]).min(dp[i - 1][j - 1])
            };
        }
    }
    dp[m][n]
}

/// Retorna o comando mais próximo se distância ≤ 3, senão `None` — port do TS:154-165.
fn suggest_command(input: &str) -> Option<&'static str> {
    let mut best: Option<&'static str> = None;
    let mut best_dist = usize::MAX;
    for &cmd in KNOWN_COMMANDS {
        let d = levenshtein(input, cmd);
        if d < best_dist {
            best_dist = d;
            best = Some(cmd);
        }
    }
    if best_dist <= 3 {
        best
    } else {
        None
    }
}

/// Extrai o próximo argumento da fatia; retorna `Err` se não houver.
/// Equivale a `requireNextArg` do TS:118-124.
fn require_next_arg<'a>(args: &'a [String], i: usize, flag: &str) -> Result<&'a str, CliError> {
    if i + 1 >= args.len() {
        return Err(CliError {
            message: format!("Error: {flag} requires a value"),
        });
    }
    Ok(&args[i + 1])
}

/// Parse fiel de `parseArgs` do TS:167-337.
///
/// Retorna `Ok(CliOptions)` com `warnings` para avisos não-fatais, ou
/// `Err(CliError)` para erros fatais (caller imprime e sai com 1).
pub fn parse_args(args: &[String]) -> Result<CliOptions, CliError> {
    let mut opts = CliOptions::default();
    let mut format_given = false;
    let mut interval_given = false;

    let mut i = 0;
    while i < args.len() {
        let arg = &args[i];

        match arg.as_str() {
            "menu" => opts.command = Command::Menu,
            "status" => opts.command = Command::Status,
            "setup" => opts.command = Command::Setup,
            // Oculto: usado só pelo helper Bash (scripts/agent-bar-open-terminal).
            "menu-font" => opts.command = Command::MenuFont,

            "assets" => {
                if args.get(i + 1).map(|s| s.as_str()) == Some("install") {
                    opts.command = Command::AssetsInstall;
                    i += 1;
                } else {
                    return Err(CliError {
                        message: "Unknown subcommand for 'assets'. Did you mean 'assets install'?"
                            .to_string(),
                    });
                }
            }

            "export" => match args.get(i + 1).map(|s| s.as_str()) {
                Some("waybar-modules") => {
                    opts.command = Command::ExportWaybarModules;
                    i += 1;
                }
                Some("waybar-css") => {
                    opts.command = Command::ExportWaybarCss;
                    i += 1;
                }
                _ => {
                    return Err(CliError {
                            message:
                                "Unknown subcommand for 'export'. Use 'export waybar-modules' or 'export waybar-css'."
                                    .to_string(),
                        });
                }
            },

            "update" => opts.command = Command::Update,
            "uninstall" => opts.command = Command::Uninstall,
            "remove" => opts.command = Command::Remove,
            "doctor" => opts.command = Command::Doctor,

            "action-right" => {
                let provider = require_next_arg(args, i, "action-right")?;
                opts.provider = Some(provider.to_string());
                opts.command = Command::ActionRight;
                i += 1;
            }

            "--dry-run" => opts.dry_run = true,

            "--yes" | "-y" => opts.yes = true,

            "--terminal" | "-t" => opts.command = Command::Terminal,

            "--refresh" | "-r" => opts.refresh = true,

            "--provider" | "-p" => {
                let val = require_next_arg(args, i, "--provider")?;
                opts.provider = Some(val.to_string());
                i += 1;
            }

            "--verbose" | "-v" => opts.verbose = true,

            "--format" => {
                let val = require_next_arg(args, i, "--format")?;
                match val {
                    "waybar" => opts.format = Format::Waybar,
                    "json" => opts.format = Format::Json,
                    v => {
                        return Err(CliError {
                            message: format!(
                                "Error: --format must be 'waybar' or 'json' (got '{v}')"
                            ),
                        });
                    }
                }
                format_given = true;
                i += 1;
            }

            "--watch" => opts.watch = true,

            "--interval" => {
                let val = require_next_arg(args, i, "--interval")?;
                // Usa parse::<i64> para rejeitar "1.5", "abc" etc.
                // "0".parse::<i64>() == Ok(0) mas 0 > 0 é false → rejeita.
                match val.parse::<i64>() {
                    Ok(n) if n > 0 => opts.interval_seconds = n as u32,
                    _ => {
                        return Err(CliError {
                            message: format!(
                                "Error: --interval must be a positive integer (got '{val}')"
                            ),
                        });
                    }
                }
                interval_given = true;
                i += 1;
            }

            "--waybar-dir" => {
                let val = require_next_arg(args, i, "--waybar-dir")?;
                opts.waybar_dir = Some(val.to_string());
                i += 1;
            }

            "--omarchy-plugins-dir" => {
                let val = require_next_arg(args, i, "--omarchy-plugins-dir")?;
                opts.omarchy_plugins_dir = Some(val.to_string());
                i += 1;
            }

            "--scripts-dir" => {
                let val = require_next_arg(args, i, "--scripts-dir")?;
                opts.scripts_dir = Some(val.to_string());
                i += 1;
            }

            "--icons-dir" => {
                let val = require_next_arg(args, i, "--icons-dir")?;
                opts.icons_dir = Some(val.to_string());
                i += 1;
            }

            "--app-bin" => {
                let val = require_next_arg(args, i, "--app-bin")?;
                opts.app_bin = Some(val.to_string());
                i += 1;
            }

            "--terminal-script" => {
                let val = require_next_arg(args, i, "--terminal-script")?;
                opts.terminal_script = Some(val.to_string());
                i += 1;
            }

            "--help" | "-h" | "help" => opts.command = Command::Help,

            "--version" | "-V" => opts.command = Command::Version,

            _ => {
                if arg.starts_with('-') {
                    // Flag desconhecida: aviso não-fatal (TS usa logger.warn)
                    opts.warnings.push(format!("Unknown option: {arg}"));
                } else {
                    // Comando desconhecido: erro fatal com sugestão se ≤3 edits
                    match suggest_command(arg) {
                        Some(s) => {
                            return Err(CliError {
                                message: format!("Unknown command: {arg}. Did you mean '{s}'?"),
                            });
                        }
                        None => {
                            return Err(CliError {
                                message: format!(
                                    "Unknown command: {arg}. Run '{APP_NAME} help' for available commands."
                                ),
                            });
                        }
                    }
                }
            }
        }

        i += 1;
    }

    // Pós-loop: validações cruzadas (TS:325-334)
    if opts.watch {
        if format_given && opts.format == Format::Waybar {
            return Err(CliError {
                message: "Error: --watch requires --format json".to_string(),
            });
        }
        opts.format = Format::Json;
    }
    if interval_given && !opts.watch {
        opts.warnings
            .push("[agent-bar] --interval has no effect without --watch".to_string());
    }

    Ok(opts)
}

// ---------------------------------------------------------------------------
// Helpers internos de renderização para build_help
// ---------------------------------------------------------------------------

use crate::theme::{box_chars, ColorToken, ANSI_BOLD, ANSI_RESET};

/// Aplica um código ANSI — retorna `""` quando `no_color == true`.
#[inline]
fn paint(code: &str, no_color: bool) -> &str {
    if no_color {
        ""
    } else {
        code
    }
}

/// Versão de `ColorToken::ansi()` que respeita `no_color`.
fn color(token: ColorToken, no_color: bool) -> String {
    if no_color {
        String::new()
    } else {
        token.ansi()
    }
}

// Coluna de alinhamento (= COL1 do TS).
const COL1: usize = 22;

/// Linha vertical simples (equivale ao `v()` do TS).
fn v_line(no_color: bool) -> String {
    format!(
        "{}{}{}",
        color(ColorToken::Magenta, no_color),
        box_chars::V,
        paint(ANSI_RESET, no_color),
    )
}

/// Header de seção com `◆` e negrito (equivale a `label()` do TS).
fn label_line(text: &str, no_color: bool) -> String {
    let mg = color(ColorToken::Magenta, no_color);
    let bold = paint(ANSI_BOLD, no_color);
    let rst = paint(ANSI_RESET, no_color);
    format!(
        "{mg}{lt}{h}{rst} {mg}{bold}{dia} {text}{rst}",
        mg = mg,
        lt = box_chars::LT,
        h = box_chars::H,
        rst = rst,
        bold = bold,
        dia = box_chars::DIAMOND,
        text = text,
    )
}

/// Linha de comando (verde) — equivale a `cmdLine()` do TS.
fn cmd_line(name: &str, desc: &str, no_color: bool) -> String {
    format!(
        "{vl}  {gc}{dot}{rst} {tb}{name:<col1$}{rst}{mt}{desc}{rst}",
        vl = v_line(no_color),
        gc = color(ColorToken::Green, no_color),
        dot = box_chars::DOT,
        rst = paint(ANSI_RESET, no_color),
        tb = color(ColorToken::TextBright, no_color),
        name = name,
        col1 = COL1,
        mt = color(ColorToken::Muted, no_color),
        desc = desc,
    )
}

/// Linha de opção/flag (amarela) — equivale a `optLine()` do TS.
fn opt_line(flags: &str, desc: &str, no_color: bool) -> String {
    format!(
        "{vl}  {yc}{dot}{rst} {tb}{flags:<col1$}{rst}{mt}{desc}{rst}",
        vl = v_line(no_color),
        yc = color(ColorToken::Yellow, no_color),
        dot = box_chars::DOT,
        rst = paint(ANSI_RESET, no_color),
        tb = color(ColorToken::TextBright, no_color),
        flags = flags,
        col1 = COL1,
        mt = color(ColorToken::Muted, no_color),
        desc = desc,
    )
}

/// Linha de informação (laranja) — equivale a `infoLine()` do TS.
fn info_line(key: &str, val: &str, no_color: bool) -> String {
    format!(
        "{vl}  {oc}{dot}{rst} {oc}{key:<col1$}{rst}{cm}{val}{rst}",
        vl = v_line(no_color),
        oc = color(ColorToken::Orange, no_color),
        dot = box_chars::DOT,
        rst = paint(ANSI_RESET, no_color),
        key = key,
        col1 = COL1,
        cm = color(ColorToken::Comment, no_color),
        val = val,
    )
}

/// Linha Waybar (ação → descrição) — equivale a `wbLine()` do TS.
fn wb_line(action: &str, desc: &str, no_color: bool) -> String {
    format!(
        "{vl}  {tb}{action:<col1$}{rst}{cm}→{rst} {mt}{desc}{rst}",
        vl = v_line(no_color),
        tb = color(ColorToken::TextBright, no_color),
        action = action,
        col1 = COL1,
        rst = paint(ANSI_RESET, no_color),
        cm = color(ColorToken::Comment, no_color),
        mt = color(ColorToken::Muted, no_color),
        desc = desc,
    )
}

// ---------------------------------------------------------------------------
// API pública: build_help / show_help
// ---------------------------------------------------------------------------

use crate::app_identity::VERSION;

/// Monta o texto de ajuda completo (multilinha, terminado em `\n`).
///
/// Porta fiel de `showHelp()` de `src/cli.ts:62-116`.
/// `no_color == true` elimina todos os códigos ANSI; texto e box chars ficam.
pub fn build_help(no_color: bool) -> String {
    let mg = color(ColorToken::Magenta, no_color);
    let bold = paint(ANSI_BOLD, no_color);
    let rst = paint(ANSI_RESET, no_color);
    let cm = color(ColorToken::Comment, no_color);

    let w: usize = 58;
    // repeat de H no header: max(0, w - APP_NAME.len() - 8)
    let h_count = w.saturating_sub(APP_NAME.len() + 8);
    let h_repeat = box_chars::H.repeat(h_count);
    // repeat de H no footer: w = 58
    let footer_h = box_chars::H.repeat(w);

    let mut out = String::new();

    // Linha em branco antes do header
    out.push('\n');

    // Header: ┏━ agent-bar v<version> ━━━…
    out.push_str(&format!(
        "{mg}{tl}{h}{rst} {mg}{bold}{name}{rst} {cm}v{version}{rst} {mg}{h_repeat}{rst}\n",
        mg = mg,
        tl = box_chars::TL,
        h = box_chars::H,
        rst = rst,
        bold = bold,
        name = APP_NAME,
        cm = cm,
        version = VERSION,
        h_repeat = h_repeat,
    ));

    out.push_str(&format!("{}\n", v_line(no_color)));

    // Seção Commands
    out.push_str(&format!("{}\n", label_line("Commands", no_color)));
    out.push_str(&format!(
        "{}\n",
        cmd_line("menu", "Interactive TUI menu", no_color)
    ));
    out.push_str(&format!(
        "{}\n",
        cmd_line("status", "Show quotas in terminal", no_color)
    ));
    out.push_str(&format!(
        "{}\n",
        cmd_line(
            "setup",
            &format!("Install + wire {APP_NAME} in Waybar"),
            no_color
        )
    ));
    out.push_str(&format!(
        "{}\n",
        cmd_line("assets install", "Install icons/helper only", no_color)
    ));
    out.push_str(&format!(
        "{}\n",
        cmd_line(
            "export waybar-modules",
            "Print Waybar JSON module contract",
            no_color
        )
    ));
    out.push_str(&format!(
        "{}\n",
        cmd_line(
            "export waybar-css",
            "Print Waybar CSS JSON contract",
            no_color
        )
    ));
    out.push_str(&format!(
        "{}\n",
        cmd_line(
            "update",
            "Update the install (self-update or managed checkout)",
            no_color
        )
    ));
    out.push_str(&format!(
        "{}\n",
        cmd_line(
            "uninstall",
            &format!("Remove {APP_NAME} + integration"),
            no_color
        )
    ));
    out.push_str(&format!(
        "{}\n",
        cmd_line("remove", "Force remove without prompt", no_color)
    ));
    out.push_str(&format!(
        "{}\n",
        cmd_line(
            "doctor",
            &format!("Detect & clean {APP_NAME} leftovers in $HOME"),
            no_color
        )
    ));
    out.push_str(&format!("{}\n", v_line(no_color)));

    // Seção Waybar
    out.push_str(&format!("{}\n", label_line("Waybar", no_color)));
    out.push_str(&format!(
        "{}\n",
        wb_line("Left click", "Interactive menu", no_color)
    ));
    out.push_str(&format!(
        "{}\n",
        wb_line("Right click", "Refresh / Login", no_color)
    ));
    out.push_str(&format!(
        "{}\n",
        wb_line("Hover", "Detailed tooltip", no_color)
    ));
    out.push_str(&format!("{}\n", v_line(no_color)));

    // Seção Flags
    out.push_str(&format!("{}\n", label_line("Flags", no_color)));
    out.push_str(&format!(
        "{}\n",
        opt_line(
            "--provider, -p <id>",
            "Single provider (Waybar module)",
            no_color
        )
    ));
    out.push_str(&format!(
        "{}\n",
        opt_line("--refresh, -r", "Invalidate cache before output", no_color)
    ));
    out.push_str(&format!(
        "{}\n",
        opt_line("--verbose, -v", "Debug logging to stderr", no_color)
    ));
    out.push_str(&format!(
        "{}\n",
        opt_line("--version, -V", "Print version and exit", no_color)
    ));
    out.push_str(&format!(
        "{}\n",
        opt_line(
            "--format <fmt>",
            "Output format: waybar (default) | json",
            no_color
        )
    ));
    out.push_str(&format!(
        "{}\n",
        opt_line("--watch", "Stream NDJSON (implies --format json)", no_color)
    ));
    out.push_str(&format!(
        "{}\n",
        opt_line(
            "--interval <s>",
            "Watch poll floor in seconds (default 60)",
            no_color
        )
    ));
    out.push_str(&format!(
        "{}\n",
        opt_line("--dry-run", "Preview changes (doctor)", no_color)
    ));
    out.push_str(&format!(
        "{}\n",
        opt_line("--yes, -y", "Assume yes (doctor/uninstall)", no_color)
    ));
    out.push_str(&format!(
        "{}\n",
        opt_line("--waybar-dir <path>", "Assets install target", no_color)
    ));
    out.push_str(&format!(
        "{}\n",
        opt_line(
            "--omarchy-plugins-dir <path>",
            "Omarchy plugin target (setup)",
            no_color
        )
    ));
    out.push_str(&format!(
        "{}\n",
        opt_line("--scripts-dir <path>", "Terminal helper target", no_color)
    ));
    out.push_str(&format!(
        "{}\n",
        opt_line("--icons-dir <path>", "CSS export icon directory", no_color)
    ));
    out.push_str(&format!(
        "{}\n",
        opt_line("--app-bin <path>", "Modules export app binary", no_color)
    ));
    out.push_str(&format!(
        "{}\n",
        opt_line(
            "--terminal-script <path>",
            "Modules export launcher",
            no_color
        )
    ));
    out.push_str(&format!("{}\n", v_line(no_color)));

    // Seção Info
    out.push_str(&format!("{}\n", label_line("Info", no_color)));
    out.push_str(&format!(
        "{}\n",
        info_line("Run with", &format!("{APP_NAME}  or  cargo run"), no_color)
    ));
    out.push_str(&format!("{}\n", v_line(no_color)));

    // Footer: ┗━━…
    out.push_str(&format!(
        "{mg}{bl}{footer_h}{rst}\n",
        mg = mg,
        bl = box_chars::BL,
        footer_h = footer_h,
        rst = rst,
    ));

    // Linha em branco após footer
    out.push('\n');

    out
}

/// Imprime `build_help` em stdout.
pub fn show_help(no_color: bool) {
    print!("{}", build_help(no_color));
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Converte uma fatia de literais em `Vec<String>` (helper de teste).
    fn args(v: &[&str]) -> Vec<String> {
        v.iter().map(|s| s.to_string()).collect()
    }

    // -----------------------------------------------------------------------
    // Defaults
    // -----------------------------------------------------------------------

    #[test]
    fn defaults_no_args() {
        let opts = parse_args(&args(&[])).unwrap();
        assert_eq!(opts.command, Command::Waybar);
        assert!(!opts.refresh);
        assert!(!opts.verbose);
        assert_eq!(opts.format, Format::Waybar);
        assert!(!opts.watch);
        assert_eq!(opts.interval_seconds, DEFAULT_INTERVAL_SECS);
        assert!(opts.warnings.is_empty());
    }

    // -----------------------------------------------------------------------
    // Commands
    // -----------------------------------------------------------------------

    #[test]
    fn command_menu() {
        assert_eq!(parse_args(&args(&["menu"])).unwrap().command, Command::Menu);
    }

    #[test]
    fn command_status() {
        assert_eq!(
            parse_args(&args(&["status"])).unwrap().command,
            Command::Status
        );
    }

    #[test]
    fn command_setup() {
        assert_eq!(
            parse_args(&args(&["setup"])).unwrap().command,
            Command::Setup
        );
    }

    #[test]
    fn command_assets_install() {
        assert_eq!(
            parse_args(&args(&["assets", "install"])).unwrap().command,
            Command::AssetsInstall
        );
    }

    #[test]
    fn command_export_waybar_modules() {
        assert_eq!(
            parse_args(&args(&["export", "waybar-modules"]))
                .unwrap()
                .command,
            Command::ExportWaybarModules
        );
    }

    #[test]
    fn command_export_waybar_css() {
        assert_eq!(
            parse_args(&args(&["export", "waybar-css"]))
                .unwrap()
                .command,
            Command::ExportWaybarCss
        );
    }

    #[test]
    fn command_update() {
        assert_eq!(
            parse_args(&args(&["update"])).unwrap().command,
            Command::Update
        );
    }

    #[test]
    fn command_uninstall() {
        assert_eq!(
            parse_args(&args(&["uninstall"])).unwrap().command,
            Command::Uninstall
        );
    }

    #[test]
    fn command_remove() {
        assert_eq!(
            parse_args(&args(&["remove"])).unwrap().command,
            Command::Remove
        );
    }

    #[test]
    fn command_doctor() {
        assert_eq!(
            parse_args(&args(&["doctor"])).unwrap().command,
            Command::Doctor
        );
    }

    #[test]
    fn menu_font_parses_as_command() {
        let opts = parse_args(&args(&["menu-font"])).unwrap();
        assert_eq!(opts.command, Command::MenuFont);
    }

    #[test]
    fn command_doctor_dry_run_yes() {
        let opts = parse_args(&args(&["doctor", "--dry-run", "--yes"])).unwrap();
        assert_eq!(opts.command, Command::Doctor);
        assert!(opts.dry_run);
        assert!(opts.yes);
    }

    #[test]
    fn setup_omarchy_plugins_dir_flag() {
        let opts = parse_args(&args(&["setup", "--omarchy-plugins-dir", "/tmp/x"])).unwrap();
        assert_eq!(opts.command, Command::Setup);
        assert_eq!(opts.omarchy_plugins_dir.as_deref(), Some("/tmp/x"));
    }

    #[test]
    fn omarchy_plugins_dir_requires_value() {
        assert!(parse_args(&args(&["setup", "--omarchy-plugins-dir"])).is_err());
    }

    #[test]
    fn omarchy_plugins_dir_defaults_none() {
        let opts = parse_args(&args(&["setup"])).unwrap();
        assert!(opts.omarchy_plugins_dir.is_none());
    }

    #[test]
    fn command_help_word() {
        assert_eq!(parse_args(&args(&["help"])).unwrap().command, Command::Help);
    }

    #[test]
    fn command_help_flag() {
        assert_eq!(
            parse_args(&args(&["--help"])).unwrap().command,
            Command::Help
        );
    }

    #[test]
    fn command_help_short() {
        assert_eq!(parse_args(&args(&["-h"])).unwrap().command, Command::Help);
    }

    #[test]
    fn command_action_right_with_provider() {
        let opts = parse_args(&args(&["action-right", "claude"])).unwrap();
        assert_eq!(opts.command, Command::ActionRight);
        assert_eq!(opts.provider.as_deref(), Some("claude"));
    }

    #[test]
    fn command_version_long() {
        assert_eq!(
            parse_args(&args(&["--version"])).unwrap().command,
            Command::Version
        );
    }

    #[test]
    fn command_version_short() {
        assert_eq!(
            parse_args(&args(&["-V"])).unwrap().command,
            Command::Version
        );
    }

    // -----------------------------------------------------------------------
    // Flags
    // -----------------------------------------------------------------------

    #[test]
    fn flag_refresh_long() {
        assert!(parse_args(&args(&["--refresh"])).unwrap().refresh);
    }

    #[test]
    fn flag_refresh_short() {
        assert!(parse_args(&args(&["-r"])).unwrap().refresh);
    }

    #[test]
    fn flag_verbose_long() {
        assert!(parse_args(&args(&["--verbose"])).unwrap().verbose);
    }

    #[test]
    fn flag_verbose_short() {
        assert!(parse_args(&args(&["-v"])).unwrap().verbose);
    }

    #[test]
    fn flag_terminal_long() {
        assert_eq!(
            parse_args(&args(&["--terminal"])).unwrap().command,
            Command::Terminal
        );
    }

    #[test]
    fn flag_terminal_short() {
        assert_eq!(
            parse_args(&args(&["-t"])).unwrap().command,
            Command::Terminal
        );
    }

    #[test]
    fn flag_provider_long() {
        let opts = parse_args(&args(&["--provider", "codex"])).unwrap();
        assert_eq!(opts.provider.as_deref(), Some("codex"));
    }

    #[test]
    fn flag_provider_short() {
        let opts = parse_args(&args(&["-p", "amp"])).unwrap();
        assert_eq!(opts.provider.as_deref(), Some("amp"));
    }

    #[test]
    fn flag_waybar_dir() {
        let opts = parse_args(&args(&["--waybar-dir", "/custom/path"])).unwrap();
        assert_eq!(opts.waybar_dir.as_deref(), Some("/custom/path"));
    }

    #[test]
    fn flag_scripts_dir() {
        let opts = parse_args(&args(&["--scripts-dir", "/scripts"])).unwrap();
        assert_eq!(opts.scripts_dir.as_deref(), Some("/scripts"));
    }

    #[test]
    fn flag_icons_dir() {
        let opts = parse_args(&args(&["--icons-dir", "/icons"])).unwrap();
        assert_eq!(opts.icons_dir.as_deref(), Some("/icons"));
    }

    #[test]
    fn flag_app_bin() {
        let opts = parse_args(&args(&["--app-bin", "/usr/bin/app"])).unwrap();
        assert_eq!(opts.app_bin.as_deref(), Some("/usr/bin/app"));
    }

    #[test]
    fn flag_terminal_script() {
        let opts = parse_args(&args(&["--terminal-script", "/bin/launch"])).unwrap();
        assert_eq!(opts.terminal_script.as_deref(), Some("/bin/launch"));
    }

    // -----------------------------------------------------------------------
    // Combinações
    // -----------------------------------------------------------------------

    #[test]
    fn combo_status_with_flags() {
        let opts =
            parse_args(&args(&["status", "--refresh", "--verbose", "-p", "claude"])).unwrap();
        assert_eq!(opts.command, Command::Status);
        assert!(opts.refresh);
        assert!(opts.verbose);
        assert_eq!(opts.provider.as_deref(), Some("claude"));
    }

    #[test]
    fn combo_flags_before_command() {
        let opts = parse_args(&args(&["-v", "-r", "menu"])).unwrap();
        assert_eq!(opts.command, Command::Menu);
        assert!(opts.verbose);
        assert!(opts.refresh);
    }

    // -----------------------------------------------------------------------
    // Unknown commands
    // -----------------------------------------------------------------------

    #[test]
    fn unknown_command_with_suggestion() {
        let err = parse_args(&args(&["setip"])).unwrap_err();
        assert!(
            err.message.contains("Did you mean 'setup'"),
            "mensagem: {}",
            err.message
        );
    }

    #[test]
    fn unknown_command_without_suggestion() {
        let err = parse_args(&args(&["xyzzy"])).unwrap_err();
        assert!(
            err.message.contains("Unknown command: xyzzy"),
            "mensagem: {}",
            err.message
        );
        assert!(
            err.message.contains("help"),
            "mensagem não contém 'help': {}",
            err.message
        );
    }

    #[test]
    fn subcommand_assets_missing_second_word() {
        let err = parse_args(&args(&["assets"])).unwrap_err();
        assert!(
            err.message.contains("assets install"),
            "mensagem: {}",
            err.message
        );
    }

    #[test]
    fn subcommand_export_missing_second_word() {
        let err = parse_args(&args(&["export"])).unwrap_err();
        assert!(
            err.message.contains("waybar-modules"),
            "mensagem: {}",
            err.message
        );
    }

    #[test]
    fn unknown_flag_does_not_error() {
        let opts = parse_args(&args(&["--unknown-flag"])).unwrap();
        assert_eq!(opts.command, Command::Waybar);
        assert!(
            opts.warnings
                .iter()
                .any(|w| w.contains("Unknown option: --unknown-flag")),
            "warnings: {:?}",
            opts.warnings
        );
    }

    // -----------------------------------------------------------------------
    // Output format flags
    // -----------------------------------------------------------------------

    #[test]
    fn format_defaults() {
        let opts = parse_args(&args(&[])).unwrap();
        assert_eq!(opts.format, Format::Waybar);
        assert!(!opts.watch);
        assert_eq!(opts.interval_seconds, 60);
    }

    #[test]
    fn format_json() {
        assert_eq!(
            parse_args(&args(&["--format", "json"])).unwrap().format,
            Format::Json
        );
    }

    #[test]
    fn watch_implies_json() {
        let opts = parse_args(&args(&["--watch"])).unwrap();
        assert!(opts.watch);
        assert_eq!(opts.format, Format::Json);
    }

    #[test]
    fn interval_with_watch() {
        let opts = parse_args(&args(&["--watch", "--interval", "30"])).unwrap();
        assert_eq!(opts.interval_seconds, 30);
    }

    #[test]
    fn invalid_format() {
        let err = parse_args(&args(&["--format", "xml"])).unwrap_err();
        assert!(
            err.message.contains("--format must be"),
            "mensagem: {}",
            err.message
        );
    }

    #[test]
    fn invalid_interval_abc() {
        let err = parse_args(&args(&["--watch", "--interval", "abc"])).unwrap_err();
        assert!(
            err.message.contains("--interval must be"),
            "mensagem: {}",
            err.message
        );
    }

    #[test]
    fn invalid_interval_float() {
        let err = parse_args(&args(&["--watch", "--interval", "1.5"])).unwrap_err();
        assert!(
            err.message.contains("--interval must be"),
            "mensagem: {}",
            err.message
        );
    }

    #[test]
    fn invalid_interval_zero() {
        let err = parse_args(&args(&["--watch", "--interval", "0"])).unwrap_err();
        assert!(
            err.message.contains("--interval must be"),
            "mensagem: {}",
            err.message
        );
    }

    #[test]
    fn watch_with_explicit_waybar_format_errors() {
        let err = parse_args(&args(&["--watch", "--format", "waybar"])).unwrap_err();
        assert!(
            err.message.contains("--watch requires --format json"),
            "mensagem: {}",
            err.message
        );
    }

    #[test]
    fn interval_without_watch_warns() {
        let opts = parse_args(&args(&["--interval", "30"])).unwrap();
        assert!(!opts.watch);
        assert!(
            opts.warnings
                .iter()
                .any(|w| w.contains("--interval has no effect without --watch")),
            "warnings: {:?}",
            opts.warnings
        );
    }

    // -----------------------------------------------------------------------
    // Levenshtein unit tests
    // -----------------------------------------------------------------------

    #[test]
    fn levenshtein_identical() {
        assert_eq!(levenshtein("setup", "setup"), 0);
    }

    #[test]
    fn levenshtein_one_typo() {
        assert_eq!(levenshtein("setip", "setup"), 1);
    }

    #[test]
    fn levenshtein_empty() {
        assert_eq!(levenshtein("", "abc"), 3);
        assert_eq!(levenshtein("abc", ""), 3);
    }

    #[test]
    fn suggest_command_returns_none_for_gibberish() {
        assert!(suggest_command("xyzzy").is_none());
    }

    #[test]
    fn suggest_command_returns_setup_for_setip() {
        assert_eq!(suggest_command("setip"), Some("setup"));
    }

    // -----------------------------------------------------------------------
    // build_help / show_help — porta de cli.test.ts:354-372
    // -----------------------------------------------------------------------

    /// Needle 1: texto verbatim da linha `update` (contrato de cli.test.ts:365).
    #[test]
    fn build_help_contains_update_description() {
        let help = build_help(false);
        assert!(
            help.contains("Update the install (self-update or managed checkout)"),
            "Texto 'Update the install (self-update or managed checkout)' não encontrado no help"
        );
    }

    /// Needle 2: linha Info — APP_NAME + dois espaços (contrato de cli.test.ts:369).
    #[test]
    fn build_help_contains_run_with_line() {
        let help = build_help(false);
        assert!(
            help.contains("agent-bar  or  cargo run"),
            "Texto 'agent-bar  or  cargo run' não encontrado no help"
        );
    }

    /// Com no_color=true os 2 needles de texto devem permanecer.
    #[test]
    fn build_help_no_color_preserves_text_needles() {
        let help = build_help(true);
        assert!(
            help.contains("Update the install (self-update or managed checkout)"),
            "no_color=true: texto 'Update the install (self-update or managed checkout)' ausente"
        );
        assert!(
            help.contains("agent-bar  or  cargo run"),
            "no_color=true: texto 'agent-bar  or  cargo run' ausente"
        );
    }

    /// Com no_color=true NÃO deve haver nenhum escape ANSI (ESC = \x1b).
    #[test]
    fn build_help_no_color_has_no_ansi_escapes() {
        let help = build_help(true);
        assert!(
            !help.contains('\x1b'),
            "no_color=true: escape ANSI encontrado no help"
        );
    }

    /// Com no_color=false o help DEVE conter escapes ANSI (sanidade).
    #[test]
    fn build_help_with_color_has_ansi_escapes() {
        let help = build_help(false);
        assert!(
            help.contains('\x1b'),
            "no_color=false: escape ANSI ausente — cores não foram emitidas"
        );
    }

    /// Deve terminar em `\n` (para que println! não adicione linha extra).
    #[test]
    fn build_help_ends_with_newline() {
        let help = build_help(false);
        assert!(help.ends_with('\n'), "build_help deve terminar em '\\n'");
    }
}
