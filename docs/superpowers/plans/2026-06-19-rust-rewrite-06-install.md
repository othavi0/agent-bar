# Plano 06 — Install (waybar-integration, contract, setup/uninstall/update/doctor)

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) ou superpowers:executing-plans para implementar task-by-task. Steps usam checkbox (`- [ ]`).

**Goal:** Portar os comandos de instalação do agent-bar (TS → Rust): integração cirúrgica do Waybar config/style (JSONC), export de módulos/CSS, asset resolution, `setup`/`uninstall`/`remove`/`update`/`doctor`, substituindo os 8 stubs de `main.rs` por implementações reais.

**Architecture:** Camadas puras-e-testáveis (export/normalize/scan/detect/patch) separadas dos shells interativos finos (setup/uninstall/update/doctor). O patcher de JSONC é um **scanner de string cirúrgico** (NÃO um crate de JSONC — comentários/ordem do `.jsonc` do usuário precisam sobreviver). Os comandos scriptáveis (`assets-install`, `export-waybar-modules`, `export-waybar-css`) emitem **JSON byte-exact** em stdout (consumidos por `install.sh`/Quickshell). Os comandos interativos usam um **helper de prompt mínimo** (confirm via stdin + linhas de status temáticas), NÃO replicam o rendering do `@clack/prompts` — a superfície interativa rica (menu/login/dashboard) é a TUI ratatui do **Plano 7**.

**Tech Stack:** Rust 1.95, `std::fs`/`std::process`, `regex` (já dep), `serde_json` (pretty 2-espaços = `JSON.stringify(x,null,2)`; compacto = `JSON.stringify(x)`), `tempfile` (já dep, testes). Sem deps novas.

## Global Constraints

- **Autoridade = TS-fonte na raiz** (`src/waybar-integration.ts`, `src/waybar-contract.ts`, `src/doctor.ts`, `src/update.ts`, `src/setup.ts`, `src/uninstall.ts`, `src/remove.ts`, `src/app-identity.ts`, `src/runtime.ts`). "Rust == comportamento do TS". Testes `tests/*.test.ts` são o contrato a portar.
- **EXCEÇÃO de ordem de providers (Plano 5, decisão do usuário):** o agregado Waybar usa ordem de registro `[claude,amp,codex]`. **NÃO** aplica a este plano** diretamente** — os comandos de install usam `settings.waybar.provider_order` (normalizado), que é um campo distinto e segue o TS fielmente (vide `normalize_provider_selection`). Não confundir.
- **JSON stdout byte-exact:** `assets-install` = compacto (`serde_json::to_string`); `export-waybar-modules`/`export-waybar-css` = pretty 2-espaços (`serde_json::to_string_pretty`). Ordem de chaves do módulo é contrato: `exec`, `return-type`, `interval`, `exec-on-event`, `tooltip`, `on-click`, `on-click-right`, `signal?` (preservada por struct serde + `IndexMap` no map de módulos).
- **stdout limpo:** payload JSON e a view de comandos terminal vão para stdout via `println!`; diagnósticos vão para stderr (`log::*`/`eprintln!`). Prompts interativos escrevem em **stderr** (stdout fica reservado a payload scriptável) — exceto a view final de status que pode ir a stdout.
- **Sem `unwrap()`/`expect()` em produção** (enforçado pelo `deny` no `lib.rs`/`main.rs`). Em teste é permitido.
- **Sem estado global mutável.** Paths/seams são injetados (DI). `apply_waybar_integration`/`run_setup`/`run_doctor`/`run_managed_update` recebem paths e seams por parâmetro para serem testáveis sem tocar `~/.config` real.
- **Não mutar o desktop real em teste.** Testes usam `tempfile::tempdir()` + paths injetados + env `AGENT_BAR_FORCE_COMPILED`. Nunca `pkill waybar` real, nunca symlink em `~/.local/bin` real, nunca tocar `~/.config/waybar`.
- **Constantes de identidade** de `app_identity.rs` (`APP_NAME`, `WAYBAR_NAMESPACE`, `WAYBAR_MODULE_PREFIX`, `WAYBAR_SELECTOR_PREFIX`, `TERMINAL_HELPER_NAME`, `BACKUP_SUFFIX`, `APP_HIDDEN_CLASS`) — nunca hardcode strings.
- **Verificação por task:** `cargo test --manifest-path rust/Cargo.toml <filtro>` (UM filtro posicional só) + `cargo clippy --manifest-path rust/Cargo.toml --all-targets -- -D warnings`. **RTK reformata o cargo:** NÃO existe `test result:`; ler bruto com `2>&1 | tail -8` e somar `passed`. Clean = `cargo clippy: No issues found`.
- **`cargo fmt --manifest-path rust/Cargo.toml` ANTES de `git add`.** Read antes de Edit (cat/sed NÃO contam p/ o harness); se Edit falhar com `string not found`, re-Read antes de re-tentar.
- **Commits:** Conventional Commits em PT, subject ≤50 chars.
- **DESCOPADO deste plano (vai p/ Plano 7 — login/TUI):** `install.rs` (`ensure_command`/`ensure_amp_cli`) — usado só por `tui/login*.ts` + `amp-cli.ts` (fluxo de login interativo). `menu` continua **stub** (a TUI do Plano 7 o substitui).

---

## Mapa de Arquivos (decomposição)

Novos módulos em `rust/src/`:

| Arquivo | Responsabilidade | TS-fonte |
| --- | --- | --- |
| `runtime.rs` | `is_system_install()` (seam `AGENT_BAR_FORCE_COMPILED` + heurística `current_exe`) | `src/runtime.ts` |
| `waybar_contract.rs` | export de módulos/CSS, asset paths, `resolve_asset_source_root`, `install_waybar_assets`, `get_all_provider_ids` | `src/waybar-contract.ts` |
| `waybar_integration.rs` | scanner cirúrgico de JSONC: `apply_waybar_integration`/`remove_waybar_integration` + primitivas | `src/waybar-integration.ts` |
| `doctor.rs` | `scan`/`run_doctor` (limpeza de leftovers npm) | `src/doctor.ts` |
| `update.rs` | `detect_install_kind`/`run_managed_update`/`run_npm_update` (seams) | `src/update.ts` |
| `setup.rs` | `run_setup`/`create_symlink`/`reload_waybar` | `src/setup.ts` |
| `uninstall.rs` | `run_uninstall` (remove → force) | `src/uninstall.ts`, `src/remove.ts` |
| `term_prompt.rs` | helper mínimo de confirm (stdin) + linhas de status temáticas | (substitui `@clack/prompts`) |

Modificados:
- `rust/src/lib.rs` — adicionar `pub mod` para cada módulo novo (ordem alfabética).
- `rust/src/main.rs` — substituir os 8 stubs (Setup/AssetsInstall/ExportWaybarModules/ExportWaybarCss/Update/Uninstall/Remove/Doctor) por dispatch real; `Menu` continua stub.
- `rust/src/theme.rs` — adicionar tokens de cor faltantes ao CSS (`Overlay`/`Surface`/`BorderSoft`/`BorderStrong`) OU usar constantes locais no `waybar_contract.rs` (ver T1).

Ordem de execução (dependências): **T1 → T2** (waybar_contract); **T3a → T3b** (waybar_integration); **T4** (doctor) e **T5** (update) independentes; **T6** (setup/uninstall, depende de T1/T2/T3b); **T7** (term_prompt + wiring, depende de tudo). T3a/T3b/T4/T5 podem ser revisados em qualquer ordem após T1.

---

## Notas de Design (decisões deliberadas — para o reviewer)

1. **`is_system_install()` substitui `isCompiledBinary()`.** No mundo TS/Bun, `isCompiledBinary()` detecta o binário `bun build --compile` via prefixo `/$bunfs`. No Rust **tudo** é compilado, então a distinção vira "instalação de sistema (AUR) vs checkout dev/managed". Implementação: honra `AGENT_BAR_FORCE_COMPILED=1` (mantido como **seam de teste**, igual ao TS); senão `true` se `std::env::current_exe()` resolve sob `/usr/`. Só o seam é exercido por teste; o branch de produção é razoável-mas-não-coberto (igual ao TS, cujo branch `$bunfs` real também não é testado). Plano 8 (cutover) revisita.

2. **`DEFAULT_REPO_ROOT` / `repo_root()` = `env!("CARGO_MANIFEST_DIR")` + `..`.** Espelha o `import.meta.dir/..` do TS (path "assado" em build, só significativo em contexto dev/source; bypassed em system install). Pré-cutover o crate está em `rust/`, então `manifest_dir/..` = raiz do projeto (contém `icons/`+`.git`). Plano 8 ajusta na promoção para a raiz.

3. **Interatividade mínima, não @clack.** Os shells `setup`/`uninstall`/`update`/`doctor` usam `term_prompt::confirm(msg, default)` (lê stdin, `y/N`) + linhas de status temáticas em stderr. NÃO replicam spinners/notes do `@clack`. Justificativa: (a) `@clack` é lib TS; (b) o contrato testado é a lógica pura + JSON stdout, não o rendering; (c) a superfície interativa rica é a TUI ratatui (Plano 7), que reusa estas mesmas funções puras. Comportamento (confirm semantics, removals, wiring) é fiel.

4. **`normalize_provider_selection` é reusado de `settings.rs`** (já portado no Plano 3/5; assinatura `(&[String], &[String]) -> (Vec<String>, Vec<String>)`). `waybar_contract.rs` re-exporta/chama; não reimplementar.

---

### Task 1: `runtime.rs` + `waybar_contract.rs` — exports puros + asset paths

**Files:**
- Create: `rust/src/runtime.rs`
- Create: `rust/src/waybar_contract.rs`
- Modify: `rust/src/lib.rs` (add `pub mod runtime;` e `pub mod waybar_contract;`, ordem alfabética)
- Test: inline `#[cfg(test)] mod tests` em ambos

**Interfaces:**
- Consumes: `app_identity::{APP_NAME, WAYBAR_NAMESPACE, WAYBAR_MODULE_PREFIX, WAYBAR_SELECTOR_PREFIX, TERMINAL_HELPER_NAME, APP_HIDDEN_CLASS}`; `settings::normalize_provider_selection`; `theme::ColorToken`; `config::Paths` (não necessário aqui).
- Produces:
  - `runtime::is_system_install() -> bool`
  - `waybar_contract::WAYBAR_PROVIDERS: [&str; 3] = ["claude","codex","amp"]`
  - `waybar_contract::get_all_provider_ids() -> Vec<String>`
  - `waybar_contract::WaybarModuleConfig` (struct serde, ver abaixo)
  - `waybar_contract::module_definition(provider: &str, app_bin: &str, terminal_script: &str, signal: Option<u8>) -> WaybarModuleConfig`
  - `waybar_contract::WaybarModulesExport { providers: Vec<String>, modules: IndexMap<String, WaybarModuleConfig> }` (serde, camelCase N/A — chaves são `providers`/`modules`)
  - `waybar_contract::export_waybar_modules(app_bin: &str, terminal_script: &str, signal: Option<u8>, providers: &[String]) -> WaybarModulesExport`
  - `waybar_contract::export_waybar_css(icons_dir: &str, provider_order: &[String], separators: SeparatorStyle) -> String` (retorna o CSS; o TS retorna `{css}` mas no Rust o caller embrulha em `{"css": ...}` no comando — ver T7)
  - `waybar_contract::WaybarAssetPaths { waybar_dir, scripts_dir, icons_dir, terminal_script, app_bin: String }`
  - `waybar_contract::get_default_waybar_asset_paths() -> WaybarAssetPaths`
  - `waybar_contract::resolve_asset_source_root() -> anyhow::Result<PathBuf>`

