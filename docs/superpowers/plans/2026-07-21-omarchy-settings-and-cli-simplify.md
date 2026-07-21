# Omarchy settings nativo + CLI simplify — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Right-click no plugin Omarchy abre settings nativo (mesmo popup); `config show`/`apply` como bridge para `settings.json`; help CLI limpo com internos escondidos e aliases — sem remover Waybar/TUI.

**Architecture:** Mini-API Rust (`src/config_cmd.rs`) serializa/valida o subset editável e grava via `settings::load`/`save`. O `Widget.qml` ganha `settingsMode` (padrão `omarchy.model-usage`), filtra chips por `waybar.providers`, e no Save faz dual-write (CLI + `updateEntryInline` só pro interval). Help público deixa de listar `action-right`/`assets`/`export`/`remove` como primários; parse desses paths permanece onde a spec exige.

**Tech Stack:** Rust (serde_json, tempfile, anyhow) + QML Quickshell (`Process`/`StdioCollector` já usados no widget; docs Quickshell Process+StdioCollector). Spec: `docs/superpowers/specs/2026-07-21-omarchy-settings-and-cli-simplify-design.md`.

**Evidência de baseline (não chutar):**

- `assets/omarchy/Widget.qml`: left popup, right `openTui()`, width `Style.space(300)`.
- `model-usage` local: right → `openSettings()`, width `Style.space(370)`, `updateEntryInline`.
- `settings::{load,save,normalize_provider_selection}` em `src/settings.rs`.
- `Command` + `build_help` em `src/cli.rs`; dispatch em `src/main.rs`.
- Plugin embutido: `include_str!("../assets/omarchy/Widget.qml")` em `omarchy_integration.rs`.

## Global Constraints

- **Rust/cargo only** — sem Node em runtime/teste.
- **Nunca mutar desktop vivo em teste** — temp dirs, `XDG_*`, `--omarchy-plugins-dir`.
- **stdout limpo** em poll/config show/apply sucesso = só JSON; logs em stderr.
- **Nunca `unwrap()`/`expect()` em produção** (ok em `#[cfg(test)]`).
- **TUI Config / `action_right.rs` / waybar modules: sem mudança funcional.**
- **Envelope quota `schemaVersion: 1` inalterado** — não filtrar providers no JSON de poll.
- **`config apply` NÃO chama `apply_waybar_integration`.**
- Identidade: `OMARCHY_PLUGIN_ID` etc. de `app_identity.rs`.
- Conventional Commits PT, subject ≤ 50 chars; zero atribuição de AI.
- Gotcha RTK: um filtro posicional por `cargo test`.
- Read arquivo antes de Edit; re-Read após outro agente.

## File map

| Path | Papel |
| --- | --- |
| `src/config_cmd.rs` | **Novo.** show envelope + apply patch/validate/save |
| `src/lib.rs` | `pub mod config_cmd` |
| `src/cli.rs` | `Command::ConfigShow` / `ConfigApply`; parse; help; aliases |
| `src/main.rs` | dispatch config |
| `assets/omarchy/Widget.qml` | settingsMode, filtro, largura, Save |
| `assets/omarchy/manifest.json` | description dos clicks |
| `docs/commands.md`, `omarchy-shell.md`, `architecture.md`, `README.md` | contrato |

## Ordem

```text
T1 config_cmd ──► T2 CLI parse+dispatch ──► T3 help+aliases
                                              │
                                              ▼
                                    T4 Widget.qml settings
                                              │
                                              ▼
                                         T5 docs ──► T6 gate
```

---

### Task 1: `config_cmd` — show / apply puros

**Files:**
- Create: `src/config_cmd.rs`
- Modify: `src/lib.rs` (adicionar `pub mod config_cmd;`)

**Interfaces:**
- Produces:
  - `pub const CONFIG_SCHEMA_VERSION: u32 = 1`
  - `pub struct ConfigView { schema_version, providers, provider_order, display_mode, notify_enabled }` com `Serialize`
  - `pub fn view_from_settings(s: &Settings) -> ConfigView`
  - `pub fn show(paths: &Paths) -> ConfigView` (= load + view)
  - `pub fn apply_json(paths: &Paths, raw: &str) -> Result<ConfigView, ApplyError>`
  - `pub enum ApplyError { Validation(String), Io(String) }` com `Display`
- Consumes: `settings::{load, save, normalize_provider_selection, DisplayMode, Settings}`, `config::Paths`

- [ ] **Step 1: Criar módulo com testes que falham (API `todo!`)**