**Contexto:** O comando `export-waybar-css` (T7) embrulha o retorno em `{"css": <string>}` e imprime pretty. O `export-waybar-modules` imprime o `WaybarModulesExport` inteiro pretty. O `apply_waybar_integration` (T3b) escreve só o `modules` map. Por isso `export_waybar_modules` devolve o struct com os dois campos.

**Atenção ao CSS (`export_waybar_css`):** é um template de string byte-exact. Reproduzir `src/waybar-contract.ts:273-325` linha-a-linha. Hexes usados: `ColorToken::Text` (#c0c9d4), `ColorToken::TextBright` (#e2e8f0), `ColorToken::Green/Yellow/Orange/Red`, mais hardcodes `#434d5d` (border-left), `#3c4656` (hover border), `rgba(192, 201, 212, 0.04)` (hover bg), e `SURFACE = #242a33` (ONE_DARK.overlay, usado só no separador `pill`/`glass`). Definir no topo de `waybar_contract.rs`: `const SURFACE: &str = "#242a33";`. O CSS junta linhas com `\n` (`[...].join('\n')`).

- [ ] **Step 1: Write failing tests (`runtime.rs`)**

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[serial_test::serial]
    fn forced_compiled_env_is_system() {
        temp_env::with_var("AGENT_BAR_FORCE_COMPILED", Some("1"), || {
            assert!(is_system_install());
        });
    }

    #[test]
    #[serial_test::serial]
    fn unset_force_env_is_not_forced() {
        // Sem o env e fora de /usr/, deve ser false no ambiente de teste (cargo target/).
        temp_env::with_var("AGENT_BAR_FORCE_COMPILED", None::<&str>, || {
            // current_exe do test runner não está sob /usr/ → false.
            assert!(!is_system_install());
        });
    }
}
```

- [ ] **Step 2: Run → fail** (`is_system_install` não existe).

Run: `cargo test --manifest-path rust/Cargo.toml runtime 2>&1 | tail -8`
Expected: erro de compilação (função inexistente).

- [ ] **Step 3: Implement `runtime.rs`**

```rust
//! Detecção de tipo de instalação. Port de `src/runtime.ts` (`isCompiledBinary`),
//! adaptado: no Rust tudo é compilado, então a distinção é "sistema (AUR) vs dev/managed".

/// `true` quando rodando como instalação de sistema (AUR/pacote).
/// Seam de teste: `AGENT_BAR_FORCE_COMPILED=1` força `true` (igual ao TS).
/// Heurística de produção: o executável resolve sob `/usr/`.
pub fn is_system_install() -> bool {
    if std::env::var_os("AGENT_BAR_FORCE_COMPILED").as_deref() == Some(std::ffi::OsStr::new("1")) {
        return true;
    }
    std::env::current_exe()
        .map(|p| p.starts_with("/usr/"))
        .unwrap_or(false)
}
```

- [ ] **Step 4: Run → pass.** `cargo test --manifest-path rust/Cargo.toml runtime 2>&1 | tail -8`

- [ ] **Step 5: Write failing tests (`waybar_contract.rs`)** — portar `tests/waybar-contract.test.ts`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::settings::SeparatorStyle;

    fn s(v: &[&str]) -> Vec<String> { v.iter().map(|x| x.to_string()).collect() }

    #[test]
    fn modules_wire_click_handlers_through_terminal_helper() {
        let e = export_waybar_modules(
            "$HOME/.local/bin/agent-bar",
            "$HOME/.config/waybar/scripts/agent-bar-open-terminal",
            None,
            &s(&["claude", "codex", "amp"]),
        );
        let claude = &e.modules["custom/agent-bar-claude"];
        assert_eq!(claude.on_click, "$HOME/.config/waybar/scripts/agent-bar-open-terminal $HOME/.local/bin/agent-bar menu");
        let codex = &e.modules["custom/agent-bar-codex"];
        assert!(codex.exec_on_event);
        assert_eq!(codex.exec, "$HOME/.local/bin/agent-bar --provider codex");
        let amp = &e.modules["custom/agent-bar-amp"];
        assert_eq!(amp.on_click_right, "$HOME/.config/waybar/scripts/agent-bar-open-terminal $HOME/.local/bin/agent-bar action-right amp");
    }

    #[test]
    fn modules_only_for_requested_providers() {
        let e = export_waybar_modules("/usr/bin/agent-bar", "/usr/bin/open-terminal", None, &s(&["claude"]));
        assert_eq!(e.modules.len(), 1);
        assert!(e.modules.contains_key("custom/agent-bar-claude"));
        assert!(!e.modules.contains_key("custom/agent-bar-codex"));
    }

    #[test]
    fn signal_present_when_provided_absent_otherwise() {
        let with = export_waybar_modules("bin", "term", Some(8), &s(&["claude", "codex"]));
        assert_eq!(with.modules["custom/agent-bar-claude"].signal, Some(8));
        let without = export_waybar_modules("bin", "term", None, &s(&["claude"]));
        assert_eq!(without.modules["custom/agent-bar-claude"].signal, None);
    }

    #[test]
    fn css_has_base_styles_icons_states() {
        let css = export_waybar_css("/home/user/.config/waybar/agent-bar/icons", &s(&["claude","codex","amp"]), SeparatorStyle::Gap);
        for sel in ["#custom-agent-bar-claude", "#custom-agent-bar-codex", "#custom-agent-bar-amp"] {
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
        for st in [SeparatorStyle::Pill, SeparatorStyle::Gap, SeparatorStyle::Bare, SeparatorStyle::Glass, SeparatorStyle::Shadow, SeparatorStyle::None] {
            let css = export_waybar_css("/icons", &s(&["claude"]), st);
            assert!(css.len() > 100);
        }
        assert!(export_waybar_css("/i", &s(&["claude"]), SeparatorStyle::Pill).contains("border-radius"));
        assert!(export_waybar_css("/i", &s(&["claude"]), SeparatorStyle::Bare).contains("border-color: transparent"));
        assert!(export_waybar_css("/i", &s(&["claude"]), SeparatorStyle::Glass).contains("rgba("));
        assert!(export_waybar_css("/i", &s(&["claude"]), SeparatorStyle::Shadow).contains("box-shadow"));
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
            [("AGENT_BAR_FORCE_COMPILED", Some("1")), ("AGENT_BAR_ASSET_DIR", None)],
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
            assert!(resolve_asset_source_root().unwrap_err().to_string().contains("AGENT_BAR_ASSET_DIR must be"));
        });
        temp_env::with_var("AGENT_BAR_ASSET_DIR", Some("relative/path"), || {
            assert!(resolve_asset_source_root().unwrap_err().to_string().contains("AGENT_BAR_ASSET_DIR must be"));
        });
    }

    #[test]
    #[serial_test::serial]
    fn default_app_bin_system_vs_local() {
        temp_env::with_var("AGENT_BAR_FORCE_COMPILED", Some("1"), || {
            assert_eq!(get_default_waybar_asset_paths().app_bin, "agent-bar");
        });
        temp_env::with_var("AGENT_BAR_FORCE_COMPILED", None::<&str>, || {
            assert_eq!(get_default_waybar_asset_paths().app_bin, "$HOME/.local/bin/agent-bar");
        });
    }
}
```

- [ ] **Step 6: Run → fail.** `cargo test --manifest-path rust/Cargo.toml waybar_contract 2>&1 | tail -8`

- [ ] **Step 7: Implement `waybar_contract.rs`**

Detalhes-chave (ler `src/waybar-contract.ts` como spec):
- `WaybarModuleConfig` struct serde com `#[serde(rename = "...")]`:
  ```rust
  #[derive(Debug, Clone, Serialize)]
  pub struct WaybarModuleConfig {
      pub exec: String,
      #[serde(rename = "return-type")]
      pub return_type: String, // sempre "json"
      pub interval: u32,       // sempre 120 (hardcoded no TS moduleDefinition)
      #[serde(rename = "exec-on-event")]
      pub exec_on_event: bool, // true
      pub tooltip: bool,       // true
      #[serde(rename = "on-click")]
      pub on_click: String,
      #[serde(rename = "on-click-right")]
      pub on_click_right: String,
      #[serde(skip_serializing_if = "Option::is_none")]
      pub signal: Option<u8>,
  }
  ```
- `module_definition`: `exec = format!("{app_bin} --provider {provider}")`; `on_click = format!("{terminal_script} {app_bin} menu")`; `on_click_right = format!("{terminal_script} {app_bin} action-right {provider}")`; `return_type="json"`, `interval=120`, `exec_on_event=true`, `tooltip=true`, `signal` passa direto.
- `WaybarModulesExport`: `#[derive(Serialize)] { pub providers: Vec<String>, pub modules: IndexMap<String, WaybarModuleConfig> }`. Usar `indexmap::IndexMap` (já dep — confirmado no Plano 4). Inserção na ordem de `providers` → preserva ordem no JSON pretty.
- `export_waybar_modules`: para cada provider, `modules.insert(format!("{WAYBAR_MODULE_PREFIX}{provider}"), module_definition(...))`. Retorna `{ providers: providers.to_vec(), modules }`.
- `separator_css(providers, style) -> String`: port de `separatorCss` (`src/waybar-contract.ts:144-216`). Se `providers` vazio → `""`. Senão monta `selectorBlock = providers.map(|p| format!("{WAYBAR_SELECTOR_PREFIX}{p}")).join(",\n")` e o bloco por estilo (pill/gap/bare/glass/shadow/none), juntando com `\n`. **Cada bloco termina com `''` extra no array** (= linha em branco final) — reproduzir o `, ''` final de cada array TS.
- `export_waybar_css(icons_dir, provider_order, separators) -> String`: port de `exportWaybarCss` (`src/waybar-contract.ts:273-325`).
  - `icon_ref(name)`: `let p = format!("{icons_dir}/{name}"); if p.starts_with('/') { file_url(p) } else { p }`. `file_url` = `format!("file://{}", p)` (port de `pathToFileURL().toString()`; os paths aqui não têm caracteres a escapar — testes usam paths simples; manter `file://` + path absoluto). **Nota:** `pathToFileURL` percent-encoda; para paths ASCII simples sem espaços é `file://` + path. Documentar como simplificação fiel ao uso real.
  - `provider_order` efetivo = se vazio, `WAYBAR_PROVIDERS`; senão o passado.
  - `all_provider_selectors` = `WAYBAR_PROVIDERS.map(|p| #custom-...-p).join(",\n")` (sempre os 3, não o order).
  - `state_selectors(state)` = `WAYBAR_PROVIDERS.map(|p| format!("{WAYBAR_SELECTOR_PREFIX}{p}.{state}")).join(", ")`.
  - Montar o vetor de linhas exatamente como o TS (linhas 286-323) e `join("\n")`. A última entrada é `separators` (string do `separator_css`).