```rust
//! `agent-bar config show|apply` — subset editável do settings.json (spec
//! 2026-07-21-omarchy-settings-and-cli-simplify).

use serde::{Deserialize, Serialize};

use crate::config::Paths;
use crate::settings::{
    normalize_provider_selection, DisplayMode, Settings,
};

pub const CONFIG_SCHEMA_VERSION: u32 = 1;

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConfigView {
    pub schema_version: u32,
    pub providers: Vec<String>,
    pub provider_order: Vec<String>,
    pub display_mode: DisplayMode,
    pub notify: NotifyView,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct NotifyView {
    pub enabled: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ApplyError {
    Validation(String),
    Io(String),
}

impl std::fmt::Display for ApplyError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ApplyError::Validation(m) | ApplyError::Io(m) => write!(f, "{m}"),
        }
    }
}

pub fn view_from_settings(s: &Settings) -> ConfigView {
    todo!("Task 1")
}

pub fn show(paths: &Paths) -> ConfigView {
    todo!("Task 1")
}

pub fn apply_json(paths: &Paths, raw: &str) -> Result<ConfigView, ApplyError> {
    todo!("Task 1")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Paths;
    use crate::settings::{self, DisplayMode};
    use std::path::PathBuf;
    use tempfile::tempdir;

    fn paths_in(dir: &std::path::Path) -> Paths {
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

    #[test]
    fn show_defaults_when_no_file() {
        let dir = tempdir().unwrap();
        let v = show(&paths_in(dir.path()));
        assert_eq!(v.schema_version, 1);
        assert_eq!(v.providers, vec!["claude", "codex", "amp", "grok"]);
        assert_eq!(v.provider_order, v.providers);
        assert_eq!(v.display_mode, DisplayMode::Remaining);
        assert!(v.notify.enabled);
    }

    #[test]
    fn apply_rejects_wrong_schema() {
        let dir = tempdir().unwrap();
        let p = paths_in(dir.path());
        let err = apply_json(&p, r#"{"schemaVersion":2,"providers":["claude"]}"#).unwrap_err();
        assert!(matches!(err, ApplyError::Validation(_)));
    }

    #[test]
    fn apply_rejects_empty_providers() {
        let dir = tempdir().unwrap();
        let p = paths_in(dir.path());
        let err = apply_json(&p, r#"{"schemaVersion":1,"providers":[]}"#).unwrap_err();
        assert!(matches!(err, ApplyError::Validation(_)));
    }

    #[test]
    fn apply_partial_preserves_omitted_fields() {
        let dir = tempdir().unwrap();
        let p = paths_in(dir.path());
        // seed
        let mut s = settings::load(&p);
        s.waybar.display_mode = DisplayMode::Used;
        s.notify.enabled = false;
        settings::save(&p, &s).unwrap();

        let v = apply_json(
            &p,
            r#"{"schemaVersion":1,"providers":["claude","codex"]}"#,
        )
        .unwrap();
        assert_eq!(v.providers, vec!["claude", "codex"]);
        assert_eq!(v.display_mode, DisplayMode::Used);
        assert!(!v.notify.enabled);

        let loaded = settings::load(&p);
        assert_eq!(loaded.waybar.providers, vec!["claude", "codex"]);
        assert_eq!(loaded.waybar.display_mode, DisplayMode::Used);
        assert!(!loaded.notify.enabled);
        // separators etc. intact
        assert_eq!(loaded.waybar.separators, s.waybar.separators);
    }

    #[test]
    fn apply_display_mode_and_notify() {
        let dir = tempdir().unwrap();
        let p = paths_in(dir.path());
        let v = apply_json(
            &p,
            r#"{"schemaVersion":1,"displayMode":"used","notify":{"enabled":false}}"#,
        )
        .unwrap();
        assert_eq!(v.display_mode, DisplayMode::Used);
        assert!(!v.notify.enabled);
    }

    #[test]
    fn apply_rejects_invalid_display_mode() {
        let dir = tempdir().unwrap();
        let p = paths_in(dir.path());
        let err = apply_json(&p, r#"{"schemaVersion":1,"displayMode":"nope"}"#).unwrap_err();
        assert!(matches!(err, ApplyError::Validation(_)));
    }

    #[test]
    fn apply_drops_unknown_provider_ids() {
        let dir = tempdir().unwrap();
        let p = paths_in(dir.path());
        let v = apply_json(
            &p,
            r#"{"schemaVersion":1,"providers":["claude","nope","codex"]}"#,
        )
        .unwrap();
        assert_eq!(v.providers, vec!["claude", "codex"]);
    }

    #[test]
    fn apply_all_unknown_providers_is_error() {
        let dir = tempdir().unwrap();
        let p = paths_in(dir.path());
        let err =
            apply_json(&p, r#"{"schemaVersion":1,"providers":["nope"]}"#).unwrap_err();
        assert!(matches!(err, ApplyError::Validation(_)));
    }
}
```

Em `src/lib.rs`, após `pub mod config;`:

```rust
pub mod config_cmd;
```

- [ ] **Step 2: Rodar testes — devem falhar em `todo!`**

Run: `cargo test config_cmd`