- `WaybarAssetPaths` + `get_default_waybar_asset_paths()`: port de `getDefaultWaybarAssetPaths` (`src/waybar-contract.ts:218-228`). `home = HOME env`; `waybar_root = home/.config/waybar`; `waybar_dir = waybar_root/{WAYBAR_NAMESPACE}`; `scripts_dir = waybar_root/scripts`; `icons_dir = waybar_dir/icons`; `terminal_script = scripts_dir/{TERMINAL_HELPER_NAME}`; `app_bin = if is_system_install() { APP_NAME.to_string() } else { format!("$HOME/.local/bin/{APP_NAME}") }`. Campos path → `String` (via `to_string_lossy`) ou `PathBuf`; para o JSON do `assets-install` (T2/T7) precisam virar string — usar `PathBuf` nos campos e converter no caller, OU `String`. **Decisão: `waybar_dir`/`scripts_dir`/`icons_dir`/`terminal_script` = `PathBuf`; `app_bin` = `String`** (é um literal com `$HOME`, não um path real).
- `resolve_asset_source_root() -> anyhow::Result<PathBuf>`: port de `resolveAssetSourceRoot` (`src/waybar-contract.ts:74-94`):
  - `has_icons = |d: &Path| d.join("icons").exists()`.
  - Se `AGENT_BAR_ASSET_DIR` setado: se `!path.is_absolute() || !has_icons(path)` → `anyhow::bail!("AGENT_BAR_ASSET_DIR must be an absolute path containing icons/ (got: {env_dir}).")`; senão `Ok(path)`.
  - Se `is_system_install()`: `SYSTEM_ASSET_DIR = format!("/usr/share/{APP_NAME}")`; se `has_icons` → `Ok`; senão `bail!("Asset directory not found at {SYSTEM_ASSET_DIR}. Reinstall the package, or set AGENT_BAR_ASSET_DIR.")`.
  - Senão (dev): `repo_root = Path::new(env!("CARGO_MANIFEST_DIR")).parent()` (= raiz do projeto pré-cutover); se `has_icons` → `Ok`; senão `bail!("Asset directory not found. Run `agent-bar setup` from a checkout, or set AGENT_BAR_ASSET_DIR.")`.
- `get_all_provider_ids() -> Vec<String>`: port de `getAllProviderIds` — começa com `WAYBAR_PROVIDERS` e adiciona `registered_provider_ids()` (de `providers::`) sem duplicar. Como o registry Rust = `[claude,amp,codex]` e `WAYBAR_PROVIDERS=[claude,codex,amp]`, o resultado é os 3 sem dup. (Usado por nada crítico no Plano 6 — exportar para paridade; testar dedup simples.)
- `WAYBAR_PROVIDERS: [&str; 3] = ["claude", "codex", "amp"]` (igual ao TS `WAYBAR_PROVIDERS`).

- [ ] **Step 8: Run → pass.** `cargo test --manifest-path rust/Cargo.toml waybar_contract 2>&1 | tail -8` (espera todos verdes). Depois `cargo clippy --manifest-path rust/Cargo.toml --all-targets -- -D warnings`.

- [ ] **Step 9: `cargo fmt` + commit**

```bash
cargo fmt --manifest-path rust/Cargo.toml
git add rust/src/runtime.rs rust/src/waybar_contract.rs rust/src/lib.rs
git commit -m "feat(rust): runtime + waybar-contract exports/assets"
```

---

### Task 2: `waybar_contract.rs` — `install_waybar_assets` (cópia de assets)

**Files:**
- Modify: `rust/src/waybar_contract.rs` (adicionar `copy_dir` + `install_waybar_assets` + `InstalledAssets`)
- Test: inline `#[cfg(test)] mod tests` (adicionar casos)

**Interfaces:**
- Consumes: `app_identity::TERMINAL_HELPER_NAME`; `resolve_asset_source_root` (T1).
- Produces:
  - `waybar_contract::InstalledAssets { icons_dir: PathBuf, terminal_script: PathBuf }` (serde, chaves `iconsDir`/`terminalScript` — ver nota JSON)
  - `waybar_contract::install_waybar_assets(waybar_dir: &Path, scripts_dir: &Path, repo_root: Option<&Path>) -> anyhow::Result<InstalledAssets>`

**Nota JSON:** o comando `assets-install` (T7) imprime `serde_json::to_string(&InstalledAssets)`. O TS imprime `{iconsDir, terminalScript}` (camelCase, paths absolutos como string). Então `InstalledAssets` serializa com `#[serde(rename_all = "camelCase")]` e os campos `PathBuf` viram string JSON via serde (PathBuf serializa como string). Confirmar: `serde_json` serializa `PathBuf` como a string do path. ✓

- [ ] **Step 1: Write failing test**

```rust
#[test]
fn install_copies_icons_and_helper_with_exec_perm() {
    use std::os::unix::fs::PermissionsExt;
    let src = tempfile::tempdir().unwrap();
    // fixture: src/icons/a.png + src/scripts/agent-bar-open-terminal
    std::fs::create_dir_all(src.path().join("icons")).unwrap();
    std::fs::write(src.path().join("icons").join("a.png"), b"png").unwrap();
    std::fs::create_dir_all(src.path().join("scripts")).unwrap();
    std::fs::write(src.path().join("scripts").join("agent-bar-open-terminal"), b"#!/bin/sh\n").unwrap();

    let dest = tempfile::tempdir().unwrap();
    let waybar_dir = dest.path().join("agent-bar");
    let scripts_dir = dest.path().join("scripts");

    let r = install_waybar_assets(&waybar_dir, &scripts_dir, Some(src.path())).unwrap();
    assert!(r.icons_dir.join("a.png").exists());
    assert!(r.terminal_script.exists());
    let mode = std::fs::metadata(&r.terminal_script).unwrap().permissions().mode();
    assert_eq!(mode & 0o777, 0o755);
}

#[test]
fn install_errors_when_icons_source_missing() {
    let src = tempfile::tempdir().unwrap(); // sem icons/
    std::fs::create_dir_all(src.path().join("scripts")).unwrap();
    std::fs::write(src.path().join("scripts").join("agent-bar-open-terminal"), b"x").unwrap();
    let dest = tempfile::tempdir().unwrap();
    let err = install_waybar_assets(&dest.path().join("a"), &dest.path().join("s"), Some(src.path())).unwrap_err().to_string();
    assert!(err.contains("Icons folder not found"), "got: {err}");
}
```

- [ ] **Step 2: Run → fail.** `cargo test --manifest-path rust/Cargo.toml install_ 2>&1 | tail -8`

- [ ] **Step 3: Implement** (port de `copyDir` + `installWaybarAssets`, `src/waybar-contract.ts:96-110,327-358`):

```rust
use std::path::{Path, PathBuf};

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

/// Copia `icons/` e o terminal helper para o destino do Waybar. `repo_root=None` resolve via `resolve_asset_source_root`.
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
    let script_source = repo_root.join("scripts").join(crate::app_identity::TERMINAL_HELPER_NAME);
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

    Ok(InstalledAssets { icons_dir: icons_dest, terminal_script: script_dest })
}
```

**Strings de contrato:** `"Icons folder not found: {path}"` e `"Terminal helper not found: {path}"` — verbatim do TS (`Icons folder not found: ${iconsSource}`).

- [ ] **Step 4: Run → pass.** `cargo test --manifest-path rust/Cargo.toml install_ 2>&1 | tail -8` + clippy.

- [ ] **Step 5: `cargo fmt` + commit**

```bash
cargo fmt --manifest-path rust/Cargo.toml
git add rust/src/waybar_contract.rs
git commit -m "feat(rust): install_waybar_assets (cópia de assets)"
```

---

### Task 3a: `waybar_integration.rs` — primitivas de scanner de string

**Files:**
- Create: `rust/src/waybar_integration.rs` (só as primitivas + testes)
- Modify: `rust/src/lib.rs` (`pub mod waybar_integration;`)
- Test: inline

**Interfaces:**
- Consumes: `regex::Regex`, `regex::escape`, `serde_json` (unescape de string).
- Produces (todas `pub(crate)` ou privadas, exceto onde T3b precisa):
  - `fn skip_string(content: &[char], i: usize) -> usize`
  - `fn find_matching_bracket(content: &[char], open_idx: usize) -> Option<usize>`
  - `fn parse_quoted_strings(block: &str) -> Vec<String>`
  - `fn arrays_equal(a: &[String], b: &[String]) -> bool` (trivial — pode usar `==`; manter como helper só se ajudar legibilidade)
  - `fn format_string_array(values: &[String], indent: &str) -> String`
  - `struct RewriteArrayResult { content: String, found: bool, changed: bool }`
  - `fn rewrite_string_array_property(content: &str, property: &str, transform: impl Fn(Vec<String>) -> Vec<String>) -> RewriteArrayResult`

**Decisão de indexação:** o scanner TS trabalha em índices de `char`/UTF-16; para JSONC ASCII (configs do Waybar são ASCII) char-index é seguro e mais simples que byte-index. **Trabalhar sobre `Vec<char>`** nas primitivas de bracket (`skip_string`/`find_matching_bracket`), e sobre `&str` + `regex` no `rewrite_string_array_property` (convertendo offsets). Para `find_matching_bracket`, receber `&[char]` e índice de char. **Cuidado:** `rewrite_string_array_property` precisa casar regex em `&str` (byte offsets) mas chamar `find_matching_bracket` em char-index. Solução: converter o `content` para `Vec<char>` uma vez e trabalhar tudo em char-index, OU operar em bytes assumindo ASCII. **Escolha: operar em bytes (`&[u8]`/`&str` byte-offsets)** — JSONC do Waybar é ASCII na prática, e o regex já dá byte-offsets. `find_matching_bracket`/`skip_string` recebem `&[u8]` e byte-index. Isto evita conversão char↔byte. Documentar a premissa ASCII (igual ao TS, que assume estrutura JSON ASCII; valores não-ASCII ficam dentro de strings, que o scanner pula via `skip_string`).

  → **Reescrever as assinaturas para bytes:**
  - `fn skip_string(content: &[u8], i: usize) -> usize` (i aponta a aspa de abertura; retorna índice após a aspa de fechamento)
  - `fn find_matching_bracket(content: &[u8], open_idx: usize) -> Option<usize>`

- [ ] **Step 1: Write failing tests**

```rust
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
    fn parse_quoted_unescapes_via_json() {
        let v = parse_quoted_strings(r#""a", "b\"c", "d\\e""#);
        assert_eq!(v, vec!["a".to_string(), "b\"c".to_string(), "d\\e".to_string()]);
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
        let r = rewrite_string_array_property(content, "include", |mut v| { v.push("b".into()); v });
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
```

- [ ] **Step 2: Run → fail.** `cargo test --manifest-path rust/Cargo.toml waybar_integration 2>&1 | tail -8`

- [ ] **Step 3: Implement primitivas** (port de `src/waybar-integration.ts:70-216`):