Expected: FAIL / panic `not yet implemented` (ou compile error se DisplayMode não serializa camelCase — ver Step 3).

- [ ] **Step 3: Implementar**

Notas de serialização (evidência `settings.rs`):

- `DisplayMode` já tem `#[serde(rename_all = "lowercase")]` no Serialize do settings — reusar o enum no `ConfigView` (precisa `Deserialize` para o patch). Se `DisplayMode` não implementa `Deserialize`, adicionar `#[derive(Deserialize)]` **só se já tiver Serialize** — checar e estender derives no enum existente sem mudar wire format.

Implementação de referência:

```rust
#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ConfigPatch {
    schema_version: Option<u32>,
    providers: Option<Vec<String>>,
    provider_order: Option<Vec<String>>,
    display_mode: Option<String>,
    notify: Option<NotifyPatch>,
}

#[derive(Debug, Default, Deserialize)]
struct NotifyPatch {
    enabled: Option<bool>,
}

pub fn view_from_settings(s: &Settings) -> ConfigView {
    ConfigView {
        schema_version: CONFIG_SCHEMA_VERSION,
        providers: s.waybar.providers.clone(),
        provider_order: s.waybar.provider_order.clone(),
        display_mode: s.waybar.display_mode,
        notify: NotifyView {
            enabled: s.notify.enabled,
        },
    }
}

pub fn show(paths: &Paths) -> ConfigView {
    view_from_settings(&crate::settings::load(paths))
}

pub fn apply_json(paths: &Paths, raw: &str) -> Result<ConfigView, ApplyError> {
    let patch: ConfigPatch = serde_json::from_str(raw).map_err(|e| {
        ApplyError::Validation(format!("invalid JSON: {e}"))
    })?;

    match patch.schema_version {
        Some(CONFIG_SCHEMA_VERSION) => {}
        Some(v) => {
            return Err(ApplyError::Validation(format!(
                "unsupported schemaVersion: {v} (expected {CONFIG_SCHEMA_VERSION})"
            )));
        }
        None => {
            return Err(ApplyError::Validation(
                "schemaVersion is required".into(),
            ));
        }
    }

    let mut s = crate::settings::load(paths);

    if let Some(providers) = patch.providers {
        let order_src = patch
            .provider_order
            .clone()
            .unwrap_or_else(|| s.waybar.provider_order.clone());
        let (providers, order) = normalize_provider_selection(&providers, &order_src);
        if providers.is_empty() {
            return Err(ApplyError::Validation(
                "providers must contain at least one known id".into(),
            ));
        }
        s.waybar.providers = providers;
        s.waybar.provider_order = order;
    } else if let Some(order) = patch.provider_order {
        let (providers, order) =
            normalize_provider_selection(&s.waybar.providers, &order);
        s.waybar.providers = providers;
        s.waybar.provider_order = order;
    }

    if let Some(mode) = patch.display_mode {
        s.waybar.display_mode = match mode.as_str() {
            "remaining" => DisplayMode::Remaining,
            "used" => DisplayMode::Used,
            other => {
                return Err(ApplyError::Validation(format!(
                    "invalid displayMode: '{other}' (remaining|used)"
                )));
            }
        };
    }

    if let Some(n) = patch.notify {
        if let Some(en) = n.enabled {
            s.notify.enabled = en;
        }
    }

    crate::settings::save(paths, &s).map_err(|e| ApplyError::Io(e.to_string()))?;
    Ok(view_from_settings(&s))
}
```

Se `DisplayMode` no Serialize do view emitir `Remaining` em vez de `remaining`, forçar:

```rust
// no ConfigView, em vez de DisplayMode cru:
// display_mode: String  com "remaining"|"used"
```

**Preferência da spec:** JSON com `"displayMode":"remaining"`. Garantir com teste:

```rust
#[test]
fn show_json_wire_format() {
    let dir = tempdir().unwrap();
    let v = show(&paths_in(dir.path()));
    let j = serde_json::to_value(&v).unwrap();
    assert_eq!(j["schemaVersion"], 1);
    assert_eq!(j["displayMode"], "remaining");
    assert_eq!(j["notify"]["enabled"], true);
    assert!(j["providers"].is_array());
}
```

Ajustar derives/`serialize_with` até o wire bater.

- [ ] **Step 4: Testes verdes**

Run: `cargo test config_cmd`

Expected: all PASS.

- [ ] **Step 5: Commit**

```bash
git add src/config_cmd.rs src/lib.rs src/settings.rs
git commit -m "feat: config show/apply subset settings"
```

---

### Task 2: CLI parse + dispatch `config`

**Files:**
- Modify: `src/cli.rs` (`Command`, `CliOptions`, `parse_args`, `KNOWN_COMMANDS`)
- Modify: `src/main.rs` (dispatch antes do path de poll)

**Interfaces:**
- Produces: `Command::ConfigShow`, `Command::ConfigApply`; em `CliOptions`:
  - `config_json: Option<String>` (literal ou conteúdo de `--file`)
  - ou flag `config_json_stdin: bool`
- Parse:
  - `config show` → ConfigShow
  - `config apply --json '...'` → ConfigApply
  - `config apply --json -` → lê stdin no **main** (parse só marca stdin)
  - `config apply --file path` → main lê file
  - `config` / `config --help` / `config help` → help de config ou help geral com seção config

- [ ] **Step 1: Testes de parse (em `cli.rs` mod tests)**

```rust
    #[test]
    fn command_config_show() {
        let opts = parse_args(&args(&["config", "show"])).unwrap();
        assert_eq!(opts.command, Command::ConfigShow);
    }

    #[test]
    fn command_config_apply_json() {
        let opts = parse_args(&args(&[
            "config",
            "apply",
            "--json",
            r#"{"schemaVersion":1}"#,
        ]))
        .unwrap();
        assert_eq!(opts.command, Command::ConfigApply);
        assert_eq!(
            opts.config_json.as_deref(),
            Some(r#"{"schemaVersion":1}"#)
        );
    }

    #[test]
    fn command_config_apply_requires_payload() {
        let err = parse_args(&args(&["config", "apply"])).unwrap_err();
        assert!(err.message.contains("--json") || err.message.contains("--file"));
    }
```

Adicionar em `CliOptions` / `Default`:

```rust
    /// Payload de `config apply --json` (None se `--json -` ou `--file`).
    pub config_json: Option<String>,
    pub config_json_stdin: bool,
    pub config_file: Option<String>,
```

- [ ] **Step 2: Rodar — falha (Command inexistente)**

Run: `cargo test cli command_config`

Expected: compile error / FAIL.

- [ ] **Step 3: Implementar parse + enum + main**

Em `Command`:

```rust
    ConfigShow,
    ConfigApply,
```

Em `parse_args`, ramo:

```rust
            "config" => {
                match args.get(i + 1).map(|s| s.as_str()) {
                    Some("show") => {
                        opts.command = Command::ConfigShow;
                        i += 1;
                    }
                    Some("apply") => {
                        opts.command = Command::ConfigApply;
                        i += 1;
                        // consumir --json / --file no loop principal OU aqui
                    }
                    Some("--help" | "help" | "-h") | None => {
                        opts.command = Command::Help; // ou help dedicado
                        if args.get(i + 1).is_some() { i += 1; }
                    }
                    Some(other) => {
                        return Err(CliError {
                            message: format!(
                                "Unknown subcommand for 'config': {other}. Use 'show' or 'apply'."
                            ),
                        });
                    }
                }
            }
```

Para `--json` / `--file` **dentro de apply**, o loop atual já processa flags genéricas — estender:

```rust
            "--json" => {
                let val = require_next_arg(args, i, "--json")?;
                if val == "-" {
                    opts.config_json_stdin = true;
                } else {
                    opts.config_json = Some(val.to_string());
                }
                i += 1;
            }
            "--file" => {
                let val = require_next_arg(args, i, "--file")?;
                opts.config_file = Some(val.to_string());
                i += 1;
            }
```

Validar no fim do parse se `ConfigApply` tem exatamente uma fonte (json literal | stdin | file).

`KNOWN_COMMANDS`: incluir `"config"` (typo suggest).

Em `main.rs`, no bloco de subcommands early-exit (junto de Setup/Doctor), **antes** do poll:

```rust
        Command::ConfigShow => {
            let paths = Paths::from_env().unwrap_or_else(|e| {
                log::error!("{e}");
                std::process::exit(1);
            });
            let view = agent_bar::config_cmd::show(&paths);
            match serde_json::to_string_pretty(&view) {
                Ok(j) => {
                    println!("{j}");
                    std::process::exit(0);
                }
                Err(e) => {
                    log::error!("{e}");
                    std::process::exit(1);
                }
            }
        }
        Command::ConfigApply => {
            let paths = Paths::from_env().unwrap_or_else(|e| {
                log::error!("{e}");
                std::process::exit(1);
            });
            let raw = if opts.config_json_stdin {
                use std::io::Read;
                let mut buf = String::new();
                if let Err(e) = std::io::stdin().read_to_string(&mut buf) {
                    log::error!("failed to read stdin: {e}");
                    std::process::exit(2);
                }
                buf
            } else if let Some(path) = &opts.config_file {
                match std::fs::read_to_string(path) {
                    Ok(s) => s,
                    Err(e) => {
                        log::error!("failed to read {path}: {e}");
                        std::process::exit(1);
                    }
                }
            } else if let Some(j) = &opts.config_json {
                j.clone()
            } else {
                log::error!("config apply requires --json <blob>, --json -, or --file <path>");
                std::process::exit(2);
            };
            match agent_bar::config_cmd::apply_json(&paths, &raw) {
                Ok(view) => match serde_json::to_string_pretty(&view) {
                    Ok(j) => {
                        println!("{j}");
                        std::process::exit(0);
                    }
                    Err(e) => {
                        log::error!("{e}");
                        std::process::exit(1);
                    }
                },
                Err(agent_bar::config_cmd::ApplyError::Validation(m)) => {
                    eprintln!("{m}");
                    std::process::exit(1);
                }
                Err(agent_bar::config_cmd::ApplyError::Io(m)) => {
                    eprintln!("{m}");
                    std::process::exit(1);
                }
            }
        }
```