```rust
//! Patcher cirúrgico do Waybar config/style (JSONC). NÃO usa crate de JSONC —
//! comentários e ordem do arquivo do usuário precisam sobreviver. Premissa: a
//! ESTRUTURA do JSONC é ASCII (chaves/colchetes/aspas); valores não-ASCII ficam
//! dentro de strings, puladas por `skip_string`. Port de `src/waybar-integration.ts`.

use regex::Regex;

/// Avança além de um literal de string JSON; `i` aponta à aspa de abertura.
/// Retorna o índice logo após a aspa de fechamento.
fn skip_string(content: &[u8], mut i: usize) -> usize {
    i += 1;
    while i < content.len() {
        match content[i] {
            b'\\' => { i += 2; continue; }
            b'"' => return i + 1,
            _ => i += 1,
        }
    }
    i
}

/// Acha o `]` que fecha o `[` em `open_idx`, honrando colchetes aninhados,
/// strings e comentários JSONC. `None` se desbalanceado.
fn find_matching_bracket(content: &[u8], open_idx: usize) -> Option<usize> {
    let mut depth: i32 = 0;
    let mut i = open_idx;
    while i < content.len() {
        let c = content[i];
        if c == b'"' {
            i = skip_string(content, i);
            continue;
        }
        if c == b'/' && content.get(i + 1) == Some(&b'/') {
            match content[i..].iter().position(|&b| b == b'\n') {
                Some(rel) => { i += rel; }       // para no '\n'
                None => return None,             // sem '\n' → resto é comentário; ']' não achado
            }
            continue;
        }
        if c == b'/' && content.get(i + 1) == Some(&b'*') {
            // procura "*/" a partir de i+2
            let rest = &content[i + 2..];
            match rest.windows(2).position(|w| w == b"*/") {
                Some(rel) => { i = i + 2 + rel + 2; }
                None => return None,
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
```

**Atenção ao port do `//` comentário:** o TS faz `const nl = content.indexOf('\n', i); i = nl === -1 ? content.length : nl;` (sem `continue` que pule o `\n`; o loop externo então processa `\n` como char normal e `i+=1`). No Rust acima, ao achar `//` sem `\n`, o TS seta `i = content.length` e o while termina → retorna -1 (None). Reproduzir: se `None` (sem `\n`), `i = content.len()` e deixar o while terminar naturalmente (resultando em `None` no fim). **Ajuste:** trocar `None => return None` por `None => { i = content.len(); continue; }` para espelhar exatamente (o while sai e cai no `None` final). Idem block-comment sem `*/`: TS `i = end === -1 ? content.length : end + 2` → setar `i = content.len()`.

```rust
/// Extrai os valores de string de um corpo `"a", "b", ...` (JSON-unescape via serde).
fn parse_quoted_strings(block: &str) -> Vec<String> {
    // regex equivalente a /"((?:\\.|[^"\\])*)"/g
    static RE: std::sync::OnceLock<Regex> = std::sync::OnceLock::new();
    let re = RE.get_or_init(|| Regex::new(r#""((?:\\.|[^"\\])*)""#).expect("regex válida"));
    let mut out = Vec::new();
    for cap in re.captures_iter(block) {
        let inner = &cap[1];
        // JSON.parse(`"${inner}"`) → serde_json::from_str
        if let Ok(s) = serde_json::from_str::<String>(&format!("\"{inner}\"")) {
            out.push(s);
        }
    }
    out
}

fn format_string_array(values: &[String], indent: &str) -> String {
    if values.is_empty() {
        return "[]".to_string();
    }
    let item_indent = format!("{indent}  ");
    let lines = values
        .iter()
        .map(|v| format!("{item_indent}{}", serde_json::to_string(v).unwrap_or_default()))
        .collect::<Vec<_>>()
        .join(",\n");
    format!("[\n{lines}\n{indent}]")
}

struct RewriteArrayResult {
    content: String,
    found: bool,
    changed: bool,
}

fn rewrite_string_array_property(
    content: &str,
    property: &str,
    transform: impl Fn(Vec<String>) -> Vec<String>,
) -> RewriteArrayResult {
    // keyPattern = /"<prop>"\s*:\s*\[/g
    let pattern = format!(r#""{}"\s*:\s*\["#, regex::escape(property));
    let re = match Regex::new(&pattern) {
        Ok(r) => r,
        Err(_) => return RewriteArrayResult { content: content.to_string(), found: false, changed: false },
    };
    let bytes = content.as_bytes();
    for m in re.find_iter(content) {
        let match_start = m.start();
        // linePrefix = trecho do início da linha até match_start
        let line_start = content[..match_start].rfind('\n').map(|p| p + 1).unwrap_or(0);
        let line_prefix = &content[line_start..match_start];
        if line_prefix.contains("//") {
            continue; // ocorrência comentada
        }
        let open_idx = m.end() - 1; // índice do '['
        let close_idx = match find_matching_bracket(bytes, open_idx) {
            Some(c) => c,
            None => continue,
        };
        let body = &content[open_idx + 1..close_idx];
        let current = parse_quoted_strings(body);
        let next = transform(current.clone());

        // indent = whitespace do início da linha
        let indent: String = line_prefix.chars().take_while(|c| c.is_whitespace()).collect();

        if current == next {
            return RewriteArrayResult { content: content.to_string(), found: true, changed: false };
        }
        let prefix = &content[match_start..open_idx]; // `"prop"\s*:\s*`
        let rewritten = format!(
            "{}{}{}{}",
            &content[..match_start],
            prefix,
            format_string_array(&next, &indent),
            &content[close_idx + 1..]
        );
        return RewriteArrayResult { content: rewritten, found: true, changed: true };
    }
    RewriteArrayResult { content: content.to_string(), found: false, changed: false }
}
```

**Nota sobre `.expect("regex válida")` em `parse_quoted_strings`:** o `deny(clippy::expect_used)` é só `not(test)`. Em produção a regex é literal constante e sempre compila; ainda assim, para satisfazer o lint, usar `OnceLock` com `Regex::new(...).expect(...)` **dispara o lint em produção**. Usar o padrão já estabelecido no projeto (`derive_claude_plan` em `claude.rs`): `OnceLock<Option<Regex>>` com `Regex::new(...).ok()` e early-return se `None`. Reescrever `parse_quoted_strings` para `RE: OnceLock<Option<Regex>>`, `let Some(re) = RE.get_or_init(|| Regex::new(...).ok()) else { return Vec::new(); }`. Idem qualquer regex em T3b.

- [ ] **Step 4: Run → pass.** `cargo test --manifest-path rust/Cargo.toml waybar_integration 2>&1 | tail -8` + clippy.

- [ ] **Step 5: `cargo fmt` + commit**

```bash
cargo fmt --manifest-path rust/Cargo.toml
git add rust/src/waybar_integration.rs rust/src/lib.rs
git commit -m "feat(rust): primitivas do patcher Waybar JSONC"
```

---

### Task 3b: `waybar_integration.rs` — apply/remove orchestration

**Files:**
- Modify: `rust/src/waybar_integration.rs` (adicionar orquestração + API pública)
- Test: inline (portar `tests/waybar-integration.test.ts`)

**Interfaces:**
- Consumes: primitivas T3a; `app_identity::{APP_NAME, WAYBAR_NAMESPACE, WAYBAR_MODULE_PREFIX, BACKUP_SUFFIX}`; `settings::{load, normalize_provider_selection}`; `config::Paths`; `waybar_contract::{export_waybar_modules, export_waybar_css, get_default_waybar_asset_paths}`.
- Produces:
  - `WaybarIntegrationPaths { waybar_config_path, waybar_style_path, modules_include_path, style_include_path }` (todos `PathBuf`)
  - `get_default_waybar_integration_paths() -> WaybarIntegrationPaths`
  - `get_app_module_ids(order: &[String]) -> Vec<String>`
  - `ApplyOptions { paths, icons_dir: Option<PathBuf>, app_bin: Option<String>, terminal_script: Option<PathBuf> }`
  - `ApplyResult { config_changed: bool, style_changed: bool, module_ids: Vec<String>, modules_include_path: PathBuf, style_include_path: PathBuf }`
  - `apply_waybar_integration(settings: &Settings, opts: ApplyOptions) -> anyhow::Result<ApplyResult>`
  - `RemoveResult { config_changed: bool, style_changed: bool, removed_includes: Vec<PathBuf> }`
  - `remove_waybar_integration(paths: &WaybarIntegrationPaths) -> anyhow::Result<RemoveResult>`

**Decisão de DI:** o TS `applyWaybarIntegration` chama `loadSettingsSync()` internamente (2×) e `resolveProviderOrder()`. Para testabilidade Rust, **`apply_waybar_integration` recebe `&Settings` por parâmetro** (o caller — setup/T7 — carrega via `settings::load`). Internamente deriva `provider_order` via `resolve_provider_order(settings)`. Os testes injetam um `Settings` default.

**`APP_STYLE_IMPORT`:** `format!("@import url(\"./{WAYBAR_NAMESPACE}/style.css\");")`.

- [ ] **Step 1: Write failing tests** (port de `tests/waybar-integration.test.ts`, 4 casos):

```rust
#[cfg(test)]
mod orchestration_tests {
    use super::*;
    use crate::config::Paths;
    use crate::settings::load;
    use std::path::PathBuf;
    use tempfile::tempdir;

    fn strip_jsonc(s: &str) -> String {
        // remove /* */ e // ... para validar via serde_json
        let block = regex::Regex::new(r"(?s)/\*.*?\*/").unwrap().replace_all(s, "");
        regex::Regex::new(r"(?m)^\s*//.*$").unwrap().replace_all(&block, "").to_string()
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
            cache_dir: dir.join("cache"), config_dir: dir.join("config"),
            claude_credentials: PathBuf::new(), codex_auth: PathBuf::new(),
            codex_sessions: PathBuf::new(), amp_settings: PathBuf::new(), amp_threads: PathBuf::new(),
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
        std::fs::write(&p.waybar_config_path, "{\n  \"modules-right\": [\"clock\", \"battery\"],\n  \"include\": [\"/existing/include.jsonc\"]\n}").unwrap();
        let s = default_settings(dir.path());
        let r = apply_waybar_integration(&s, apply_opts(&p)).unwrap();
        assert!(r.config_changed);
        let patched = std::fs::read_to_string(&p.waybar_config_path).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&strip_jsonc(&patched)).unwrap();
        let mr = parsed["modules-right"].as_array().unwrap();
        assert!(mr.iter().any(|v| v == "clock"));
        assert!(mr.iter().any(|v| v == "battery"));
        for id in get_app_module_ids(&vec!["claude".into(),"codex".into(),"amp".into()]) {
            assert!(mr.iter().any(|v| v.as_str() == Some(&id)), "missing {id}");
        }
        let inc = parsed["include"].as_array().unwrap();
        assert!(inc.iter().any(|v| v == "/existing/include.jsonc"));
        assert!(inc.iter().any(|v| v.as_str() == Some(p.modules_include_path.to_str().unwrap())));
    }

    #[test]
    fn does_not_corrupt_nested_array() {
        let dir = tempdir().unwrap();
        let p = test_paths(dir.path());
        std::fs::write(&p.waybar_config_path, "{\n  \"modules-right\": [\"clock\", {\"name\": \"x\", \"items\": [\"a\", \"b\"]}],\n  \"include\": []\n}").unwrap();
        let s = default_settings(dir.path());
        apply_waybar_integration(&s, apply_opts(&p)).unwrap();
        let patched = std::fs::read_to_string(&p.waybar_config_path).unwrap();
        assert!(serde_json::from_str::<serde_json::Value>(&strip_jsonc(&patched)).is_ok());
    }

    #[test]
    fn leaves_commented_modules_right_untouched() {
        let dir = tempdir().unwrap();
        let p = test_paths(dir.path());
        std::fs::write(&p.waybar_config_path, "{\n  // \"modules-right\": [\"old-module\"],\n  \"modules-right\": [\"clock\"],\n  \"include\": []\n}").unwrap();
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
        std::fs::write(&p.waybar_config_path, "{\n  \"modules-right\": [\"clock\"],\n  \"include\": []\n}").unwrap();
        std::fs::write(&p.waybar_style_path, "window { color: red; }\n").unwrap();
        let s = default_settings(dir.path());
        apply_waybar_integration(&s, apply_opts(&p)).unwrap();
        let rr = remove_waybar_integration(&p).unwrap();
        assert!(rr.config_changed);
        let final_cfg: serde_json::Value = serde_json::from_str(&strip_jsonc(&std::fs::read_to_string(&p.waybar_config_path).unwrap())).unwrap();
        let mr = final_cfg["modules-right"].as_array().unwrap();
        for id in get_app_module_ids(&vec!["claude".into(),"codex".into(),"amp".into()]) {
            assert!(!mr.iter().any(|v| v.as_str() == Some(&id)));
        }
        assert!(mr.iter().any(|v| v == "clock"));
        let backup = format!("{}{}", p.waybar_style_path.display(), crate::app_identity::BACKUP_SUFFIX);
        assert!(std::fs::read_to_string(&backup).unwrap().contains("window { color: red; }"));
    }
}
```

- [ ] **Step 2: Run → fail.** `cargo test --manifest-path rust/Cargo.toml waybar_integration 2>&1 | tail -8`

- [ ] **Step 3: Implement orquestração** (port de `src/waybar-integration.ts:218-481`). Helpers privados a portar fielmente:
  - `read_text(path) -> Option<String>` (None se não existe).
  - `write_text(path, content)`: `mkdir -p dirname`; escreve `content` garantindo `\n` final.
  - `backup_if_needed(path)`: `let bk = format!("{}{}", path.display(), BACKUP_SUFFIX); if !exists(bk) && exists(path) { copy(path, bk) }`.
  - `insert_property_into_first_object(content, property_text) -> Result<String>`: port de `insertPropertyIntoFirstObject` (`:218-232`); acha `{`, calcula indent via regex `\n(\s*)"`, detecta objeto vazio (`firstToken.startsWith('}')`); `bail!` se não há `{` com a msg `"Waybar config must contain an object to insert {APP_NAME} integration."`.
  - `is_managed_module(value) -> bool`: `value.starts_with(WAYBAR_MODULE_PREFIX)`.
  - `strip_managed_style_imports(content) -> String`: 3 regexes `(?m)` — port de `:238-246`. Regex 1: `^\s*/\*\s*{APP_NAME} managed import\s*\*/\n?`; Regex 2: `^\s*@import\s+url\((['"])\./{WAYBAR_NAMESPACE}/style\.css\1\);?\n?` — **o backreference `\1` não é suportado pelo crate `regex`**; reescrever sem backref: casar `["']` na abertura e aceitar `["']` no fechamento (ou duas alternativas explícitas). Como o conteúdo é gerado por nós (`APP_STYLE_IMPORT` usa `"`), casar literalmente `@import\s+url\("\./{ns}/style\.css"\);?` é suficiente; manter tolerância a `'` via alternância `(?:"\./{ns}/style\.css"|'\./{ns}/style\.css')`. Regex 3: `^\s*\n` (primeira linha em branco, **sem** `(?m)` — só no início).
  - `ensure_include_path(content, include_path) -> (String, bool)`: usa `rewrite_string_array_property` com transform que faz push se ausente; se `found`, retorna; senão `insert_property_into_first_object` com `"include": [<formatted>]`.
  - `remove_include_paths(content, &[paths]) -> (String, bool)`: rewrite com filter.
  - `reconcile_managed_modules(values, module_ids) -> Vec<String>`: port de `:277-299` (substitui managed pelos novos em ordem, mantém não-managed, anexa restantes).
  - `ensure_modules_right(content, module_ids) -> (String, bool)`: rewrite com `reconcile_managed_modules`; fallback insert.
  - `remove_modules_right(content) -> (String, bool)`: rewrite filtrando managed.
  - `ensure_style_import(content) -> (String, bool)`: port de `:325-333`.
  - `remove_style_import(content) -> (String, bool)`: port de `:335-338`.
  - `build_bootstrap_config(module_ids, include_path) -> String`: `serde_json::to_string_pretty` de um objeto na ordem exata `{layer:"top", position:"top", "modules-left":[], "modules-center":[], "modules-right":module_ids, include:[include_path]}`. **Ordem importa** → usar `serde_json::json!` macro NÃO garante ordem; usar uma struct serde com os campos na ordem, OU um `Vec<(&str, Value)>` serializado manualmente. **Decisão:** struct `BootstrapConfig` com `#[serde(rename="modules-left")]` etc., serializada com `to_string_pretty`.
  - `resolve_provider_order(settings) -> Vec<String>`: port de `:355-364` — `normalize_provider_selection(&settings.waybar.providers, &settings.waybar.provider_order)` → se `order` não-vazio, `order`; senão `providers`.
  - `get_app_module_ids(order)`: `order.iter().map(|p| format!("{WAYBAR_MODULE_PREFIX}{p}")).collect()`.
  - `apply_waybar_integration(settings, opts)`: port de `:380-436`:
    1. `defaults = get_default_waybar_asset_paths()`.
    2. `provider_order = resolve_provider_order(settings)`; `module_ids = get_app_module_ids(&provider_order)`.
    3. `export = export_waybar_modules(app_bin || defaults.app_bin, terminal_script || defaults.terminal_script, settings.waybar.signal, &provider_order)`; `write_text(modules_include_path, to_string_pretty(&export.modules))` — **só o map `modules`**, pretty.
    4. `css = export_waybar_css(icons_dir || defaults.icons_dir, &provider_order, settings.waybar.separators)`; `write_text(style_include_path, css)`.
    5. `current_config = read_text(waybar_config_path)`; se `None` → `build_bootstrap_config(module_ids, modules_include_path)`; senão `ensure_include_path` → `ensure_modules_right`.
    6. `config_changed = current != next`; se changed → `backup_if_needed` + `write_text`.
    7. style: `current_style = read_text(style_path)`; `ensure_style_import(current_style.unwrap_or_default())`; se `changed || current_style is None` → backup + write.
    8. retorna `ApplyResult`.
  - `remove_waybar_integration(paths)`: port de `:438-481` — remove includes/modules do config (com backup), remove style import (com backup), `rm -f` dos include files gerados, coleta `removed_includes`.

**Cuidado (paths como string nos arrays):** os include paths inseridos no JSONC são os paths absolutos como string (ex: `paths.modulesIncludePath`). No Rust, converter `PathBuf` → `&str` via `to_str()` (premissa: paths UTF-8; em teste são tempdirs UTF-8). Onde `to_str()` der `None`, usar `to_string_lossy()`.

- [ ] **Step 4: Run → pass.** `cargo test --manifest-path rust/Cargo.toml waybar_integration 2>&1 | tail -8` (T3a + T3b verdes) + clippy.

- [ ] **Step 5: `cargo fmt` + commit**

```bash
cargo fmt --manifest-path rust/Cargo.toml
git add rust/src/waybar_integration.rs
git commit -m "feat(rust): apply/remove integração Waybar"
```

---

### Task 4: `doctor.rs` — scan + run_doctor (limpeza de leftovers npm)

**Files:**
- Create: `rust/src/doctor.rs`
- Modify: `rust/src/lib.rs` (`pub mod doctor;`)
- Test: inline (portar `tests/doctor.test.ts`, 14 casos)

**Interfaces:**
- Consumes: `std::fs`, `serde_json`.
- Produces:
  - `DoctorFindings { package_json_path: Option<PathBuf>, package_json_orphan: bool, package_json_mixed: bool, node_modules_dir: Option<PathBuf>, lockfiles: Vec<PathBuf> }`
  - `scan(home: &Path) -> DoctorFindings`
  - `DoctorStatus` enum `{ Clean, Cancelled, Cleaned, MixedOnly }`
  - `DoctorResult { status: DoctorStatus, removed: Vec<PathBuf>, findings: DoctorFindings }`
  - `DoctorOptions<'a> { home: &'a Path, dry_run: bool, yes: bool, confirm: &'a dyn Fn(&DoctorFindings) -> bool }`
  - `run_doctor(opts: DoctorOptions) -> DoctorResult`

**Constantes:** `TARGET_PACKAGE = "@noctuacore/agent-bar"`; `LOCKFILE_NAMES = ["bun.lock", "bun.lockb", "package-lock.json"]`.

- [ ] **Step 1: Write failing tests** (port de `tests/doctor.test.ts`). Exemplos (portar TODOS os 14):

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn scan_clean_when_nothing_relevant() {
        let h = tempdir().unwrap();
        let f = scan(h.path());
        assert!(!f.package_json_orphan && !f.package_json_mixed);
        assert!(f.node_modules_dir.is_none());
        assert!(f.lockfiles.is_empty());
    }

    #[test]
    fn scan_detects_orphan_package_json() {
        let h = tempdir().unwrap();
        std::fs::write(h.path().join("package.json"), r#"{"dependencies":{"@noctuacore/agent-bar":"^4.0.0"}}"#).unwrap();
        let f = scan(h.path());
        assert!(f.package_json_orphan && !f.package_json_mixed);
    }

    #[test]
    fn scan_flags_mixed_package_json() {
        let h = tempdir().unwrap();
        std::fs::write(h.path().join("package.json"), r#"{"dependencies":{"@noctuacore/agent-bar":"^4.0.0","other":"1.0.0"}}"#).unwrap();
        let f = scan(h.path());
        assert!(!f.package_json_orphan && f.package_json_mixed);
    }

    #[test]
    fn scan_ignores_unrelated_package_json() {
        let h = tempdir().unwrap();
        std::fs::write(h.path().join("package.json"), r#"{"dependencies":{"other":"1.0.0"}}"#).unwrap();
        let f = scan(h.path());
        assert!(!f.package_json_orphan && !f.package_json_mixed);
    }

    #[test]
    fn scan_detects_node_modules() {
        let h = tempdir().unwrap();
        std::fs::create_dir_all(h.path().join("node_modules").join("@noctuacore").join("agent-bar")).unwrap();
        let f = scan(h.path());
        assert_eq!(f.node_modules_dir, Some(h.path().join("node_modules").join("@noctuacore").join("agent-bar")));
    }

    #[test]
    fn scan_lockfiles_only_when_orphan_or_missing() {
        let h = tempdir().unwrap();
        std::fs::write(h.path().join("bun.lock"), "").unwrap();
        std::fs::write(h.path().join("package-lock.json"), "{}").unwrap();
        let f = scan(h.path());
        assert_eq!(f.lockfiles, vec![h.path().join("bun.lock"), h.path().join("package-lock.json")]);
        std::fs::write(h.path().join("package.json"), r#"{"dependencies":{"other":"1.0.0"}}"#).unwrap();
        assert!(scan(h.path()).lockfiles.is_empty());
    }

    #[test]
    fn scan_considers_dev_dependencies() {
        let h = tempdir().unwrap();
        std::fs::write(h.path().join("package.json"), r#"{"devDependencies":{"@noctuacore/agent-bar":"^4.0.0"}}"#).unwrap();
        assert!(scan(h.path()).package_json_orphan);
    }

    #[test]
    fn run_doctor_clean() {
        let h = tempdir().unwrap();
        let r = run_doctor(DoctorOptions { home: h.path(), dry_run: false, yes: false, confirm: &|_| true });
        assert!(matches!(r.status, DoctorStatus::Clean));
        assert!(r.removed.is_empty());
    }

    #[test]
    fn run_doctor_removes_orphan_set_when_confirmed() {
        let h = tempdir().unwrap();
        std::fs::write(h.path().join("package.json"), r#"{"dependencies":{"@noctuacore/agent-bar":"^4.0.0"}}"#).unwrap();
        std::fs::write(h.path().join("bun.lock"), "").unwrap();
        std::fs::create_dir_all(h.path().join("node_modules").join("@noctuacore").join("agent-bar")).unwrap();
        let r = run_doctor(DoctorOptions { home: h.path(), dry_run: false, yes: false, confirm: &|_| true });
        assert!(matches!(r.status, DoctorStatus::Cleaned));
        assert!(!h.path().join("package.json").exists());
        assert!(!h.path().join("bun.lock").exists());
        assert!(!h.path().join("node_modules").join("@noctuacore").join("agent-bar").exists());
    }

    #[test]
    fn run_doctor_mixed_keeps_package_json() {
        let h = tempdir().unwrap();
        std::fs::write(h.path().join("package.json"), r#"{"dependencies":{"@noctuacore/agent-bar":"^4.0.0","other":"1.0.0"}}"#).unwrap();
        std::fs::write(h.path().join("bun.lock"), "").unwrap();
        std::fs::create_dir_all(h.path().join("node_modules").join("@noctuacore").join("agent-bar")).unwrap();
        let r = run_doctor(DoctorOptions { home: h.path(), dry_run: false, yes: false, confirm: &|_| true });
        assert!(matches!(r.status, DoctorStatus::MixedOnly));
        assert_eq!(r.removed, vec![h.path().join("node_modules").join("@noctuacore").join("agent-bar")]);
        assert!(h.path().join("package.json").exists());
        assert!(h.path().join("bun.lock").exists());
    }

    #[test]
    fn run_doctor_cancelled() {
        let h = tempdir().unwrap();
        std::fs::write(h.path().join("package.json"), r#"{"dependencies":{"@noctuacore/agent-bar":"^4.0.0"}}"#).unwrap();
        let r = run_doctor(DoctorOptions { home: h.path(), dry_run: false, yes: false, confirm: &|_| false });
        assert!(matches!(r.status, DoctorStatus::Cancelled));
        assert!(h.path().join("package.json").exists());
    }

    #[test]
    fn run_doctor_dry_run_reports_without_removing() {
        let h = tempdir().unwrap();
        std::fs::write(h.path().join("package.json"), r#"{"dependencies":{"@noctuacore/agent-bar":"^4.0.0"}}"#).unwrap();
        std::fs::write(h.path().join("bun.lock"), "").unwrap();
        std::fs::create_dir_all(h.path().join("node_modules").join("@noctuacore").join("agent-bar")).unwrap();
        let r = run_doctor(DoctorOptions { home: h.path(), dry_run: true, yes: false, confirm: &|_| true });
        assert!(matches!(r.status, DoctorStatus::Cleaned));
        assert_eq!(r.removed.len(), 3);
        assert!(h.path().join("package.json").exists());
    }

    #[test]
    fn run_doctor_yes_skips_confirm() {
        let h = tempdir().unwrap();
        std::fs::write(h.path().join("package.json"), r#"{"dependencies":{"@noctuacore/agent-bar":"^4.0.0"}}"#).unwrap();
        let mut called = false;
        let r = run_doctor(DoctorOptions { home: h.path(), dry_run: false, yes: true, confirm: &|_| { /* não deve ser chamado */ false } });
        assert!(matches!(r.status, DoctorStatus::Cleaned));
        assert!(!h.path().join("package.json").exists());
        let _ = called;
    }
}
```

*(O teste `yes_skips_confirm` no TS verifica que o callback não foi chamado. Em Rust, capturar via `Cell<bool>` num closure: `let called = std::cell::Cell::new(false); confirm: &|_| { called.set(true); false }` e assert `!called.get()`. Ajustar o teste para usar `Cell`.)*

- [ ] **Step 2: Run → fail.** `cargo test --manifest-path rust/Cargo.toml doctor 2>&1 | tail -8`

- [ ] **Step 3: Implement** (port de `src/doctor.ts`):
  - `read_json(path) -> Option<serde_json::Value>` (parse leniente, `None` em erro).
  - `classify_package_json(pkg: Option<&Value>) -> (bool orphan, bool mixed)`: merge `dependencies` + `devDependencies` (keys); se `TARGET_PACKAGE` ausente → `(false,false)`; se único → `(true,false)`; senão `(false,true)`.
  - `find_node_modules_dir(home) -> Option<PathBuf>`: `home/node_modules/@noctuacore/agent-bar` se `is_dir`.
  - `find_lockfiles(home, classification) -> Vec<PathBuf>`: se `mixed`/`legit` → `[]`; senão `LOCKFILE_NAMES.filter(exists)`.
  - `scan(home)`: monta `classification` (`orphan`/`mixed`/`legit`/`none`), retorna `DoctorFindings`. **`package_json_path = Some` só quando `pkg` parseou** (TS: `pkg !== null ? path : null`).
  - `planned_removals(findings) -> Vec<PathBuf>`: orphan→package_json; node_modules; se `!mixed`→lockfiles.
  - `run_doctor`: `scan`; se nada → `Clean`; `approved = yes || confirm(&findings)`; se `!approved` → `Cancelled`; `removals = planned_removals`; se `!dry_run` → `rm -rf` cada; `status = if mixed && !orphan { MixedOnly } else { Cleaned }`.

- [ ] **Step 4: Run → pass.** `cargo test --manifest-path rust/Cargo.toml doctor 2>&1 | tail -8` + clippy.

- [ ] **Step 5: `cargo fmt` + commit**

```bash
cargo fmt --manifest-path rust/Cargo.toml
git add rust/src/doctor.rs rust/src/lib.rs
git commit -m "feat(rust): doctor scan + limpeza de leftovers"
```

---

### Task 5: `update.rs` — detect_install_kind + managed/npm update (seams)

**Files:**
- Create: `rust/src/update.rs`
- Modify: `rust/src/lib.rs` (`pub mod update;`)
- Test: inline (portar `tests/update.test.ts`, 11 casos)

**Interfaces:**
- Consumes: `runtime::is_system_install`; `std::path::Path`.
- Produces:
  - `InstallKind` enum `{ ManagedGit, DevGit, Npm, System }`
  - `detect_install_kind(repo_root: &Path, install_root: &Path) -> InstallKind`
  - `is_managed_install_root(repo_root, install_root) -> bool`
  - `CommandResult { ok: bool, output: String }`
  - tipo seam `CommandRunner`: `Fn(&str /*cmd*/, &[&str] /*args*/, &Path /*cwd*/) -> CommandResult` — **async no TS, mas as funções puras testadas usam um runner síncrono mockado**. Decisão: modelar `CommandRunner` como trait object síncrono `&dyn Fn(&str, &[String], &Path) -> CommandResult` (o runner real do main usa `std::process::Command` bloqueante — aceitável; `update` não está no hot-path async do Waybar). Assim os testes não precisam de tokio.
  - `UpdateSummary { repo_root, install_root, current_commit, current_branch, upstream, commits: Vec<String>, local_changes: Vec<String>, has_updates, has_local_changes, dependency_files_changed, needs_dependency_install }`
  - `ManagedUpdateStatus` enum `{ WrongRoot, UpToDate, Cancelled, Updated }`
  - `ManagedUpdateResult { status, repo_root, install_root, summary: Option<UpdateSummary>, installed_dependencies: bool }`
  - `run_managed_update(opts: ManagedUpdateOptions) -> anyhow::Result<ManagedUpdateResult>`
  - `NpmUpdateSummary { package_name, current_version }`, `NpmUpdateStatus { Cancelled, Updated }`, `NpmUpdateResult { status, summary }`
  - `run_npm_update(opts: NpmUpdateOptions) -> anyhow::Result<NpmUpdateResult>`

**Seams (struct de opções com closures):**
```rust
pub struct ManagedUpdateOptions<'a> {
    pub repo_root: &'a Path,
    pub install_root: &'a Path,
    pub run_command: &'a dyn Fn(&str, &[String], &Path) -> CommandResult,
    pub run_setup: &'a dyn Fn(),               // side-effect; testes contam invocações via Cell
    pub confirm: &'a dyn Fn(&UpdateSummary) -> bool,
}
```
(NpmUpdateOptions análogo com `confirm_npm`.)

**Constantes:** `DEPENDENCY_FILES = ["package.json", "bun.lock", "bun.lockb"]`. `UpdateCommandError`: ao falhar um comando, `anyhow::bail!("{step} failed{}", if output.trim().is_empty() {""} else {format!(": {}", output.trim())})`.

- [ ] **Step 1: Write failing tests** (port de `tests/update.test.ts`, 11 casos). O fake runner registra `(cmd, args)` e devolve output mapeado por `"cmd args"`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;
    use std::collections::HashMap;
    use tempfile::tempdir;

    struct Fake {
        outputs: HashMap<String, String>,
        commands: RefCell<Vec<(String, Vec<String>)>>,
    }
    impl Fake {
        fn new(pairs: &[(&str, &str)]) -> Self {
            Fake { outputs: pairs.iter().map(|(k,v)| (k.to_string(), v.to_string())).collect(), commands: RefCell::new(vec![]) }
        }
        fn run(&self, cmd: &str, args: &[String], _cwd: &Path) -> CommandResult {
            self.commands.borrow_mut().push((cmd.to_string(), args.to_vec()));
            let key = format!("{} {}", cmd, args.join(" "));
            CommandResult { ok: true, output: self.outputs.get(&key).cloned().unwrap_or_default() }
        }
    }

    #[test]
    fn managed_aborts_outside_install_root() {
        let fake = Fake::new(&[]);
        let r = run_managed_update(ManagedUpdateOptions {
            repo_root: Path::new("/tmp/dev/agent-bar"),
            install_root: Path::new("/home/test/.agent-bar"),
            run_command: &|c,a,p| fake.run(c,a,p),
            run_setup: &|| {},
            confirm: &|_| true,
        }).unwrap();
        assert!(matches!(r.status, ManagedUpdateStatus::WrongRoot));
        assert!(fake.commands.borrow().is_empty());
    }

    // ... portar os outros: discards+resets+installs (assert sequência completa de comandos),
    // skips bun install quando deps inalteradas + node_modules existe,
    // cancelled quando confirm=false (sem reset/clean),
    // up-to-date quando sem commits e clean (confirm não chamado → usar Cell+panic? no: usar &|_| unreachable!()),
    // detect_install_kind managed/dev/npm/system, runNpmUpdate updated/cancelled.
}
```

**Nota sobre o teste de sequência exata** (`discards local changes...`): o TS asserta `commands` igual a uma lista exata de `["git", [args...]]`. Portar fielmente — é o contrato mais forte. Sequência: `rev-parse --git-dir`, `rev-parse --short HEAD`, `branch --show-current`, `fetch --prune origin`, `rev-parse --abbrev-ref --symbolic-full-name @{u}`, `log --oneline HEAD..origin/master -10`, `status --short`, `diff --name-only HEAD origin/master -- package.json bun.lock bun.lockb`, `reset --hard origin/master`, `clean -fd`, `bun install`.

**Nota `resolveUpstream`:** o `@{u}` usa `runCommand` direto (não `requireCommand`); se `ok && output.trim()` não-vazio → usa; senão `requireCommand(... rev-parse --verify origin/master)`. No fake, a key `git rev-parse --abbrev-ref --symbolic-full-name @{u}` devolve `origin/master\n`.

**Nota `run_setup` count:** usar `std::cell::Cell<u32>` no teste, `run_setup: &|| count.set(count.get()+1)`.

- [ ] **Step 2: Run → fail.** `cargo test --manifest-path rust/Cargo.toml update 2>&1 | tail -8`

- [ ] **Step 3: Implement** (port de `src/update.ts:113-286`):
  - `is_managed_install_root`: `canonicalize`-free — o TS usa `resolve()` (normaliza sem tocar fs). Em Rust, comparar paths normalizados. Como os testes passam paths absolutos limpos (tempdirs), comparar `repo_root == install_root` após `Path::components` normalization simples. **Para fidelidade ao `resolve()`**, implementar `fn resolve(p: &Path) -> PathBuf` que limpa `.`/`..`/duplo-sep sem fs (igual `path.resolve`). Premissa: paths já absolutos nos testes → comparar diretamente `repo_root == install_root` é suficiente para os casos testados; documentar.
  - `detect_install_kind`: se `is_system_install()` → `System`; se `!repo_root.join(".git").exists()` → `Npm`; senão `is_managed_install_root ? ManagedGit : DevGit`.
  - `require_command(runner, cwd, step, cmd, args) -> Result<String>`: roda; se `!ok` → `bail!` (msg acima); senão `Ok(output.trim())`.
  - `resolve_upstream(runner, repo_root) -> Result<String>`: como nota acima.
  - `split_lines(output) -> Vec<String>`: `split('\n').map(trim).filter(non-empty)`.
  - `run_managed_update`: port de `:161-252` — wrong-root early; sequência de comandos; monta `UpdateSummary`; `needs_dependency_install = dependency_files_changed || !repo_root.join("node_modules").exists()`; se `!has_updates && !has_local_changes` → `UpToDate` (sem chamar confirm); `confirm`; se `!approved` → `Cancelled`; `reset --hard upstream` + `clean -fd`; se `needs_dependency_install` → `bun install` + `installed=true`; `run_setup()`; `Updated`.
  - `read_package_info(repo_root) -> Result<NpmUpdateSummary>`: lê `package.json`, exige `name`+`version`; `bail!("package.json is missing name or version")`.
  - `run_npm_update`: port de `:265-286` — `read_package_info`; `confirm_npm`; se `!approved` → `Cancelled`; `bun add -g {name}`; `run_setup()`; `Updated`.

- [ ] **Step 4: Run → pass.** `cargo test --manifest-path rust/Cargo.toml update 2>&1 | tail -8` + clippy.

- [ ] **Step 5: `cargo fmt` + commit**

```bash
cargo fmt --manifest-path rust/Cargo.toml
git add rust/src/update.rs rust/src/lib.rs
git commit -m "feat(rust): update (detect kind + managed/npm)"
```

---

### Task 6: `term_prompt.rs` + `setup.rs` + `uninstall.rs`

**Files:**
- Create: `rust/src/term_prompt.rs`
- Create: `rust/src/setup.rs`
- Create: `rust/src/uninstall.rs`
- Modify: `rust/src/lib.rs` (`pub mod setup; pub mod term_prompt; pub mod uninstall;`)
- Test: inline em `setup.rs` (system-install path) + `term_prompt.rs` (parse de resposta).

**Interfaces:**
- Consumes: `runtime::is_system_install`; `waybar_contract::{get_default_waybar_asset_paths, install_waybar_assets}`; `waybar_integration::{apply_waybar_integration, remove_waybar_integration, get_default_waybar_integration_paths, ApplyOptions, WaybarIntegrationPaths}`; `doctor::scan`; `settings::Settings`; `config::Paths`; `app_identity::APP_NAME`; `theme`.
- Produces:
  - `term_prompt::confirm(message: &str, default_yes: bool) -> bool` (lê stdin; em não-TTY/EOF retorna `default_yes`)
  - `term_prompt::parse_answer(input: &str, default_yes: bool) -> bool` (puro, testável: `y`/`yes`→true, `n`/`no`→false, vazio→default)
  - `term_prompt::status(label: &str, msg: &str)` / `term_prompt::note(...)` (escrevem em **stderr** com cor temática; gate `NO_COLOR`)
  - `setup::reload_waybar()` (spawn `pkill -SIGUSR2 waybar`, ignora erro)
  - `setup::create_symlink(home: &Path) -> std::io::Result<PathBuf>` (symlink `~/.local/bin/agent-bar` → `repo_root/scripts/agent-bar` — só dev)
  - `setup::SetupConfig<'a>` com seams (paths/asset-source/skip-reload) p/ teste
  - `setup::run_setup(settings: &Settings, cfg: SetupConfig, confirm: bool, clear_screen: bool) -> anyhow::Result<bool>`
  - `uninstall::run_uninstall(settings_dir: &Path, cache_dir: &Path, home: &Path, force: bool, title: &str, integration_paths: &WaybarIntegrationPaths) -> anyhow::Result<()>`

**Design de `run_setup` testável:** o TS chama `getDefaultWaybarAssetPaths()`/`getDefaultWaybarIntegrationPaths()` e muta `~`. Para testar o caminho **system-install** sem tocar o desktop, `SetupConfig` injeta: `asset_paths: Option<WaybarAssetPaths>`, `integration_paths: Option<WaybarIntegrationPaths>`, `repo_root: Option<PathBuf>` (passa a `install_waybar_assets`), e `skip_reload: bool`. O teste seta `AGENT_BAR_FORCE_COMPILED=1`, injeta tempdirs e um repo_root-fixture com `icons/`+`scripts/agent-bar-open-terminal`, e verifica: retorna `true`, **não** cria symlink, `app_bin == "agent-bar"`. Espelha `tests/setup.test.ts`.

- [ ] **Step 1: Write failing tests (`term_prompt`)**

```rust
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn parse_answer_variants() {
        assert!(parse_answer("y", false));
        assert!(parse_answer("Yes", false));
        assert!(!parse_answer("n", true));
        assert!(!parse_answer("NO", true));
        assert!(parse_answer("", true));      // vazio → default
        assert!(!parse_answer("", false));
        assert!(!parse_answer("garbage", false)); // não-reconhecido → default
        assert!(parse_answer("  yes  ", false)); // trim
    }
}
```

`parse_answer`: `let t = input.trim().to_lowercase(); match t.as_str() { "y"|"yes" => true, "n"|"no" => false, "" => default_yes, _ => default_yes }`.

- [ ] **Step 2: Write failing test (`setup` system path)** — port de `tests/setup.test.ts`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Paths;
    use crate::settings::load;
    use std::path::PathBuf;
    use tempfile::tempdir;

    #[test]
    #[serial_test::serial]
    fn setup_system_install_skips_symlink_uses_path_appbin() {
        let repo = tempdir().unwrap(); // fixture de assets
        std::fs::create_dir_all(repo.path().join("icons")).unwrap();
        std::fs::write(repo.path().join("icons").join("a.png"), b"x").unwrap();
        std::fs::create_dir_all(repo.path().join("scripts")).unwrap();
        std::fs::write(repo.path().join("scripts").join("agent-bar-open-terminal"), b"#!/bin/sh\n").unwrap();

        let dest = tempdir().unwrap();
        let asset_paths = crate::waybar_contract::WaybarAssetPaths {
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
            cache_dir: dest.path().join("c"), config_dir: dest.path().join("cfg"),
            claude_credentials: PathBuf::new(), codex_auth: PathBuf::new(),
            codex_sessions: PathBuf::new(), amp_settings: PathBuf::new(), amp_threads: PathBuf::new(),
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
        assert!(!dest.path().join(".local").join("bin").join("agent-bar").exists());
    }
}
```

**Decisão:** `SetupConfig` ganha `system_install: bool` explícito (em vez de chamar `is_system_install()` dentro de `run_setup`) para o teste não depender de env global. O `main` (T7) passa `system_install: runtime::is_system_install()`.

- [ ] **Step 3: Run → fail.** `cargo test --manifest-path rust/Cargo.toml term_prompt 2>&1 | tail -8` e `... setup 2>&1 | tail -8`

- [ ] **Step 4: Implement `term_prompt.rs`**
  - `confirm(message, default_yes)`: se `!stdin().is_terminal()` → retorna `default_yes` (não bloqueia em pipe). Senão imprime `message` + ` [y/N] ` (ou `[Y/n]`) em stderr, lê uma linha de stdin, `parse_answer`.
  - `status`/`note`: `eprintln!` com cor (`theme::ColorToken::*.ansi()` se `!NO_COLOR`).

- [ ] **Step 5: Implement `setup.rs`**
  - `reload_waybar()`: `let _ = std::process::Command::new("pkill").args(["-SIGUSR2","waybar"]).stdout(Stdio::null()).stderr(Stdio::null()).spawn();`.
  - `create_symlink(home)`: port de `createSymlink` — `local_bin = home/.local/bin`; `mkdir -p`; `link = local_bin/agent-bar`; `target = repo_root/scripts/agent-bar` (repo_root = `env!("CARGO_MANIFEST_DIR")/..` ou injetado); `let _ = remove_file(&link)`; `std::os::unix::fs::symlink(target, &link)`; retorna `link`. **No teste system-install, NÃO é chamado.**
  - `run_setup(settings, cfg, confirm, clear_screen)`: port de `runSetup` (`src/setup.ts:49-177`), versão terminal-lite:
    1. (clear_screen ignorado ou `print!("\x1b[2J\x1b[H")` se TTY — manter simples: no-op em teste).
    2. `note` descrevendo as ações (stderr).
    3. se `confirm` → `term_prompt::confirm("Apply {APP_NAME} setup now?", true)`; se `false` → retorna `Ok(false)`.
    4. `install_waybar_assets(asset_paths.waybar_dir, asset_paths.scripts_dir, cfg.repo_root.as_deref())`.
    5. se `!cfg.system_install` → `create_symlink(&cfg.home)`.
    6. `apply_waybar_integration(settings, ApplyOptions { paths: integration_paths, icons_dir: Some(asset.icons_dir), app_bin: Some(asset.app_bin), terminal_script: Some(asset.terminal_script) })`.
    7. se `!cfg.skip_reload` → `reload_waybar()`.
    8. status de sucesso (stderr); scan de leftovers (`doctor::scan(&cfg.home)`) e warn se houver (best-effort, nunca falha o setup).
    9. retorna `Ok(true)`.
  - **Erros:** propagar via `?`/`anyhow::Result`; o caller (T7) imprime e sai 1.

- [ ] **Step 6: Implement `uninstall.rs`**
  - `run_uninstall(settings_dir, cache_dir, home, force, title, integration_paths)`: port de `runUninstall` (`src/uninstall.ts:47-129`):
    1. `note` listando o que será removido (stderr).
    2. se `!force` → `term_prompt::confirm("Continue with uninstall?", false)`; se `false` → retorna `Ok(())`.
    3. `remove_waybar_integration(integration_paths)`.
    4. `remove_path_if_exists` para: `asset_paths.waybar_dir`, `asset_paths.terminal_script`, `settings_dir`, `cache_dir`, `home/.local/bin/agent-bar`. (asset paths via `get_default_waybar_asset_paths()` ou injetados.)
    5. se config/style changed → `reload_waybar()` (gate por seam skip_reload no teste se necessário; uninstall não tem teste obrigatório, manter simples).
    6. status final.
  - `remove` (T7) chama `run_uninstall(..., force=true, title="agent-bar remove")`.

- [ ] **Step 7: Run → pass.** `cargo test --manifest-path rust/Cargo.toml setup 2>&1 | tail -8` (+ `term_prompt`) + clippy.

- [ ] **Step 8: `cargo fmt` + commit**

```bash
cargo fmt --manifest-path rust/Cargo.toml
git add rust/src/term_prompt.rs rust/src/setup.rs rust/src/uninstall.rs rust/src/lib.rs
git commit -m "feat(rust): setup/uninstall + prompt de terminal"
```

---

### Task 7: `main.rs` — wiring dos comandos de install

**Files:**
- Modify: `rust/src/main.rs:92-133` (substituir os 8 stubs; `Menu` continua stub)
- Test: inline (gates já existentes) + smokes manuais documentados.

**Interfaces:**
- Consumes: tudo de T1-T6 + `cli::CliOptions`.
- Produces: dispatch real. Sem nova API pública.

**Dispatch a implementar (substituir cada `log::error!`+exit1):**

- `Command::AssetsInstall`: port de `index.ts:69-77`:
  ```rust
  let defaults = waybar_contract::get_default_waybar_asset_paths();
  let waybar_dir = opts.waybar_dir.as_deref().map(PathBuf::from).unwrap_or(defaults.waybar_dir);
  let scripts_dir = opts.scripts_dir.as_deref().map(PathBuf::from).unwrap_or(defaults.scripts_dir);
  match waybar_contract::install_waybar_assets(&waybar_dir, &scripts_dir, None) {
      Ok(r) => { println!("{}", serde_json::to_string(&r).unwrap_or_default()); std::process::exit(0); }
      Err(e) => { log::error!("{e}"); std::process::exit(1); }
  }
  ```
  *(JSON compacto, stdout.)*

- `Command::ExportWaybarModules`: port de `index.ts:79-97`:
  ```rust
  let defaults = waybar_contract::get_default_waybar_asset_paths();
  let app_bin = opts.app_bin.clone().unwrap_or(defaults.app_bin);
  let terminal_script = opts.terminal_script.as_deref().map(PathBuf::from).unwrap_or(defaults.terminal_script);
  let term_str = terminal_script.to_string_lossy().to_string();
  let export = waybar_contract::export_waybar_modules(&app_bin, &term_str, settings.waybar.signal, &settings.waybar.provider_order);
  println!("{}", serde_json::to_string_pretty(&export).unwrap_or_default());
  std::process::exit(0);
  ```
  **Nota:** precisa de `settings` carregado. Mover a construção de `Paths`/`settings` para ANTES do match de comandos de install (ou carregar localmente). **Decisão:** carregar `paths`+`settings` no topo do handler de cada export/assets (são one-shot; sem custo). Reordenar: tirar AssetsInstall/ExportWaybar* do bloco de short-circuit cedo e tratá-los APÓS a construção de Ctx? Não — eles não precisam de Ctx (sem HTTP). Carregar `let paths = Paths::from_env()?; let settings = settings::load(&paths);` localmente no arm. Como o match atual é antes da construção de Ctx, fazer load local nesses arms.

- `Command::ExportWaybarCss`: port de `index.ts:99-114`:
  ```rust
  let defaults = waybar_contract::get_default_waybar_asset_paths();
  let icons_dir = opts.icons_dir.as_deref().map(PathBuf::from).unwrap_or(defaults.icons_dir);
  let css = waybar_contract::export_waybar_css(&icons_dir.to_string_lossy(), &settings.waybar.provider_order, settings.waybar.separators);
  // o TS imprime JSON.stringify({css}, null, 2)
  let wrapped = serde_json::json!({ "css": css });
  println!("{}", serde_json::to_string_pretty(&wrapped).unwrap_or_default());
  std::process::exit(0);
  ```

- `Command::Setup`: port de `index.ts:63-67`:
  ```rust
  let paths = Paths::from_env()...; let settings = settings::load(&paths);
  let asset_paths = waybar_contract::get_default_waybar_asset_paths();
  let ipaths = waybar_integration::get_default_waybar_integration_paths();
  let home = std::env::var_os("HOME").map(PathBuf::from).unwrap_or_default();
  let cfg = setup::SetupConfig { asset_paths: Some(asset_paths), integration_paths: Some(ipaths), repo_root: None, home, skip_reload: false, system_install: runtime::is_system_install() };
  match setup::run_setup(&settings, cfg, true, true) { Ok(_) => exit(0), Err(e) => { log::error!("{e}"); exit(1) } }
  ```

- `Command::Doctor`: port de `index.ts:117-121`:
  ```rust
  let home = std::env::var_os("HOME").map(PathBuf::from).unwrap_or_default();
  let confirm = |f: &doctor::DoctorFindings| term_prompt::confirm(if opts.dry_run {"Show what would be removed?"} else {"Remove the leftovers above?"}, true);
  // descrever findings (note) antes; usar confirm seam
  let result = doctor::run_doctor(doctor::DoctorOptions { home: &home, dry_run: opts.dry_run, yes: opts.yes, confirm: &confirm });
  // imprimir outro conforme status (stderr)
  std::process::exit(0);
  ```
  *(Imprimir os findings via `term_prompt::note` antes do confirm; outro final por status.)*

- `Command::Update`: port de `index.ts:124-128` + a lógica do `update::main` (`src/update.ts:348-446`):
  ```rust
  let repo_root = update::repo_root(); // env!(CARGO_MANIFEST_DIR)/..
  match update::detect_install_kind(&repo_root, &update::default_install_root()) {
      InstallKind::System => { /* info: use AUR helper */ }
      InstallKind::DevGit => { /* error: dev checkout, use git pull */ }
      InstallKind::Npm => { /* run_npm_update interactive */ }
      InstallKind::ManagedGit => { /* run_managed_update interactive */ }
  }
  ```
  *(O runner real = `std::process::Command` síncrono; `run_setup` re-aplica via `setup::run_setup(&settings, cfg, false, false)` — confirm=false, clear=false.)*

- `Command::Uninstall`: port de `index.ts:131-135`:
  ```rust
  let ipaths = waybar_integration::get_default_waybar_integration_paths();
  let asset = waybar_contract::get_default_waybar_asset_paths();
  let home = ...; let settings_dir = home.join(".config").join(APP_NAME); let cache = paths.cache_dir.clone();
  uninstall::run_uninstall(&settings_dir, &cache, &home, false, &format!("{APP_NAME} uninstall"), &ipaths)?;
  std::process::exit(0);
  ```

- `Command::Remove`: port de `index.ts:137-141` → `run_uninstall(..., force=true, title="{APP_NAME} remove", ...)`.

- `Command::Menu`: **continua stub** (Plano 7 TUI). Trocar a msg p/ apontar Plano 7:
  ```rust
  log::error!("'menu' abre a TUI (Plano 7) — ainda não implementado.");
  std::process::exit(1);
  ```

**Reorganização do match:** os arms de install precisam de `paths`/`settings` que hoje são construídos DEPOIS do match (linha 136+). **Decisão:** dentro de cada arm de install, construir `let paths = match Paths::from_env() {...}; let settings = settings::load(&paths);` localmente (one-shot, sem Ctx HTTP). Não mover o Ctx global. Manter os arms `Help`/`Version`/`Menu` como estão.

- [ ] **Step 1: Implement os arms** (substituir os 8 `log::error!` por dispatch real; ler `main.rs` antes de editar). Manter os testes de gate existentes (`should_notify`/`is_hidden_module`/`print_waybar`) intactos.

- [ ] **Step 2: `cargo build` + `cargo clippy`**

Run: `cargo build --manifest-path rust/Cargo.toml 2>&1 | tail -5` e `cargo clippy --manifest-path rust/Cargo.toml --all-targets -- -D warnings 2>&1 | tail -5`
Expected: build ok, `cargo clippy: No issues found`.

- [ ] **Step 3: Smokes end-to-end** (binário compilado, em tempdirs — NÃO tocar `~/.config` real):

```bash
BIN=rust/target/debug/agent-bar
cargo build --manifest-path rust/Cargo.toml 2>&1 | tail -2
# export-waybar-modules → JSON pretty com {providers, modules}
AGENT_BAR_FORCE_COMPILED=1 $BIN export-waybar-modules | head -20
# export-waybar-css → {"css": "..."} pretty
AGENT_BAR_FORCE_COMPILED=1 $BIN export-waybar-css | head -10
# assets-install em tempdir (paths injetados) → JSON compacto {iconsDir,terminalScript}
TMP=$(mktemp -d); AGENT_BAR_ASSET_DIR=$(pwd) $BIN assets-install --waybar-dir $TMP/wb --scripts-dir $TMP/sc
# doctor --dry-run num HOME tempdir vazio → "Nothing to clean"
HOME=$(mktemp -d) $BIN doctor --dry-run --yes
rm -rf $TMP
```
Expected: JSON válido nos três; doctor reporta limpo. Verificar ordem de chaves do módulo (`exec` primeiro, `signal` ausente sem setting).

- [ ] **Step 4: Rodar a suíte inteira** (garantir zero regressão):

Run: `cargo test --manifest-path rust/Cargo.toml 2>&1 | tail -10`
Expected: todos os testes passam (378 anteriores + novos de T1-T7).

- [ ] **Step 5: `cargo fmt` + commit**

```bash
cargo fmt --manifest-path rust/Cargo.toml
git add rust/src/main.rs
git commit -m "feat(rust): wiring dos comandos de install no main"
```

---

## Self-Review (autor)

**1. Spec coverage** — comandos do `index.ts`/resume §4:
- `assets-install` → T2+T7 ✓; `export-waybar-modules`/`-css` → T1+T7 ✓; `setup` → T6+T7 ✓; `doctor` → T4+T7 ✓; `update` → T5+T7 ✓; `uninstall`/`remove` → T6+T7 ✓; patcher JSONC → T3a+T3b ✓; asset resolution → T1 ✓; `is_system_install` → T1 ✓.
- DESCOPADO consciente: `install.rs` (ensure_command/amp) → Plano 7; `menu` → stub (Plano 7). Documentado.

**2. Placeholder scan** — sem TBD/TODO; cada arm de T7 tem código; primitivas T3a têm código completo; testes T4/T5 listam casos (alguns "portar os demais" com a sequência exata especificada — o implementer tem o `.test.ts` como fonte). **Risco:** T4/T5 dizem "portar TODOS os 14/11 casos" sem escrever cada um. Mitigação: o brief deve apontar o `.test.ts` exato + a tabela de `(cmd,args)` para o caso de sequência. Os exemplos dados cobrem os shapes; o implementer replica o resto 1:1 do arquivo TS.

**3. Type consistency** — `WaybarAssetPaths`/`WaybarIntegrationPaths`/`ApplyOptions`/`ApplyResult`/`InstalledAssets`/`DoctorFindings`/`UpdateSummary`/`SetupConfig` nomeados consistentemente entre T1/T2/T3b/T6/T7. `export_waybar_css` retorna `String` (caller embrulha `{"css":...}`). `export_waybar_modules` retorna `WaybarModulesExport`. `is_system_install` (runtime). `normalize_provider_selection` reusado de `settings.rs`. ✓

**Riscos conhecidos (para o reviewer):**
- **R1 — premissa ASCII no patcher JSONC** (T3a): byte-index assume estrutura ASCII; valores não-ASCII ficam em strings (puladas). Igual ao TS. Documentado no header do módulo.
- **R2 — `pathToFileURL` simplificado** (T1 `export_waybar_css`): `file://` + path sem percent-encoding. Fiel ao uso real (paths ASCII de `~/.config`); divergiria só com espaços/não-ASCII no icons_dir. Os testes não exercem isso.
- **R3 — backreference regex** (T3b `strip_managed_style_imports`): crate `regex` não suporta `\1`; reescrito sem backref (alternância `"`/`'`). Verificar paridade no round-trip test.
- **R4 — `resolve()` de path** (T5): comparação direta de paths absolutos (sem normalização `..`); suficiente p/ os testes (tempdirs limpos). Documentado.
- **R5 — `repo_root()` baked** (T1/T5/T6): `env!("CARGO_MANIFEST_DIR")/..` = raiz pré-cutover; Plano 8 ajusta na promoção.