(Usar `?`/match sem unwrap em prod.)

Smoke manual (temp XDG):

```bash
XDG_CONFIG_HOME=/tmp/ab-cfg-test cargo run -- config show
XDG_CONFIG_HOME=/tmp/ab-cfg-test cargo run -- config apply --json '{"schemaVersion":1,"providers":["claude"]}'
```

- [ ] **Step 4: Testes**

Run: `cargo test cli`

Expected: PASS (incluir novos testes).

- [ ] **Step 5: Commit**

```bash
git add src/cli.rs src/main.rs
git commit -m "feat: CLI config show e apply"
```

---

### Task 3: Help limpo + aliases (`-t`, `remove`)

**Files:**
- Modify: `src/cli.rs` (`build_help`, `parse_args`, `KNOWN_COMMANDS`, testes help)
- Modify: `src/main.rs` só se `Command::Terminal` / `Remove` mudarem de semântica

**Spec §4:**

- Help lista: menu, status, config show|apply, setup, update, uninstall, doctor + flags machine.
- **Não** lista: action-right, menu-font, assets, export, remove (remove vira alias).
- `-t` / `--terminal` → `Command::Status` (não `Terminal`).
- `remove` → `Command::Uninstall` + `yes = true` (force path). Preferir **não** manter `Command::Remove` no parse se main puder unificar; se `Remove` ainda existir no enum por testes, mapear parse de `remove` para Uninstall+yes e atualizar testes.

- [ ] **Step 1: Testes**

```rust
    #[test]
    fn terminal_flag_aliases_status() {
        let opts = parse_args(&args(&["--terminal"])).unwrap();
        assert_eq!(opts.command, Command::Status);
        let opts = parse_args(&args(&["-t"])).unwrap();
        assert_eq!(opts.command, Command::Status);
    }

    #[test]
    fn remove_aliases_uninstall_yes() {
        let opts = parse_args(&args(&["remove"])).unwrap();
        assert_eq!(opts.command, Command::Uninstall);
        assert!(opts.yes);
    }

    #[test]
    fn build_help_hides_internals() {
        let help = build_help(true);
        assert!(help.contains("config"));
        assert!(help.contains("menu"));
        assert!(help.contains("status"));
        assert!(!help.contains("action-right"));
        assert!(!help.contains("assets install"));
        assert!(!help.contains("export waybar-modules"));
        assert!(!help.contains("menu-font"));
        // remove não como comando de vitrine
        assert!(
            !help.lines().any(|l| l.contains("remove") && l.contains("Force")),
            "remove não deve aparecer como comando primário"
        );
    }

    #[test]
    fn action_right_still_parses() {
        let opts = parse_args(&args(&["action-right", "claude"])).unwrap();
        assert_eq!(opts.command, Command::ActionRight);
    }
```

Atualizar testes antigos que esperam `Command::Terminal` ou `Command::Remove` no parse.

- [ ] **Step 2: FAIL nos asserts de help/alias**

Run: `cargo test cli`

- [ ] **Step 3: Reescrever `build_help` (grupos)**

Estrutura alvo (texto; manter box-drawing existente):

```text
Commands
  menu / status / config show / config apply / setup / update / uninstall / doctor

Omarchy / Waybar (resumo curto)
  default poll = Waybar JSON; --format json = shell
  Omarchy: left usage, right settings, middle refresh
  Waybar: left menu, right action-right (internal)

Flags
  --provider, --refresh, --format, --watch, --interval, --verbose, …
```

Remover `cmd_line` de assets/export/remove. Adicionar config. Ajustar seção Waybar desatualizada (“Right click Refresh/Login” → TUI focada / internos).

Parse:

```rust
            "--terminal" | "-t" => opts.command = Command::Status,

            "remove" => {
                opts.command = Command::Uninstall;
                opts.yes = true;
            }
```

Main: se `Uninstall` + `yes`, chamar `run_uninstall(..., force=true)` — **ler** `uninstall::run_uninstall` e o branch atual de `Remove`/`Uninstall` para unificar. Hoje `Remove` passa `true` e `Uninstall` passa `false`. Se `opts.yes` no Uninstall interativo ainda não força, mapear: `force = opts.yes` no call de Uninstall e deletar branch `Command::Remove` **ou** manter Remove só no enum unused.

`KNOWN_COMMANDS`: tirar `"assets"`, `"export"`, `"action-right"` da lista de typo **ou** manter action-right para suggest se user digitar mal — spec: internos parseáveis; typo suggest de action-right ainda útil. Manter `action-right` em KNOWN_COMMANDS; remover assets/export se quiser help mental limpo.

- [ ] **Step 4: PASS**

Run: `cargo test cli`

- [ ] **Step 5: Commit**

```bash
git add src/cli.rs src/main.rs
git commit -m "refactor: help CLI e aliases remove/-t"
```

---

### Task 4: Widget.qml — settingsMode + filtro + largura

**Files:**
- Modify: `assets/omarchy/Widget.qml` (fonte embutida; setup reescreve drop-in)
- Modify: `assets/omarchy/manifest.json` (description dos clicks)
- Modify: snapshot `src/snapshots/agent_bar__omarchy_integration__tests__omarchy_manifest.snap` via teste

**Referência obrigatória (ler no disco antes de editar):**

- `/usr/share/omarchy/shell/plugins/model-usage/Widget.qml` — `openSettings`, `saveSettings`, `settingsMode`, `SettingsContent`, `updateEntryInline`
- Quickshell: `Process` + `StdioCollector` `onStreamFinished` (já no widget)

**Não há cargo test de QML.** Verificação: asserts de string no `omarchy_integration` se existirem; senão smoke manual Task 6. Atualizar snapshot do manifest.

- [ ] **Step 1: Atualizar manifest description**

Em `assets/omarchy/manifest.json`:

```json
"description": "Quota chips per provider; left = usage, right = settings, middle = refresh.",
```

(e o campo `barWidget.description` igual.)

- [ ] **Step 2: Rodar snapshot**

Run: `cargo test omarchy_integration`

Expected: snapshot fail se description mudou → `INSTA_UPDATE=auto cargo test omarchy_integration` **só** após conferir o snap.

- [ ] **Step 3: Estado e helpers no root do Widget**

Adicionar properties (junto das existentes):

```qml
  property bool settingsMode: false
  property var draftSettings: ({
    providers: [],
    providerOrder: [],
    displayMode: "remaining",
    notifyEnabled: true,
    refreshIntervalSec: 60
  })
  property var enabledIds: null          // null = sem filtro ainda
  property string displayMode: "remaining"
  property string settingsStatusText: ""
  property bool settingsBusy: false
```

`close()`:

```qml
  function close() {
    popupOpen = false
    settingsMode = false
    settingsBusy = false
  }
```

Largura/altura popup:

```qml
    contentWidth: Style.space(370)
    contentHeight: Math.min(popupCol.implicitHeight + Style.space(20), Style.space(560))
```

- [ ] **Step 4: Processes config show / apply**

```qml
  Process {
    id: configShowProc
    command: ["bash", "-lc", "agent-bar config show --format json"]
    stdout: StdioCollector {
      waitForEnd: true
      onStreamFinished: root.onConfigShowFinished(text)
    }
  }

  Process {
    id: configApplyProc
    property string payload: ""
    command: ["bash", "-lc", "agent-bar config apply --json " + Util.shellQuote(payload)]
    stdout: StdioCollector {
      waitForEnd: true
      onStreamFinished: root.onConfigApplyFinished(text)
    }
    // capturar falha: se Process expõe exit code no shell, preferir;
    // senão parse JSON — se inválido, settingsStatusText = "apply failed"
  }
```

**Nota:** passar JSON via argv com `shellQuote` é frágil se o blob tiver aspas. Preferir:

```qml
    command: ["bash", "-lc", "agent-bar config apply --json -"]
    stdin: payload   // se Process do Quickshell suportar stdin
```

Se `stdin` não existir no Process do Omarchy 4.0.0.alpha, usar tempfile:

```qml
    command: ["bash", "-lc",
      "f=$(mktemp) && cat >\"$f\" <<'AGENT_BAR_JSON'\n" + payload + "\nAGENT_BAR_JSON\nagent-bar config apply --file \"$f\"; e=$?; rm -f \"$f\"; exit $e"
    ]
```

Validar no desktop (Task 6) qual caminho funciona; **preferir `--file` via mktemp** se stdin indisponível (evidenciar no commit message).

Implementar:

```qml
  function applyConfigView(data) {
    if (!data || data.schemaVersion !== 1) return false
    enabledIds = Array.isArray(data.providerOrder) ? data.providerOrder.slice()
              : (Array.isArray(data.providers) ? data.providers.slice() : [])
    displayMode = data.displayMode === "used" ? "used" : "remaining"
    return true
  }

  function onConfigShowFinished(text) {
    try {
      var data = JSON.parse(String(text || ""))
      if (!applyConfigView(data)) throw new Error("bad config")
      draftSettings = {
        providers: (data.providers || []).slice(),
        providerOrder: (data.providerOrder || data.providers || []).slice(),
        displayMode: displayMode,
        notifyEnabled: !!(data.notify && data.notify.enabled),
        refreshIntervalSec: Math.min(3600, Math.max(30, Number(setting("refreshIntervalSec", 60)) || 60))
      }
      settingsStatusText = ""
    } catch (e) {
      settingsStatusText = "failed to load settings"
    }
  }

  function openSettings() {
    settingsMode = true
    popupOpen = true
    settingsStatusText = ""
    if (!configShowProc.running) configShowProc.running = true
  }

  function showUsage() {
    settingsMode = false
    settingsStatusText = ""
  }

  function visibleProviders() {
    var all = root.providers
    if (!enabledIds || !enabledIds.length) return all
    var out = []
    for (var i = 0; i < enabledIds.length; i++) {
      var id = enabledIds[i]
      for (var j = 0; j < all.length; j++) {
        if (all[j] && all[j].provider === id) { out.push(all[j]); break }
      }
    }
    return out
  }

  function chipLabel(p) {
    if (!p.available) return "–"
    var w = p.primary
    if (!w) return ""
    if (root.displayMode === "used") {
      var used = Number(w.used)
      if (isFinite(used)) return Math.round(used) + "%"
      var rem = Number(w.remaining)
      if (isFinite(rem)) return Math.round(100 - rem) + "%"
      return ""
    }
    if (!isFinite(Number(w.remaining))) return ""
    return Math.round(Number(w.remaining)) + "%"
  }

  function saveSettings() {
    if (settingsBusy) return
    var prov = draftSettings.providers || []
    if (!prov.length) {
      settingsStatusText = "Keep at least one provider"
      return
    }
    settingsBusy = true
    settingsStatusText = "Saving…"
    var blob = JSON.stringify({
      schemaVersion: 1,
      providers: prov,
      providerOrder: draftSettings.providerOrder || prov,
      displayMode: draftSettings.displayMode === "used" ? "used" : "remaining",
      notify: { enabled: !!draftSettings.notifyEnabled }
    })
    // disparar apply (ver path file/json acima)
    root._pendingApplyBlob = blob
    root.runConfigApply(blob)
  }

  function onConfigApplyFinished(text) {
    settingsBusy = false
    try {
      var data = JSON.parse(String(text || ""))
      if (!applyConfigView(data)) throw new Error("bad apply result")
    } catch (e) {
      settingsStatusText = "apply failed"
      return
    }
    // interval → shell.json
    var interval = Math.min(3600, Math.max(30, Number(draftSettings.refreshIntervalSec) || 60))
    if (bar && bar.shell && typeof bar.shell.updateEntryInline === "function") {
      try {
        var next = Object.assign({}, root.settings || {}, { refreshIntervalSec: interval })
        bar.shell.updateEntryInline(root.moduleName, next)
      } catch (e2) {
        settingsStatusText = "settings saved; interval not persisted"
        root.refresh(true)
        return
      }
    }
    settingsStatusText = "Saved"
    root.refresh(true)
  }

  Component.onCompleted: {
    if (!configShowProc.running) configShowProc.running = true
  }
```

- [ ] **Step 5: Mouse + popup body**

Right-click:

```qml
            if (mouse.button === Qt.RightButton) root.openSettings()
            else if (mouse.button === Qt.MiddleButton) root.refresh(true)
            else {
              root.showUsage()
              root.popupOpen = !root.popupOpen
            }
```

Repeater de chips e seções usage: `model: root.visibleProviders()` (não `root.providers` cru).

Popup column: se `settingsMode`, mostrar `SettingsHeader` + `SettingsContent`; senão usage atual + footer novo.

Footer usage:

```qml
        Text {
          visible: !root.settingsMode
          text: "right-click: settings · middle: refresh"
          // …
        }
        Text {
          visible: !root.settingsMode
          text: "Abrir menu (TUI)"
          color: Color.accent
          // MouseArea → root.openTui()
        }
```

Settings UI (padrão model-usage, denso):

- Header: `← Usage` | Settings | `Save`
- Section Providers: 4 toggles + ↑↓
- Display: Remaining | Used (dois botões, selected = accent)
- Alerts: notify toggle
- Refresh: stepper ±30 nos bounds
- Status + hint `s saves · esc closes`

Toggles: implementar `component Toggle` mínimo (label + switch visual com Rectangle) se `qs.Ui` não exportar o mesmo Toggle do model-usage — **copiar o pattern visual do model-usage**, não inventar glass.

Ordem ↑↓: só entre ids em `draftSettings.providers`; disabled known ids aparecem off abaixo.

Não desligar o último:

```qml
  function setProviderEnabled(id, on) {
    var p = (draftSettings.providers || []).slice()
    var idx = p.indexOf(id)
    if (on && idx < 0) p.push(id)
    if (!on) {
      if (p.length <= 1 && idx >= 0) {
        settingsStatusText = "Keep at least one provider"
        return
      }
      if (idx >= 0) p.splice(idx, 1)
    }
    // rebuild providerOrder: enabled order first
    var order = (draftSettings.providerOrder || []).filter(function(x) { return p.indexOf(x) >= 0 })
    p.forEach(function(x) { if (order.indexOf(x) < 0) order.push(x) })
    draftSettings = Object.assign({}, draftSettings, { providers: p, providerOrder: order })
  }
```

- [ ] **Step 6: Commit assets**

```bash
git add assets/omarchy/Widget.qml assets/omarchy/manifest.json src/snapshots/
git commit -m "feat: settings nativo no popup Omarchy"
```

---

### Task 5: Docs

**Files:**
- Modify: `docs/commands.md`
- Modify: `docs/omarchy-shell.md`
- Modify: `docs/architecture.md` (dispatch `config`; action-right ainda Waybar)
- Modify: `README.md` (tabela Omarchy clicks)

**Não** editar `CHANGELOG.md` (só no release).

- [ ] **Step 1: Reescrever seções**

`docs/omarchy-shell.md` — substituir right-click TUI por settings; documentar dual-write; filtro `waybar.providers`; link TUI; trade-off apply ≠ Waybar reload.

`docs/commands.md` — taxonomia Usage / Setup / Machine / Internal; `config show|apply` com schema; action-right/menu-font/assets/export em Internal; `remove` = alias uninstall -y; `-t` = status.

`docs/architecture.md` — uma linha no module map para `config_cmd.rs`.

`README.md` — Omarchy 4: left usage, right settings, middle refresh; `agent-bar menu` para dashboard.

- [ ] **Step 2: `git diff --check`**

- [ ] **Step 3: Commit**

```bash
git add docs/commands.md docs/omarchy-shell.md docs/architecture.md README.md
git commit -m "docs: config Omarchy e CLI simplify"
```

---

### Task 6: Gate de verificação

**Files:** nenhum de produto (só prova).

- [ ] **Step 1: Testes automatizados**

```bash
cargo test config_cmd
cargo test cli
cargo test settings
cargo test omarchy_integration
cargo clippy --all-targets -- -D warnings
```

Expected: PASS, clippy clean.

- [ ] **Step 2: Smoke CLI com XDG temp**

```bash
export XDG_CONFIG_HOME=/tmp/agent-bar-gate-$$
export XDG_CACHE_HOME=/tmp/agent-bar-gate-cache-$$
mkdir -p "$XDG_CONFIG_HOME" "$XDG_CACHE_HOME"
cargo run --quiet -- config show | head
cargo run --quiet -- config apply --json '{"schemaVersion":1,"providers":["claude","codex"],"displayMode":"used","notify":{"enabled":false}}'
cargo run --quiet -- config show
# help não lista internos
cargo run --quiet -- help | tee /tmp/ab-help.txt
rg -n "action-right|assets install|export waybar" /tmp/ab-help.txt && exit 1 || true
rg -n "config" /tmp/ab-help.txt
```

- [ ] **Step 3: Manual desktop Omarchy (3 provas)** — **pedir aprovação** antes de `agent-bar setup` se for reescrever plugin live.

1. **Funcional:** right-click → settings; toggle provider; Save; chip some/volta; refresh interval; notify; link TUI abre menu.
2. **Perceptual:** largura ~model-usage; settings denso; sem decoração.
3. **Dados:** `config show` reflete toggles; `settings.json` coerente; `shell.json` entry com `refreshIntervalSec`.

Após mudar QML embutido: `agent-bar setup` (com OK do user) ou copiar `Widget.qml` pro drop-in de teste.

- [ ] **Step 4: Commit vazio não** — se só prova, sem commit. Se fix de bug do gate, commit focado.

---

## Spec coverage (self-review do plano)

| Spec | Task |
| --- | --- |
| config show/apply schema + regras | T1–T2 |
| apply sem Waybar reload | T1 (save only) |
| Help + hide + aliases | T3 |
| settingsMode, width 370, dual-write | T4 |
| filtro chips + displayMode label | T4 |
| link TUI, middle refresh | T4 |
| TUI Config intocada | (nenhuma task toca `tui/`) |
| action-right parse preservado | T3 teste |
| Docs | T5 |
| 3 provas manuais | T6 |

**Placeholders:** nenhum TBD de requisito; path stdin vs mktemp do apply no QML tem fallback explícito a validar no desktop.

**Tipos:** `ConfigView` / `ApplyError` / `Command::ConfigShow|ConfigApply` consistentes T1→T2→T4.
