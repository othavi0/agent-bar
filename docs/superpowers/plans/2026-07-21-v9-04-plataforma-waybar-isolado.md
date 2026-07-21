# Plataforma Waybar Isolado Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Isolar o Waybar como tier legado gated por plataforma — `src/platform.rs::detect()` único, zero escrita em `~/.config/waybar/` sem Waybar no PATH, `update` reinstala o plugin omarchy quando detectado, e `doctor`/`uninstall` ficam Omarchy-aware.

**Architecture:** Um módulo novo (`src/platform.rs`) expõe `Platform { omarchy, waybar }` via `detect()`; três pontos de escrita (`update` ManagedGit, `update` Standalone, Save da TUI) passam a consultar essa função em vez de hardcodar. `waybar_contract.rs`/`waybar_integration.rs` mudam de diretório (não de identidade de módulo) para `src/waybar/`, preservando os filtros de teste do CLAUDE.md via `#[path]`. `doctor`/`uninstall` ganham checagens/gates Omarchy que só avisam, nunca falham o comando.

**Tech Stack:** Rust/cargo, tokio (`#[tokio::test]`), serde_json, insta (snapshots ratatui), tempfile, serial_test (mocks de env global).

## Global Constraints

- Rust/cargo only; nunca `unwrap()`/`expect()` em produção — propagar com `?`/`anyhow::bail!`.
- Constantes de identidade de `src/app_identity.rs`, nunca strings hardcoded.
- Provider error strings são contrato — este plano não as toca.
- XML-escape só em `render_pango.rs`; nunca round-trip de `config.jsonc` do Waybar via `serde_json`.
- `#[tokio::test]` async / `#[test]` sync; sem rede/CLIs vivas/Waybar real; `XDG_CONFIG_HOME`/`HOME` setados ANTES de qualquer chamada que os leia; restaurar env em todo teste que o mute (padrão save/restore já usado em `setup.rs`/`omarchy_integration.rs`).
- Gotcha RTK: `cargo test` com APENAS UM filtro posicional por invocação.
- Nunca mutar `~/.config/waybar`/`~/.config/agent-bar` reais em teste — sempre temp dirs; funções que leem `$HOME`/`XDG_CONFIG_HOME` direto exigem `#[serial_test::serial]` + save/restore do env.
- Commits: Conventional Commits em PT, subject ≤50 chars. Zero atribuição de AI em qualquer texto.
- Prosa em pt-BR; identificadores/código em inglês.

## Nota sobre o contrato de interfaces (desvio deliberado)

O contrato fixado pelo orquestrador descreve o passo de módulo (Task 6) como
`pub use waybar::contract as waybar_contract;`. Isso quebraria
`cargo test waybar_contract`: o filtro de `cargo test` casa **substring** na
string totalmente qualificada do teste (`waybar::contract::tests::foo`), e
`"waybar_contract"` (com `_`) não é substring de `"waybar::contract"` (com
`::`). A Task 6 abaixo implementa o mesmo objetivo (arquivos físicos em
`src/waybar/{contract,integration}.rs`, `crate::waybar_contract`/
`crate::waybar_integration` continuam resolvendo) só que via `#[path]` +
`pub use waybar::waybar_contract;` (nome do módulo preservado), que é a única
forma de manter os dois filtros do CLAUDE.md passando sem editar os testes.
Ver `openQuestions` do resumo desta tarefa. Desvio APROVADO pelo
orquestrador: a alternativa literal quebraria os filtros
`cargo test waybar_contract`/`cargo test waybar_integration` do CLAUDE.md.

### Task 1: `src/platform.rs` — detecção única de plataforma

**Files:**
- Create: `src/platform.rs`
- Modify: `src/lib.rs:14-16` (insere `pub mod platform;` entre `omarchy_integration` e `providers`)

**Interfaces:**
- Produces: `pub struct Platform { pub omarchy: bool, pub waybar: bool }`, `pub fn detect() -> Platform`, `pub fn detect_with(shell_dir: &Path, path_var: Option<&OsStr>) -> Platform` (núcleo testável de `detect()`).
- Consumes: `omarchy_integration::omarchy_shell_present` (pub, `src/omarchy_integration.rs:37-39`), `setup::waybar_present` (pub, `src/setup.rs:31-36`).

- [ ] Escrever o teste que falha — criar `src/platform.rs` só com o esqueleto de teste:
  ```rust
  //! Detecção única de plataforma (Omarchy-shell / Waybar) usada por todos os
  //! gates de escrita em `~/.config/waybar/` (spec 2026-07-21, seção E): setup,
  //! `update` (ManagedGit + Standalone) e o Save da TUI Config leem `detect()`
  //! em vez de reimplementar a checagem cada um a seu jeito.

  #[cfg(test)]
  mod tests {
      #[test]
      fn detect_with_omarchy_only() {
          panic!("not implemented");
      }
  }
  ```
- [ ] Rodar `cargo test platform` e ver falhar: `panicked at 'not implemented'`.
- [ ] Implementação mínima — substituir o conteúdo inteiro de `src/platform.rs` por:
  ```rust
  //! Detecção única de plataforma (Omarchy-shell / Waybar) usada por todos os
  //! gates de escrita em `~/.config/waybar/` (spec 2026-07-21, seção E): setup,
  //! `update` (ManagedGit + Standalone) e o Save da TUI Config leem `detect()`
  //! em vez de reimplementar a checagem cada um a seu jeito.

  use std::ffi::OsStr;
  use std::path::Path;

  use crate::app_identity::OMARCHY_SHELL_DIR;

  #[derive(Debug, Clone, Copy, PartialEq, Eq)]
  pub struct Platform {
      /// omarchy-shell presente (`OMARCHY_SHELL_DIR` existe + CLI `omarchy` no PATH).
      pub omarchy: bool,
      /// Binário `waybar` no PATH.
      pub waybar: bool,
  }

  /// Núcleo testável: mesma composição de `detect()`, com `shell_dir`/`path_var`
  /// injetáveis — mesmo padrão de mock de
  /// `omarchy_integration::omarchy_shell_present`/`setup::waybar_present`.
  pub fn detect_with(shell_dir: &Path, path_var: Option<&OsStr>) -> Platform {
      Platform {
          omarchy: crate::omarchy_integration::omarchy_shell_present(shell_dir, path_var),
          waybar: crate::setup::waybar_present(path_var),
      }
  }

  /// Detecção real do processo — único ponto de decisão consumido pelos gates
  /// de plataforma (setup, update, TUI Save).
  pub fn detect() -> Platform {
      detect_with(Path::new(OMARCHY_SHELL_DIR), std::env::var_os("PATH").as_deref())
  }

  #[cfg(test)]
  mod tests {
      use super::*;
      use tempfile::tempdir;

      #[test]
      fn detect_with_omarchy_only() {
          let shell = tempdir().unwrap();
          let bin = tempdir().unwrap();
          std::fs::write(bin.path().join("omarchy"), "#!/bin/sh\n").unwrap();
          let path_var = std::ffi::OsString::from(bin.path());

          let platform = detect_with(shell.path(), Some(&path_var));
          assert!(platform.omarchy);
          assert!(!platform.waybar);
      }

      #[test]
      fn detect_with_waybar_only() {
          let shell = tempdir().unwrap(); // dir existe, mas sem CLI `omarchy` no PATH
          let bin = tempdir().unwrap();
          std::fs::write(bin.path().join("waybar"), "#!/bin/sh\n").unwrap();
          let path_var = std::ffi::OsString::from(bin.path());

          let platform = detect_with(shell.path(), Some(&path_var));
          assert!(!platform.omarchy);
          assert!(platform.waybar);
      }

      #[test]
      fn detect_with_neither_present() {
          let shell = tempdir().unwrap();
          let bin = tempdir().unwrap();
          let path_var = std::ffi::OsString::from(bin.path());

          let platform = detect_with(&shell.path().join("nope"), Some(&path_var));
          assert!(!platform.omarchy);
          assert!(!platform.waybar);
      }

      #[test]
      fn detect_with_both_present() {
          let shell = tempdir().unwrap();
          let bin = tempdir().unwrap();
          std::fs::write(bin.path().join("omarchy"), "#!/bin/sh\n").unwrap();
          std::fs::write(bin.path().join("waybar"), "#!/bin/sh\n").unwrap();
          let path_var = std::ffi::OsString::from(bin.path());

          let platform = detect_with(shell.path(), Some(&path_var));
          assert!(platform.omarchy);
          assert!(platform.waybar);
      }
  }
  ```
- [ ] Registrar o módulo — em `src/lib.rs`, linhas 14-16 hoje são:
  ```rust
  pub mod notify;
  pub mod omarchy_integration;
  pub mod providers;
  ```
  Substituir por:
  ```rust
  pub mod notify;
  pub mod omarchy_integration;
  pub mod platform;
  pub mod providers;
  ```
- [ ] Rodar `cargo test platform` e ver passar: 4 testes `ok`.
- [ ] Commit: `git add src/platform.rs src/lib.rs && git commit -m "feat: platform::detect() unico p/ gates Waybar/Omarchy"`

### Task 2: gate no `update` ManagedGit (main.rs)

**Files:**
- Modify: `src/main.rs:25-28` (import), `src/main.rs:50-60` (helper puros, insere função nova), `src/main.rs:396` (declara `let platform = platform::detect();`), `src/main.rs:479-498` (closure `run_setup` do braço `ManagedGit`), `src/main.rs:800-823` (test helpers, sem mudança de conteúdo — só referência), `src/main.rs` test module (novo teste)

**Interfaces:**
- Consumes: `platform::Platform`/`platform::detect()` (Task 1), `setup::SetupConfig`/`setup::OmarchySetupOptions` (`src/setup.rs:67-96`), `omarchy_integration::default_omarchy_plugins_dir` (`src/omarchy_integration.rs:26-32`).
- Produces: `fn managed_update_setup_config(platform, repo_root, home) -> setup::SetupConfig` (testável, usada só por `Command::Update`).

- [ ] Escrever o teste que falha — no `mod tests` de `src/main.rs` (após `settings_without_provider`, linha 840), adicionar:
  ```rust
  #[test]
  fn managed_update_setup_config_gates_on_platform() {
      let home = PathBuf::from("/tmp/agent-bar-test-managed-update-home");
      let repo = PathBuf::from("/tmp/agent-bar-test-managed-update-repo");

      let omarchy_only = managed_update_setup_config(
          platform::Platform {
              omarchy: true,
              waybar: false,
          },
          repo.clone(),
          home.clone(),
      );
      assert!(omarchy_only.omarchy.is_some());
      assert!(omarchy_only.skip_waybar);

      let waybar_only = managed_update_setup_config(
          platform::Platform {
              omarchy: false,
              waybar: true,
          },
          repo,
          home,
      );
      assert!(waybar_only.omarchy.is_none());
      assert!(!waybar_only.skip_waybar);
  }
  ```
- [ ] Rodar `cargo test update` e ver falhar: `error[E0425]: cannot find function 'managed_update_setup_config'`.
- [ ] Implementação mínima — em `src/main.rs:25-28`, hoje:
  ```rust
  use agent_bar::{
      doctor, install, omarchy_integration, runtime, setup, term_prompt, uninstall, update,
      waybar_contract, waybar_integration,
  };
  ```
  Substituir por:
  ```rust
  use agent_bar::{
      doctor, install, omarchy_integration, platform, runtime, setup, term_prompt, uninstall,
      update, waybar_contract, waybar_integration,
  };
  ```
- [ ] Adicionar a função pura logo após `is_hidden_module` (linha 54, antes de `print_waybar`):
  ```rust
  /// `SetupConfig` do update ManagedGit (spec E/F): reinstala o plugin omarchy
  /// quando `platform.omarchy` e só toca `~/.config/waybar/` quando
  /// `platform.waybar` — mesma composição de `platform::detect()` usada pelo
  /// Standalone e pelo Save da TUI.
  fn managed_update_setup_config(
      platform: platform::Platform,
      repo_root: PathBuf,
      home: PathBuf,
  ) -> setup::SetupConfig {
      setup::SetupConfig {
          asset_paths: Some(waybar_contract::get_default_waybar_asset_paths()),
          integration_paths: Some(waybar_integration::get_default_waybar_integration_paths()),
          repo_root: Some(repo_root),
          home: home.clone(),
          skip_reload: false,
          system_install: runtime::is_system_install(),
          omarchy: platform.omarchy.then(|| setup::OmarchySetupOptions {
              plugins_dir: omarchy_integration::default_omarchy_plugins_dir(&home),
              run_cli: true,
          }),
          skip_waybar: !platform.waybar,
      }
  }
  ```
- [ ] Declarar `platform` no topo do braço `Command::Update` — em `src/main.rs:396`, hoje:
  ```rust
              let install_root = home.join(format!(".{APP_NAME}"));
  ```
  Substituir por:
  ```rust
              let install_root = home.join(format!(".{APP_NAME}"));
              let platform = platform::detect();
  ```
  (declarada aqui para que a Task 2 compile sozinha; a Task 3 reusa esta
  mesma variável — já declarada — no closure `omarchy_setup_hint`.)
- [ ] Trocar o corpo do braço `ManagedGit`. Hoje (`src/main.rs:479-498`):
  ```rust
                      let settings_for_setup = settings.clone();
                      let repo_root_for_setup = root.clone();
                      let home_for_setup = home.clone();
                      let run_setup = || {
                          let asset_paths = waybar_contract::get_default_waybar_asset_paths();
                          let ipaths = waybar_integration::get_default_waybar_integration_paths();
                          let cfg = setup::SetupConfig {
                              asset_paths: Some(asset_paths),
                              integration_paths: Some(ipaths),
                              repo_root: Some(repo_root_for_setup.clone()),
                              home: home_for_setup.clone(),
                              skip_reload: false,
                              system_install: runtime::is_system_install(),
                              omarchy: None,
                              skip_waybar: false,
                          };
                          if let Err(e) = setup::run_setup(&settings_for_setup, cfg, false, false) {
                              log::error!("Setup falhou após update: {e}");
                          }
                      };
  ```
  Substituir por:
  ```rust
                      let settings_for_setup = settings.clone();
                      let repo_root_for_setup = root.clone();
                      let home_for_setup = home.clone();
                      let run_setup = || {
                          let cfg = managed_update_setup_config(
                              platform,
                              repo_root_for_setup.clone(),
                              home_for_setup.clone(),
                          );
                          if let Err(e) = setup::run_setup(&settings_for_setup, cfg, false, false) {
                              log::error!("Setup falhou após update: {e}");
                          }
                      };
  ```
  (a variável `platform` já foi declarada acima, nesta mesma task — a Task 3 reusa-a sem redeclarar.)
- [ ] Rodar `cargo test update` e ver passar: novo teste + suíte de `update.rs` `ok`.
- [ ] Commit: `git add -A && git commit -m "feat: update ManagedGit usa platform::detect()"`

### Task 3: gate no `update` Standalone (update.rs + main.rs)

**Files:**
- Modify: `src/update.rs:597-614` (`StandaloneUpdateOptions`), `src/update.rs:684-704` (`run_standalone_update` + `refresh_waybar_assets_from_extract`), `src/update.rs:1338-1350` e `:1374-1386` (2 testes existentes), `src/update.rs` (novos testes ao fim do módulo)
- Modify: `src/main.rs:400-409` (`omarchy_setup_hint` passa a receber `platform`, já declarada na Task 2), `src/main.rs:517-519` (hint pós-ManagedGit), `src/main.rs:553-599` (opts + mensagem do Standalone)

**Interfaces:**
- Consumes: `platform::Platform` (Task 1), `omarchy_integration::install_omarchy_plugin` (`src/omarchy_integration.rs:70-90`).
- Produces: `StandaloneUpdateOptions.skip_waybar: bool`, `StandaloneUpdateOptions.omarchy_plugins_dir: Option<&Path>`, `fn apply_standalone_platform_gates(...)` (testável sem download/checksum reais).

- [ ] Escrever o teste que falha — ao fim do `mod tests` de `src/update.rs` (após `refresh_waybar_assets_from_extract_copies_icons_and_helper`, linha 1318), adicionar:
  ```rust
  #[test]
  fn apply_standalone_platform_gates_skips_waybar_when_absent() {
      let tmp = tempdir().unwrap();
      let extracted = tmp.path().join("extracted");
      fs::create_dir_all(extracted.join("icons")).unwrap();
      fs::write(extracted.join("icons").join("grok-icon.svg"), b"<svg/>").unwrap();

      let waybar_dir = tmp.path().join("waybar-agent-bar");
      let scripts_dir = tmp.path().join("waybar-scripts");

      apply_standalone_platform_gates(&extracted, &waybar_dir, &scripts_dir, true, None).unwrap();

      assert!(
          !waybar_dir.exists(),
          "waybar_dir não deve ser tocado quando skip_waybar=true"
      );
  }

  #[test]
  fn apply_standalone_platform_gates_reinstalls_omarchy_plugin_when_detected() {
      let tmp = tempdir().unwrap();
      let extracted = tmp.path().join("extracted-empty");
      fs::create_dir_all(&extracted).unwrap();
      let waybar_dir = tmp.path().join("waybar-agent-bar");
      let scripts_dir = tmp.path().join("waybar-scripts");
      let plugins_dir = tmp.path().join("omarchy-plugins");

      apply_standalone_platform_gates(
          &extracted,
          &waybar_dir,
          &scripts_dir,
          true,
          Some(plugins_dir.as_path()),
      )
      .unwrap();

      let plugin_dir = plugins_dir.join(crate::app_identity::OMARCHY_PLUGIN_ID);
      assert!(plugin_dir.join("manifest.json").exists());
      assert!(plugin_dir.join("Widget.qml").exists());
  }
  ```
- [ ] Rodar `cargo test update` e ver falhar: `error[E0425]: cannot find function 'apply_standalone_platform_gates'`.
- [ ] Implementação mínima — em `src/update.rs:597-614`, hoje:
  ```rust
  pub struct StandaloneUpdateOptions<'a> {
      pub current_version: &'a str,
      pub exe_path: &'a Path,
      pub data_dir: &'a Path,
      /// Destino dos icons Waybar (`~/.config/waybar/agent-bar` em produção).
      /// Só re-copia icons/helper — **não** patcha config nem dá reload.
      pub waybar_dir: &'a Path,
      /// Destino do terminal helper (`~/.config/waybar/scripts` em produção).
      pub scripts_dir: &'a Path,
      pub run_command: &'a dyn Fn(&str, &[String], &Path) -> CommandResult,
      pub http: &'a reqwest::Client,
      pub releases_api_url: String,
      pub download_base_url: String,
      /// Seams de teste: substituem os checks reais de `sha256sum`/`tar` no PATH
      /// (produção usa `crate::install::has_cmd`).
      pub has_sha256sum: &'a dyn Fn() -> bool,
      pub has_tar: &'a dyn Fn() -> bool,
  }
  ```
  Substituir por:
  ```rust
  pub struct StandaloneUpdateOptions<'a> {
      pub current_version: &'a str,
      pub exe_path: &'a Path,
      pub data_dir: &'a Path,
      /// Destino dos icons Waybar (`~/.config/waybar/agent-bar` em produção).
      /// Só re-copia icons/helper — **não** patcha config nem dá reload.
      pub waybar_dir: &'a Path,
      /// Destino do terminal helper (`~/.config/waybar/scripts` em produção).
      pub scripts_dir: &'a Path,
      /// `true` quando Waybar está ausente do PATH — pula
      /// `refresh_waybar_assets_from_extract` (spec E: zero escrita em
      /// `~/.config/waybar/` sem Waybar).
      pub skip_waybar: bool,
      /// `Some(dir)` quando omarchy-shell foi detectado — reinstala o plugin
      /// com o binário novo, matando o drift binário↔QML (spec F).
      pub omarchy_plugins_dir: Option<&'a Path>,
      pub run_command: &'a dyn Fn(&str, &[String], &Path) -> CommandResult,
      pub http: &'a reqwest::Client,
      pub releases_api_url: String,
      pub download_base_url: String,
      /// Seams de teste: substituem os checks reais de `sha256sum`/`tar` no PATH
      /// (produção usa `crate::install::has_cmd`).
      pub has_sha256sum: &'a dyn Fn() -> bool,
      pub has_tar: &'a dyn Fn() -> bool,
  }
  ```
- [ ] Em `src/update.rs:684-704`, hoje:
  ```rust
      replace_binary_atomic(&new_binary, opts.exe_path)?;
      install_standalone_assets(tmp_path, opts.data_dir)?;
      // Icons/helper que a Waybar lê em ~/.config — sem re-patchar modules/CSS.
      refresh_waybar_assets_from_extract(tmp_path, opts.waybar_dir, opts.scripts_dir)?;

      Ok(StandaloneUpdateStatus::Updated {
          old_version: opts.current_version.to_string(),
          new_version: ver_bare,
      })
  }

  /// Re-copia `icons/` e o terminal helper do tarball extraído para os paths da
  /// Waybar. Não mexe em `config.jsonc` / `style.css` / reload — isso é `setup`.
  fn refresh_waybar_assets_from_extract(
      extracted_dir: &Path,
      waybar_dir: &Path,
      scripts_dir: &Path,
  ) -> anyhow::Result<()> {
      crate::waybar_contract::install_waybar_assets(waybar_dir, scripts_dir, Some(extracted_dir))
          .map(|_| ())
  }
  ```
  Substituir por:
  ```rust
      replace_binary_atomic(&new_binary, opts.exe_path)?;
      install_standalone_assets(tmp_path, opts.data_dir)?;
      apply_standalone_platform_gates(
          tmp_path,
          opts.waybar_dir,
          opts.scripts_dir,
          opts.skip_waybar,
          opts.omarchy_plugins_dir,
      )?;

      Ok(StandaloneUpdateStatus::Updated {
          old_version: opts.current_version.to_string(),
          new_version: ver_bare,
      })
  }

  /// Re-copia `icons/` e o terminal helper do tarball extraído para os paths da
  /// Waybar. Não mexe em `config.jsonc` / `style.css` / reload — isso é `setup`.
  fn refresh_waybar_assets_from_extract(
      extracted_dir: &Path,
      waybar_dir: &Path,
      scripts_dir: &Path,
  ) -> anyhow::Result<()> {
      crate::waybar_contract::install_waybar_assets(waybar_dir, scripts_dir, Some(extracted_dir))
          .map(|_| ())
  }

  /// Pós-extração, gated por plataforma (spec E/F): icons/helper da Waybar só
  /// são re-copiados com Waybar no PATH; o plugin omarchy só é reinstalado
  /// quando detectado. Extraída p/ ser testável sem rodar download/checksum
  /// reais (mesmo motivo de `refresh_waybar_assets_from_extract` já ser uma
  /// função à parte).
  fn apply_standalone_platform_gates(
      extracted_dir: &Path,
      waybar_dir: &Path,
      scripts_dir: &Path,
      skip_waybar: bool,
      omarchy_plugins_dir: Option<&Path>,
  ) -> anyhow::Result<()> {
      if !skip_waybar {
          refresh_waybar_assets_from_extract(extracted_dir, waybar_dir, scripts_dir)?;
      }
      if let Some(plugins_dir) = omarchy_plugins_dir {
          crate::omarchy_integration::install_omarchy_plugin(plugins_dir)?;
      }
      Ok(())
  }
  ```
- [ ] Atualizar os 2 testes existentes que constroem `StandaloneUpdateOptions` para compilar. Em `run_standalone_update_reports_up_to_date` (linha ~1338) e `run_standalone_update_fails_clearly_without_sha256sum` (linha ~1374), em ambos, logo após a linha `scripts_dir: &tmp.path().join("scripts"),`, adicionar:
  ```rust
              skip_waybar: false,
              omarchy_plugins_dir: None,
  ```
- [ ] Rodar `cargo test update` e ver passar: suíte inteira de `update.rs` `ok` (incl. os 2 testes novos).
- [ ] Implementação em `main.rs` — nota: `let platform = platform::detect();` já foi
  declarada na Task 2 (logo após `let install_root = ...`, `src/main.rs:396`);
  esta task só troca o uso, no closure `omarchy_setup_hint`. Em
  `src/main.rs:400-409`, hoje:
  ```rust
              // O binário novo traz QML novo — o drop-in do omarchy-shell só
              // atualiza via setup (o update não toca nele; ver setup::SetupConfig.omarchy).
              let omarchy_setup_hint = |home: &Path| {
                  let plugin_dir = omarchy_integration::default_omarchy_plugins_dir(home)
                      .join(app_identity::OMARCHY_PLUGIN_ID);
                  if plugin_dir.exists() {
                      term_prompt::note(&format!(
                          "Plugin omarchy-shell detectado. Rode `{} setup` para atualizá-lo.",
                          app_identity::APP_NAME
                      ));
                  }
              };
  ```
  Substituir por:
  ```rust
              // O binário novo traz QML novo — update (T2/T3) já reinstala o
              // drop-in quando `platform.omarchy`. O hint vira fallback só
              // para quando a detecção falhar nesta rodada mas o diretório
              // ainda existir (spec F).
              let omarchy_setup_hint = |home: &Path, platform: platform::Platform| {
                  if platform.omarchy {
                      return;
                  }
                  let plugin_dir = omarchy_integration::default_omarchy_plugins_dir(home)
                      .join(app_identity::OMARCHY_PLUGIN_ID);
                  if plugin_dir.exists() {
                      term_prompt::note(&format!(
                          "Plugin omarchy-shell detectado mas fora de sync (detecção falhou nesta rodada). Rode `{} setup` para atualizá-lo.",
                          app_identity::APP_NAME
                      ));
                  }
              };
  ```
- [ ] Em `src/main.rs:517-519` (dentro de `ManagedUpdateStatus::Updated`), hoje:
  ```rust
                                  update::ManagedUpdateStatus::Updated => {
                                      term_prompt::status("OK", "Update aplicado");
                                      omarchy_setup_hint(&home);
                                  }
  ```
  Substituir por:
  ```rust
                                  update::ManagedUpdateStatus::Updated => {
                                      term_prompt::status("OK", "Update aplicado");
                                      omarchy_setup_hint(&home, platform);
                                  }
  ```
- [ ] Em `src/main.rs:553-573` (montagem de `opts` do Standalone), hoje:
  ```rust
                      let data_dir = update::default_data_dir(&home);
                      let waybar_paths = waybar_contract::get_default_waybar_asset_paths();
                      let opts = update::StandaloneUpdateOptions {
                          current_version: app_identity::VERSION,
                          exe_path: &current_exe,
                          data_dir: &data_dir,
                          waybar_dir: &waybar_paths.waybar_dir,
                          scripts_dir: &waybar_paths.scripts_dir,
                          run_command: &run_real_command,
                          http: &http,
                          releases_api_url: format!(
                              "https://api.github.com/repos/{}/releases/latest",
                              update::GITHUB_REPO
                          ),
                          download_base_url: format!(
                              "https://github.com/{}/releases/download",
                              update::GITHUB_REPO
                          ),
                          has_sha256sum: &|| install::has_cmd("sha256sum"),
                          has_tar: &|| install::has_cmd("tar"),
                      };
  ```
  Substituir por:
  ```rust
                      let data_dir = update::default_data_dir(&home);
                      let waybar_paths = waybar_contract::get_default_waybar_asset_paths();
                      let omarchy_plugin_dir_for_update = platform
                          .omarchy
                          .then(|| omarchy_integration::default_omarchy_plugins_dir(&home));
                      let opts = update::StandaloneUpdateOptions {
                          current_version: app_identity::VERSION,
                          exe_path: &current_exe,
                          data_dir: &data_dir,
                          waybar_dir: &waybar_paths.waybar_dir,
                          scripts_dir: &waybar_paths.scripts_dir,
                          skip_waybar: !platform.waybar,
                          omarchy_plugins_dir: omarchy_plugin_dir_for_update.as_deref(),
                          run_command: &run_real_command,
                          http: &http,
                          releases_api_url: format!(
                              "https://api.github.com/repos/{}/releases/latest",
                              update::GITHUB_REPO
                          ),
                          download_base_url: format!(
                              "https://github.com/{}/releases/download",
                              update::GITHUB_REPO
                          ),
                          has_sha256sum: &|| install::has_cmd("sha256sum"),
                          has_tar: &|| install::has_cmd("tar"),
                      };
  ```
- [ ] Em `src/main.rs` (arm `Ok(update::StandaloneUpdateStatus::Updated { ... })`, logo abaixo do bloco anterior), hoje:
  ```rust
                          Ok(update::StandaloneUpdateStatus::Updated {
                              old_version,
                              new_version,
                          }) => {
                              term_prompt::status(
                                  "OK",
                                  &format!(
                                      "agent-bar atualizado: v{old_version} -> v{new_version}. Icons e helper da Waybar atualizados."
                                  ),
                              );
                              omarchy_setup_hint(&home);
                              std::process::exit(0);
                          }
  ```
  Substituir por:
  ```rust
                          Ok(update::StandaloneUpdateStatus::Updated {
                              old_version,
                              new_version,
                          }) => {
                              let mut msg = format!(
                                  "agent-bar atualizado: v{old_version} -> v{new_version}."
                              );
                              if platform.waybar {
                                  msg.push_str(" Icons e helper da Waybar atualizados.");
                              }
                              if platform.omarchy {
                                  msg.push_str(" Plugin omarchy-shell reinstalado.");
                              }
                              term_prompt::status("OK", &msg);
                              omarchy_setup_hint(&home, platform);
                              std::process::exit(0);
                          }
  ```
- [ ] Rodar `cargo test update` e ver passar (recompila `main.rs` + `update.rs`).
- [ ] Rodar `cargo clippy --all-targets -- -D warnings` e ver passar (variável `platform` movida pro closure `run_setup` da Task 2 é `Copy`, sem warning de captura).
- [ ] Commit: `git add -A && git commit -m "feat: update Standalone reinstala plugin omarchy"`

### Task 4: `AppState.platform` + gate no Save da TUI

**Files:**
- Modify: `src/tui/state.rs:274-275` (campo novo em `AppState`), `src/tui/state.rs:308-309` (default em `AppState::new()`)
- Modify: `src/tui/event_loop.rs:88-114` (`handle_save_config`), `src/tui/event_loop.rs:227` (boot override)
- Create (dentro do arquivo existente): `#[cfg(test)] mod tests` ao fim de `src/tui/event_loop.rs`

**Interfaces:**
- Consumes: `platform::Platform`/`platform::detect()` (Task 1).
- Produces: `AppState.platform: Platform` (consumido pela Task 5).

- [ ] Escrever o teste que falha — criar `#[cfg(test)] mod tests` ao final de `src/tui/event_loop.rs` (arquivo tem 363 linhas hoje; adicionar depois da última `}`):
  ```rust
  #[cfg(test)]
  mod tests {
      use super::*;
      use crate::config::Paths;
      use crate::providers::OwnedCtx;
      use crate::settings::load as load_settings;
      use crate::tui::state::ConfigState;
      use std::path::PathBuf;
      use tempfile::tempdir;

      fn make_octx(cfg_dir: &std::path::Path) -> OwnedCtx {
          let paths = Paths {
              cache_dir: cfg_dir.join("cache"),
              config_dir: cfg_dir.join("config"),
              claude_credentials: PathBuf::new(),
              codex_auth: PathBuf::new(),
              codex_sessions: PathBuf::new(),
              amp_settings: PathBuf::new(),
              amp_threads: PathBuf::new(),
              grok_home: PathBuf::new(),
              grok_auth: PathBuf::new(),
          };
          let settings = load_settings(&paths);
          OwnedCtx {
              client: reqwest::Client::new(),
              paths,
              settings,
              local_offset: time::UtcOffset::UTC,
              claude_usage_url: String::new(),
              version: "0.0.0-test",
              home: cfg_dir.to_path_buf(),
          }
      }

      #[test]
      #[serial_test::serial]
      fn handle_save_config_skips_waybar_write_when_omarchy_only() {
          let fake_home = tempdir().unwrap();
          let prev_home = std::env::var_os("HOME");
          std::env::set_var("HOME", fake_home.path());

          let octx = make_octx(fake_home.path());
          let mut state = AppState::new();
          state.platform = crate::platform::Platform {
              omarchy: true,
              waybar: false,
          };
          state.config_state = Some(ConfigState::new(&octx.settings));

          handle_save_config(&mut state, &octx);

          assert!(
              octx.paths.config_dir.join("settings.json").exists(),
              "settings.json deveria ter sido salvo mesmo com waybar ausente"
          );
          assert!(
              !fake_home.path().join(".config").join("waybar").exists(),
              "zero escrita em ~/.config/waybar quando Omarchy-only"
          );

          match prev_home {
              Some(v) => std::env::set_var("HOME", v),
              None => std::env::remove_var("HOME"),
          }
      }
  }
  ```
- [ ] Rodar `cargo test tui::event_loop` e ver falhar: `error[E0609]: no field 'platform' on type 'AppState'`.
- [ ] Implementação mínima — em `src/tui/state.rs:266-267`, hoje:
  ```rust
      pub animations: bool,
  }
  ```
  Substituir por:
  ```rust
      pub animations: bool,
      /// Plataforma detectada (Omarchy-shell / Waybar) — gate de campos
      /// só-Waybar na aba Config (`ConfigField::visible`) e do Save
      /// (`handle_save_config`). Default `{omarchy:false, waybar:true}`
      /// (mantém testes/snapshots determinísticos com o comportamento
      /// legado); `event_loop::run` sobrescreve com `platform::detect()`
      /// no boot real — mesmo padrão de `local_offset`/`glyph_mode`.
      pub platform: crate::platform::Platform,
  }
  ```
- [ ] Em `src/tui/state.rs:308-309` (dentro de `AppState::new()`), hoje:
  ```rust
              animations: true,
          }
      }
  }
  ```
  Substituir por:
  ```rust
              animations: true,
              platform: crate::platform::Platform {
                  omarchy: false,
                  waybar: true,
              },
          }
      }
  }
  ```
- [ ] Rodar `cargo test tui::event_loop` e ver falhar de novo (agora por comportamento): o teste espera que `~/.config/waybar` não seja criado, mas `handle_save_config` ainda chama `apply_waybar_integration` incondicional.
- [ ] Gatear o Save — em `src/tui/event_loop.rs:88-114`, hoje:
  ```rust
  fn handle_save_config(state: &mut AppState, octx: &OwnedCtx) {
      let edited = match state.config_state.as_ref() {
          Some(cs) => cs.edit_settings.clone(),
          None => return,
      };

      let result: Result<(), String> = (|| {
          settings::save(&octx.paths, &edited).map_err(|e| format!("save falhou: {e}"))?;

          let paths = get_default_waybar_integration_paths();
          let opts = ApplyOptions {
              paths,
              icons_dir: None,
              app_bin: None,
              terminal_script: None,
          };
          waybar_integration::apply_waybar_integration(&edited, opts)
              .map_err(|e| format!("apply falhou: {e}"))?;

          setup::reload_waybar();
          Ok(())
      })();

      for a in update(state, Action::ConfigSaveResult(result)) {
          update(state, a);
      }
  }
  ```
  Substituir por:
  ```rust
  fn handle_save_config(state: &mut AppState, octx: &OwnedCtx) {
      let edited = match state.config_state.as_ref() {
          Some(cs) => cs.edit_settings.clone(),
          None => return,
      };
      let platform = state.platform;

      let result: Result<(), String> = (|| {
          settings::save(&octx.paths, &edited).map_err(|e| format!("save falhou: {e}"))?;

          if platform.waybar {
              let paths = get_default_waybar_integration_paths();
              let opts = ApplyOptions {
                  paths,
                  icons_dir: None,
                  app_bin: None,
                  terminal_script: None,
              };
              waybar_integration::apply_waybar_integration(&edited, opts)
                  .map_err(|e| format!("apply falhou: {e}"))?;

              setup::reload_waybar();
          }
          Ok(())
      })();

      for a in update(state, Action::ConfigSaveResult(result)) {
          update(state, a);
      }
  }
  ```
- [ ] Boot real — em `src/tui/event_loop.rs:227` (logo após `state.animations = octx.settings.menu.animations;`), hoje a linha seguinte é:
  ```rust
      // Zonas clicaveis do frame atual (Task 9): populado por `render`, limpo
  ```
  Inserir ANTES dela:
  ```rust
      // Plataforma real (spec E): sem isto, `state.platform` fica no default
      // {omarchy:false, waybar:true} do AppState::new() mesmo em Omarchy-only —
      // o Save recriaria ~/.config/waybar do zero e a aba Config mostraria
      // campos só-Waybar que não fazem sentido ali.
      state.platform = crate::platform::detect();
      // Zonas clicaveis do frame atual (Task 9): populado por `render`, limpo
  ```
- [ ] Rodar `cargo test tui::event_loop` e ver passar.
- [ ] Rodar `cargo test tui::state` (garante que os fixtures `AppState::new()` de `render/config.rs`, `render/detail/mod.rs`, `render/history.rs`, `update/mod.rs` continuam OK com o campo novo) e ver passar sem snapshots quebrados.
- [ ] Commit: `git add -A && git commit -m "feat: Save da TUI so escreve waybar com platform.waybar"`

### Task 5: Config platform-aware (esconder campos só-Waybar)

**Files:**
- Modify: `src/tui/state.rs:94-103` (adiciona `ConfigField::visible` no `impl ConfigField`)
- Modify: `src/tui/render/config.rs:16-46,49-50,67,109,119,161,186,193` (assinaturas + chamadas)
- Modify: `src/tui/update/config.rs:150-158,160-171,181-201` (`config_down`, `config_enter_edit`, `config_confirm_edit`)
- Modify: `src/tui/update/mod.rs` (novo teste após `config_navigate_clamps_at_bounds`, linha 781)

**Interfaces:**
- Consumes: `AppState.platform` (Task 4).
- Produces: `ConfigField::visible(platform: Platform) -> Vec<ConfigField>`.

- [ ] Escrever o teste que falha — em `src/tui/render/config.rs`, no `mod tests` (após `config_renders_waybar_and_tui_sections`, antes do `}` final de linha 419), adicionar:
  ```rust
  #[test]
  fn config_hides_waybar_only_fields_when_omarchy_only() {
      let backend = ratatui::backend::TestBackend::new(64, 24);
      let mut terminal = ratatui::Terminal::new(backend).unwrap();
      let mut state = state_on_waybar();
      state.platform = crate::platform::Platform {
          omarchy: true,
          waybar: false,
      };
      terminal
          .draw(|f| render_config(&state, f, f.area(), &mut HitMap::default()))
          .unwrap();
      let text = buffer_to_string(terminal.backend().buffer());

      assert!(
          !text.contains("Separadores"),
          "Separadores deveria estar oculto:\n{text}"
      );
      assert!(!text.contains("Sinal"), "Sinal deveria estar oculto:\n{text}");
      assert!(
          !text.contains("Intervalo"),
          "Intervalo deveria estar oculto:\n{text}"
      );
      assert!(
          text.contains("Provedores"),
          "Provedores deveria continuar visível:\n{text}"
      );
      assert!(
          text.contains("Câmbio"),
          "Câmbio (FxRate) deveria continuar visível:\n{text}"
      );
  }
  ```
- [ ] Rodar `cargo test config` e ver falhar: os campos só-Waybar aparecem mesmo com `platform.omarchy=true, waybar=false` (o teste falha nas 3 primeiras assertivas).
- [ ] Implementação mínima — em `src/tui/state.rs`, dentro de `impl ConfigField` (logo após o array `pub const ALL`, linha 103, antes de `pub fn label`), adicionar:
  ```rust
      /// Campos exibidos p/ a plataforma dada: esconde Separators/Signal/
      /// Interval (só afetam o Waybar) quando `platform` é exatamente
      /// Omarchy-only (`omarchy: true, waybar: false`) — spec 2026-07-21 §E.
      /// Providers/ProviderOrder/DisplayMode continuam (o Widget.qml também
      /// os lê) e FxRate continua (só afeta esta TUI).
      pub fn visible(platform: crate::platform::Platform) -> Vec<ConfigField> {
          let hide_waybar_only = platform.omarchy && !platform.waybar;
          ConfigField::ALL
              .into_iter()
              .filter(|f| {
                  !hide_waybar_only
                      || !matches!(
                          f,
                          ConfigField::Separators | ConfigField::Signal | ConfigField::Interval
                      )
              })
              .collect()
      }
  ```
- [ ] Em `src/tui/render/config.rs:27-45` (dispatch de `render_config`), hoje:
  ```rust
      match &state.config_state {
          None => {
              // Ainda nao inicializado (primeira entrada na aba)
              let block = Block::default()
                  .borders(Borders::ALL)
                  .border_type(BorderType::Rounded)
                  .border_style(Style::default().fg(to_ratatui(ColorToken::Comment)));
              let p = Paragraph::new(Span::styled(
                  " Carregando config...",
                  Style::default().fg(to_ratatui(ColorToken::Muted)),
              ))
              .block(block);
              frame.render_widget(p, area);
          }
          Some(cs) => {
              render_field_list(cs, frame, list_area);
              render_field_detail(cs, frame, detail_area, hits);
          }
      }
  ```
  Substituir o braço `Some(cs)` por:
  ```rust
          Some(cs) => {
              let visible = ConfigField::visible(state.platform);
              render_field_list(cs, &visible, frame, list_area);
              render_field_detail(cs, &visible, frame, detail_area, hits);
          }
      }
  ```
- [ ] Em `src/tui/render/config.rs:49-91` (`render_field_list`), trocar a assinatura de:
  ```rust
  fn render_field_list(cs: &ConfigState, frame: &mut Frame, area: Rect) {
  ```
  por:
  ```rust
  fn render_field_list(cs: &ConfigState, visible: &[ConfigField], frame: &mut Frame, area: Rect) {
  ```
  e a linha 67 de:
  ```rust
      for (i, field) in ConfigField::ALL.iter().enumerate() {
  ```
  para:
  ```rust
      for (i, field) in visible.iter().enumerate() {
  ```
- [ ] Em `src/tui/render/config.rs:109-162` (`render_field_detail`), trocar a assinatura de:
  ```rust
  fn render_field_detail(cs: &ConfigState, frame: &mut Frame, area: Rect, hits: &mut HitMap) {
  ```
  por:
  ```rust
  fn render_field_detail(cs: &ConfigState, visible: &[ConfigField], frame: &mut Frame, area: Rect, hits: &mut HitMap) {
  ```
  a linha 119 de `let field = ConfigField::ALL[cs.selected_field];` para `let field = visible[cs.selected_field];`, e a chamada da linha 161 de:
  ```rust
      render_help_and_status(cs, frame, help_area, hits);
  ```
  para:
  ```rust
      render_help_and_status(cs, visible, frame, help_area, hits);
  ```
- [ ] Em `src/tui/render/config.rs:186-239` (`render_help_and_status`), trocar a assinatura de:
  ```rust
  fn render_help_and_status(cs: &ConfigState, frame: &mut Frame, area: Rect, hits: &mut HitMap) {
  ```
  por:
  ```rust
  fn render_help_and_status(cs: &ConfigState, visible: &[ConfigField], frame: &mut Frame, area: Rect, hits: &mut HitMap) {
  ```
  e a linha 193 de `let field = ConfigField::ALL[cs.selected_field];` para `let field = visible[cs.selected_field];`.
- [ ] Rodar `cargo test tui::render::config` e ver passar (o teste novo `config_hides_waybar_only_fields_when_omarchy_only`; os snapshots existentes usam `state_on_waybar()` com `platform` default `{waybar:true}` → `visible == ALL`, então continuam byte-a-byte iguais).
- [ ] Escrever o teste de navegação que falha — em `src/tui/update/mod.rs`, logo após `config_navigate_clamps_at_bounds` (linha 781), adicionar:
  ```rust
  #[test]
  fn config_navigate_clamps_at_reduced_max_when_omarchy_only() {
      let mut state = AppState::new();
      state.platform = crate::platform::Platform {
          omarchy: true,
          waybar: false,
      };
      update(&mut state, Action::InitConfig(fake_settings()));

      // Só Providers/ProviderOrder/DisplayMode/FxRate ficam visíveis (4
      // campos, índices 0..=3) — Separators/Signal/Interval somem.
      for _ in 0..10 {
          update(&mut state, Action::ConfigDown);
      }
      assert_eq!(state.config_state.as_ref().unwrap().selected_field, 3);
  }
  ```
- [ ] Rodar `cargo test tui::update` e ver falhar: `selected_field` chega em 6 (usa `ConfigField::ALL.len()` cheio), não 3.
- [ ] Gatear a navegação — em `src/tui/update/config.rs:150-158`, hoje:
  ```rust
  pub(super) fn config_down(state: &mut AppState) -> Vec<Action> {
      if let Some(cs) = state.config_state.as_mut() {
          let max = ConfigField::ALL.len().saturating_sub(1);
          if !cs.editing && cs.selected_field < max {
              cs.selected_field += 1;
          }
      }
      vec![]
  }
  ```
  Substituir por:
  ```rust
  pub(super) fn config_down(state: &mut AppState) -> Vec<Action> {
      let max = ConfigField::visible(state.platform).len().saturating_sub(1);
      if let Some(cs) = state.config_state.as_mut() {
          if !cs.editing && cs.selected_field < max {
              cs.selected_field += 1;
          }
      }
      vec![]
  }
  ```
- [ ] Em `src/tui/update/config.rs:160-171`, hoje:
  ```rust
  pub(super) fn config_enter_edit(state: &mut AppState) -> Vec<Action> {
      if let Some(cs) = state.config_state.as_mut() {
          if !cs.editing {
              let field = ConfigField::ALL[cs.selected_field];
              let current = field_value_string(field, cs);
              cs.input = tui_input::Input::new(current);
              cs.editing = true;
              cs.status_msg = None;
          }
      }
      vec![]
  }
  ```
  Substituir por:
  ```rust
  pub(super) fn config_enter_edit(state: &mut AppState) -> Vec<Action> {
      let visible = ConfigField::visible(state.platform);
      if let Some(cs) = state.config_state.as_mut() {
          if !cs.editing {
              let field = visible[cs.selected_field];
              let current = field_value_string(field, cs);
              cs.input = tui_input::Input::new(current);
              cs.editing = true;
              cs.status_msg = None;
          }
      }
      vec![]
  }
  ```
- [ ] Em `src/tui/update/config.rs:181-201`, hoje:
  ```rust
  pub(super) fn config_confirm_edit(state: &mut AppState) -> Vec<Action> {
      if let Some(cs) = state.config_state.as_mut() {
          if cs.editing {
              let field = ConfigField::ALL[cs.selected_field];
              let value = cs.input.value().to_string();
              match apply_field_edit(field, &value, cs) {
                  Ok(()) => {
                      cs.editing = false;
                      cs.input = tui_input::Input::default();
                      cs.status_msg =
                          Some("Campo atualizado. Pressione [s] para salvar.".to_string());
                  }
                  Err(e) => {
                      cs.status_msg = Some(format!("Erro: {e}"));
                      // Mantem edicao aberta para correcao.
                  }
              }
          }
      }
      vec![]
  }
  ```
  Substituir por:
  ```rust
  pub(super) fn config_confirm_edit(state: &mut AppState) -> Vec<Action> {
      let visible = ConfigField::visible(state.platform);
      if let Some(cs) = state.config_state.as_mut() {
          if cs.editing {
              let field = visible[cs.selected_field];
              let value = cs.input.value().to_string();
              match apply_field_edit(field, &value, cs) {
                  Ok(()) => {
                      cs.editing = false;
                      cs.input = tui_input::Input::default();
                      cs.status_msg =
                          Some("Campo atualizado. Pressione [s] para salvar.".to_string());
                  }
                  Err(e) => {
                      cs.status_msg = Some(format!("Erro: {e}"));
                      // Mantem edicao aberta para correcao.
                  }
              }
          }
      }
      vec![]
  }
  ```
- [ ] Rodar `cargo test tui::update` e ver passar (os testes existentes `config_navigate_clamps_at_bounds`/`fx_rate_index`/`config_enter_edit_sets_input_to_current_value` continuam OK: `AppState::new()` tem `platform.waybar=true` por default → `visible() == ALL`).
- [ ] Commit: `git add -A && git commit -m "feat: aba Config esconde campos so-Waybar em Omarchy-only"`

### Task 6: mover `waybar_contract.rs`/`waybar_integration.rs` para `src/waybar/`

**Files:**
- Create: `src/waybar/mod.rs`
- Modify (rename via `git mv`, sem edição de conteúdo): `src/waybar_contract.rs` → `src/waybar/contract.rs`, `src/waybar_integration.rs` → `src/waybar/integration.rs`
- Modify: `src/lib.rs:29-30`

**Interfaces:**
- Produces: `crate::waybar_contract::*` e `crate::waybar_integration::*` continuam resolvendo (usados por `setup.rs`, `update.rs`, `main.rs`, `event_loop.rs`, `formatters/waybar.rs` sem qualquer edição neles).
- Consumes: nenhuma interface nova de tasks anteriores — mecânico.

- [ ] Rodar `cargo test waybar_contract` e `cargo test waybar_integration` ANTES da mudança — confirmar baseline verde (evita atribuir uma falha pré-existente à mudança).
- [ ] Mover os arquivos preservando histórico:
  ```bash
  mkdir -p src/waybar
  git mv src/waybar_contract.rs src/waybar/contract.rs
  git mv src/waybar_integration.rs src/waybar/integration.rs
  ```
- [ ] Rodar `cargo test waybar_contract` e ver falhar: `error[E0433]: failed to resolve: could not find 'waybar_contract' in the crate root` (o `mod` antigo em `lib.rs` ainda aponta pro arquivo que não existe mais).
- [ ] Implementação mínima — criar `src/waybar/mod.rs`:
  ```rust
  //! Agrupa o tier legado Waybar (spec 2026-07-21 §E): `contract.rs`
  //! (formatos exportados: `modules.jsonc`/`style.css`) e `integration.rs`
  //! (patch in-place do `config.jsonc`/`style.css` do usuário).
  //!
  //! Os módulos mantêm o IDENTIFICADOR original (`waybar_contract`,
  //! `waybar_integration`) via `#[path]` — só o arquivo físico mudou de
  //! lugar. Isso preserva `crate::waybar_contract::*`/`crate::waybar_integration::*`
  //! em todo o resto do crate SEM editar nenhum callsite, e mantém
  //! `cargo test waybar_contract`/`cargo test waybar_integration` (CLAUDE.md)
  //! passando: o filtro de `cargo test` casa substring na string totalmente
  //! qualificada do teste, e um `pub use ... as waybar_contract` (renomear
  //! só no re-export) NÃO teria o mesmo efeito — o teste ficaria em
  //! `waybar::contract::tests::…`, que não contém a substring
  //! `"waybar_contract"` (o separador é `::`, não `_`).
  #[path = "contract.rs"]
  pub mod waybar_contract;
  #[path = "integration.rs"]
  pub mod waybar_integration;
  ```
- [ ] Em `src/lib.rs:29-30`, hoje:
  ```rust
  pub mod waybar_contract;
  pub mod waybar_integration;
  ```
  Substituir por:
  ```rust
  pub mod waybar;
  pub use waybar::waybar_contract;
  pub use waybar::waybar_integration;
  ```
- [ ] Rodar `cargo test waybar_contract` e ver passar (mesma suíte de antes, agora em `crate::waybar::waybar_contract::tests::*`, mas o filtro substring continua casando).
- [ ] Rodar `cargo test waybar_integration` e ver passar.
- [ ] Rodar `cargo test setup` (consome `waybar_contract::install_waybar_assets`/`waybar_integration::apply_waybar_integration` via re-export) e ver passar sem edição.
- [ ] Rodar `cargo clippy --all-targets -- -D warnings` e ver passar.
- [ ] Commit: `git add -A && git commit -m "refactor: agrupa waybar_contract/integration em src/waybar/"`

### Task 7: `doctor` ganha checagens Omarchy

**Files:**
- Modify: `src/omarchy_integration.rs:26-32` (adiciona `default_omarchy_shell_json_path` logo após `default_omarchy_plugins_dir`), `src/omarchy_integration.rs` (adiciona `shell_json_has_plugin_entry` + `json_contains_string` antes do `#[cfg(test)] mod tests`, linha 144), `src/omarchy_integration.rs` (novos testes)
- Modify: `src/doctor.rs:1-3` (imports), `src/doctor.rs:25-30` (`DoctorResult` ganha campo), `src/doctor.rs:111-144` (adiciona `OmarchyFindings`/`scan_omarchy` após `scan`), `src/doctor.rs:162-210` (`run_doctor` popula o campo novo), `src/doctor.rs` (novos testes)
- Modify: `src/main.rs:358-381` (`Command::Doctor`, imprime avisos)

**Interfaces:**
- Consumes: `app_identity::{APP_NAME, OMARCHY_PLUGIN_ID, VERSION}`, `omarchy_integration::default_omarchy_plugins_dir`.
- Produces: `pub struct OmarchyFindings { .. }`, `pub fn scan_omarchy(home: &Path) -> OmarchyFindings`, `impl OmarchyFindings { pub fn warnings(&self) -> Vec<String> }`, `omarchy_integration::default_omarchy_shell_json_path`, `omarchy_integration::shell_json_has_plugin_entry`.

- [ ] Escrever o teste que falha — em `src/omarchy_integration.rs`, no `mod tests` (após `default_plugins_dir_respects_xdg_config_home`, linha 170), adicionar:
  ```rust
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
  ```
- [ ] Rodar `cargo test omarchy_integration` e ver falhar: `error[E0425]: cannot find function 'shell_json_has_plugin_entry'`.
- [ ] Implementação mínima — em `src/omarchy_integration.rs`, logo após `default_omarchy_plugins_dir` (linha 32, antes de `pub fn omarchy_shell_present`), adicionar:
  ```rust
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
  ```
  E, no fim do arquivo, logo antes de `#[cfg(test)]` (linha 144), adicionar:
  ```rust
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
  ```
- [ ] Rodar `cargo test omarchy_integration` e ver passar (3 testes novos).
- [ ] Escrever o teste que falha para o `doctor` — em `src/doctor.rs`, no `mod tests` (após `scan_considers_dev_dependencies`, linha 316), adicionar:
  ```rust
  fn with_clean_xdg_config_home<T>(f: impl FnOnce() -> T) -> T {
      let prev = std::env::var_os("XDG_CONFIG_HOME");
      std::env::remove_var("XDG_CONFIG_HOME");
      let result = f();
      match prev {
          Some(v) => std::env::set_var("XDG_CONFIG_HOME", v),
          None => std::env::remove_var("XDG_CONFIG_HOME"),
      }
      result
  }

  #[test]
  #[serial_test::serial]
  fn scan_omarchy_clean_when_nothing_installed() {
      with_clean_xdg_config_home(|| {
          let h = tempdir().unwrap();
          let f = scan_omarchy(h.path());
          assert!(f.manifest_version_mismatch.is_none());
          assert!(f.plugin_dir_without_shell_entry.is_none());
          assert!(f.shell_entry_without_plugin_dir.is_none());
          assert!(f.warnings().is_empty());
      });
  }

  #[test]
  #[serial_test::serial]
  fn scan_omarchy_flags_manifest_version_mismatch() {
      with_clean_xdg_config_home(|| {
          let h = tempdir().unwrap();
          let plugin_dir = h
              .path()
              .join(".config")
              .join("omarchy")
              .join("plugins")
              .join(crate::app_identity::OMARCHY_PLUGIN_ID);
          std::fs::create_dir_all(&plugin_dir).unwrap();
          std::fs::write(
              plugin_dir.join("manifest.json"),
              r#"{"id":"agent-bar.usage","version":"0.0.1-old"}"#,
          )
          .unwrap();

          let f = scan_omarchy(h.path());
          assert!(f.manifest_version_mismatch.is_some());
          assert!(f
              .manifest_version_mismatch
              .as_ref()
              .unwrap()
              .contains("0.0.1-old"));
      });
  }

  #[test]
  #[serial_test::serial]
  fn scan_omarchy_flags_plugin_dir_without_shell_entry() {
      with_clean_xdg_config_home(|| {
          let h = tempdir().unwrap();
          let plugin_dir = h
              .path()
              .join(".config")
              .join("omarchy")
              .join("plugins")
              .join(crate::app_identity::OMARCHY_PLUGIN_ID);
          std::fs::create_dir_all(&plugin_dir).unwrap();
          std::fs::write(
              plugin_dir.join("manifest.json"),
              format!(
                  r#"{{"id":"agent-bar.usage","version":"{}"}}"#,
                  crate::app_identity::VERSION
              ),
          )
          .unwrap();
          let shell_json = h.path().join(".config").join("omarchy").join("shell.json");
          std::fs::create_dir_all(shell_json.parent().unwrap()).unwrap();
          std::fs::write(&shell_json, r#"{"bar":{"plugins":[]}}"#).unwrap();

          let f = scan_omarchy(h.path());
          assert!(f.manifest_version_mismatch.is_none());
          assert!(f.plugin_dir_without_shell_entry.is_some());
          assert!(f.shell_entry_without_plugin_dir.is_none());
      });
  }

  #[test]
  #[serial_test::serial]
  fn scan_omarchy_flags_shell_entry_without_plugin_dir() {
      with_clean_xdg_config_home(|| {
          let h = tempdir().unwrap();
          let shell_json = h.path().join(".config").join("omarchy").join("shell.json");
          std::fs::create_dir_all(shell_json.parent().unwrap()).unwrap();
          std::fs::write(
              &shell_json,
              r#"{"bar":{"plugins":[{"id":"agent-bar.usage"}]}}"#,
          )
          .unwrap();

          let f = scan_omarchy(h.path());
          assert!(f.shell_entry_without_plugin_dir.is_some());
          assert!(f.plugin_dir_without_shell_entry.is_none());
      });
  }
  ```
- [ ] Rodar `cargo test doctor` e ver falhar: `error[E0425]: cannot find function 'scan_omarchy'`.
- [ ] Implementação mínima — em `src/doctor.rs:1-3`, hoje:
  ```rust
  use serde_json::Value;
  use std::fs;
  use std::path::{Path, PathBuf};
  ```
  Substituir por:
  ```rust
  use serde_json::Value;
  use std::fs;
  use std::path::{Path, PathBuf};

  use crate::app_identity::{APP_NAME, OMARCHY_PLUGIN_ID, VERSION};
  use crate::omarchy_integration::{default_omarchy_plugins_dir, default_omarchy_shell_json_path, shell_json_has_plugin_entry};
  ```
  Em `src/doctor.rs:25-30`, hoje:
  ```rust
  #[derive(Debug, Clone)]
  pub struct DoctorResult {
      pub status: DoctorStatus,
      pub removed: Vec<PathBuf>,
      pub findings: DoctorFindings,
  }
  ```
  Substituir por:
  ```rust
  #[derive(Debug, Clone)]
  pub struct DoctorResult {
      pub status: DoctorStatus,
      pub removed: Vec<PathBuf>,
      pub findings: DoctorFindings,
      pub omarchy: OmarchyFindings,
  }
  ```
  Logo após a função `scan` (linha 144, antes de `fn planned_removals`), adicionar:
  ```rust
  /// Achados Omarchy do `doctor`: drift binário↔plugin e referências
  /// penduradas em `shell.json`. Leitura pura — NUNCA escreve nada, NUNCA
  /// falha o comando (viram avisos, spec 2026-07-21 §F).
  #[derive(Debug, Clone, Default)]
  pub struct OmarchyFindings {
      /// `Some` quando o manifest instalado tem `version` diferente do binário.
      pub manifest_version_mismatch: Option<String>,
      /// `Some` quando o diretório do plugin existe mas `shell.json` não
      /// referencia `agent-bar.usage`.
      pub plugin_dir_without_shell_entry: Option<String>,
      /// `Some` quando `shell.json` referencia `agent-bar.usage` mas o
      /// diretório do plugin não existe.
      pub shell_entry_without_plugin_dir: Option<String>,
  }

  impl OmarchyFindings {
      /// Achados como mensagens prontas p/ `term_prompt::status("Aviso", ...)`.
      pub fn warnings(&self) -> Vec<String> {
          [
              &self.manifest_version_mismatch,
              &self.plugin_dir_without_shell_entry,
              &self.shell_entry_without_plugin_dir,
          ]
          .into_iter()
          .filter_map(|w| w.clone())
          .collect()
      }
  }

  pub fn scan_omarchy(home: &Path) -> OmarchyFindings {
      let plugin_dir = default_omarchy_plugins_dir(home).join(OMARCHY_PLUGIN_ID);
      let dir_exists = plugin_dir.is_dir();

      let manifest_version_mismatch = if dir_exists {
          read_json(&plugin_dir.join("manifest.json"))
              .and_then(|v| v.get("version").and_then(|v| v.as_str()).map(str::to_string))
              .filter(|installed| installed != VERSION)
              .map(|installed| {
                  format!(
                      "Plugin omarchy instalado (v{installed}) diverge do binário (v{VERSION}). Rode `{APP_NAME} setup` para atualizá-lo."
                  )
              })
      } else {
          None
      };

      let shell_json_path = default_omarchy_shell_json_path(home);
      let shell_has_entry = shell_json_has_plugin_entry(&shell_json_path);

      let plugin_dir_without_shell_entry = (dir_exists && !shell_has_entry).then(|| {
          format!(
              "Diretório do plugin existe ({}) mas {} não referencia `{OMARCHY_PLUGIN_ID}`. Rode `omarchy bar plugin add {OMARCHY_PLUGIN_ID}`.",
              plugin_dir.display(),
              shell_json_path.display()
          )
      });

      let shell_entry_without_plugin_dir = (!dir_exists && shell_has_entry).then(|| {
          format!(
              "{} referencia `{OMARCHY_PLUGIN_ID}` mas o diretório do plugin não existe ({}). Rode `{APP_NAME} setup`.",
              shell_json_path.display(),
              plugin_dir.display()
          )
      });

      OmarchyFindings {
          manifest_version_mismatch,
          plugin_dir_without_shell_entry,
          shell_entry_without_plugin_dir,
      }
  }
  ```
  Em `src/doctor.rs:162-210` (`run_doctor`), hoje:
  ```rust
  pub fn run_doctor(opts: DoctorOptions<'_>) -> DoctorResult {
      let findings = scan(opts.home);

      let nothing_to_do = !findings.package_json_orphan
          && !findings.package_json_mixed
          && findings.node_modules_dir.is_none()
          && findings.lockfiles.is_empty();

      if nothing_to_do {
          return DoctorResult {
              status: DoctorStatus::Clean,
              removed: vec![],
              findings,
          };
      }

      let approved = opts.yes || (opts.confirm)(&findings);
      if !approved {
          return DoctorResult {
              status: DoctorStatus::Cancelled,
              removed: vec![],
              findings,
          };
      }

      let removals = planned_removals(&findings);

      if !opts.dry_run {
          for path in &removals {
              if path.is_dir() {
                  let _ = fs::remove_dir_all(path);
              } else {
                  let _ = fs::remove_file(path);
              }
          }
      }

      let status = if findings.package_json_mixed && !findings.package_json_orphan {
          DoctorStatus::MixedOnly
      } else {
          DoctorStatus::Cleaned
      };

      DoctorResult {
          status,
          removed: removals,
          findings,
      }
  }
  ```
  Substituir por:
  ```rust
  pub fn run_doctor(opts: DoctorOptions<'_>) -> DoctorResult {
      let findings = scan(opts.home);
      let omarchy = scan_omarchy(opts.home);

      let nothing_to_do = !findings.package_json_orphan
          && !findings.package_json_mixed
          && findings.node_modules_dir.is_none()
          && findings.lockfiles.is_empty();

      if nothing_to_do {
          return DoctorResult {
              status: DoctorStatus::Clean,
              removed: vec![],
              findings,
              omarchy,
          };
      }

      let approved = opts.yes || (opts.confirm)(&findings);
      if !approved {
          return DoctorResult {
              status: DoctorStatus::Cancelled,
              removed: vec![],
              findings,
              omarchy,
          };
      }

      let removals = planned_removals(&findings);

      if !opts.dry_run {
          for path in &removals {
              if path.is_dir() {
                  let _ = fs::remove_dir_all(path);
              } else {
                  let _ = fs::remove_file(path);
              }
          }
      }

      let status = if findings.package_json_mixed && !findings.package_json_orphan {
          DoctorStatus::MixedOnly
      } else {
          DoctorStatus::Cleaned
      };

      DoctorResult {
          status,
          removed: removals,
          findings,
          omarchy,
      }
  }
  ```
  Isso também exige atualizar os testes existentes de `run_doctor` (`run_doctor_clean`, `run_doctor_removes_orphan_set_when_confirmed`, `run_doctor_mixed_keeps_package_json`, `run_doctor_cancelled`, `run_doctor_dry_run_reports_without_removing`, `run_doctor_yes_skips_confirm`, linhas 318-465) — nenhum deles constrói `DoctorResult` diretamente (só chamam `run_doctor(...)` e leem `r.status`/`r.removed`), então continuam compilando sem alteração.
- [ ] Rodar `cargo test doctor` e ver passar (4 testes novos + suíte existente intacta).
- [ ] Imprimir os avisos no CLI — em `src/main.rs:358-381` (`Command::Doctor`), hoje o bloco termina em:
  ```rust
                  doctor::DoctorStatus::Cancelled => {
                      term_prompt::status("Cancelado", "Doctor não aplicou mudanças");
                  }
              }
              std::process::exit(0);
          }
  ```
  Substituir por:
  ```rust
                  doctor::DoctorStatus::Cancelled => {
                      term_prompt::status("Cancelado", "Doctor não aplicou mudanças");
                  }
              }
              for warning in result.omarchy.warnings() {
                  term_prompt::status("Aviso", &warning);
              }
              std::process::exit(0);
          }
  ```
- [ ] Rodar `cargo test doctor` e ver passar novamente (recompilação de `main.rs`).
- [ ] Rodar `cargo clippy --all-targets -- -D warnings` e ver passar.
- [ ] Commit: `git add -A && git commit -m "feat: doctor detecta drift do plugin omarchy"`

### Task 8: `uninstall` só remove o plugin se `omarchy ... remove` confirmar

**Files:**
- Modify: `src/uninstall.rs:36-44` (assinatura de `run_uninstall`), `src/uninstall.rs:85-98` (lógica de remoção), `src/uninstall.rs` (novo `#[cfg(test)] mod tests` — arquivo não tem testes hoje)
- Modify: `src/main.rs:619-627` (call site)

**Interfaces:**
- Consumes: `omarchy_integration::{omarchy_cli_available, run_omarchy_remove_commands, remove_omarchy_plugin}` (agora injetadas via parâmetro, não chamadas direto).
- Produces: `run_uninstall(..., cli_available_fn: &dyn Fn() -> bool, remove_commands_fn: &dyn Fn() -> Vec<String>)`.

- [ ] Escrever o teste que falha — criar `#[cfg(test)] mod tests` ao final de `src/uninstall.rs` (arquivo tem 115 linhas hoje):
  ```rust
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
  }
  ```
- [ ] Rodar `cargo test uninstall` e ver falhar: `error[E0061]: this function takes 7 arguments but 9 were supplied`.
- [ ] Implementação mínima — em `src/uninstall.rs:36-44`, hoje:
  ```rust
  pub fn run_uninstall(
      settings_dir: &Path,
      cache_dir: &Path,
      home: &Path,
      force: bool,
      title: &str,
      integration_paths: &WaybarIntegrationPaths,
      omarchy_plugins_dir: &Path,
  ) -> anyhow::Result<()> {
  ```
  Substituir por (o `#[allow]` é necessário porque a assinatura vai a 9
  parâmetros e `clippy::too_many_arguments` dispara com `-D warnings` —
  mesma convenção já usada em `src/formatters/builders/shared.rs`):
  ```rust
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
  ```
  Em `src/uninstall.rs:3-11` (imports), hoje:
  ```rust
  use std::path::Path;

  use crate::app_identity::{APP_NAME, OMARCHY_PLUGIN_ID};
  use crate::omarchy_integration::{
      omarchy_cli_available, remove_omarchy_plugin, run_omarchy_remove_commands,
  };
  use crate::waybar_contract::get_default_waybar_asset_paths;
  use crate::waybar_integration::{remove_waybar_integration, WaybarIntegrationPaths};
  use crate::{setup, term_prompt};
  ```
  Substituir por:
  ```rust
  use std::path::Path;

  use crate::app_identity::{APP_NAME, OMARCHY_PLUGIN_ID};
  use crate::omarchy_integration::remove_omarchy_plugin;
  use crate::waybar_contract::get_default_waybar_asset_paths;
  use crate::waybar_integration::{remove_waybar_integration, WaybarIntegrationPaths};
  use crate::{setup, term_prompt};
  ```
  (`omarchy_cli_available`/`run_omarchy_remove_commands` deixam de ser chamadas direto — agora vêm por parâmetro, como seams de teste.)
  Em `src/uninstall.rs:85-98`, hoje:
  ```rust
      // Omarchy-shell: desregistra no shell (best-effort) e remove o drop-in.
      let plugin_dir = omarchy_plugins_dir.join(OMARCHY_PLUGIN_ID);
      if plugin_dir.exists() {
          if omarchy_cli_available() {
              for warning in run_omarchy_remove_commands() {
                  term_prompt::status("Aviso", &warning);
              }
          }
          match remove_omarchy_plugin(omarchy_plugins_dir) {
              Ok(true) => removed.push(plugin_dir.to_string_lossy().into_owned()),
              Ok(false) => {}
              Err(_) => failed.push(plugin_dir.to_string_lossy().into_owned()),
          }
      }
  ```
  Substituir por:
  ```rust
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
  ```
- [ ] Rodar `cargo test uninstall` e ver falhar de novo: agora falha no call site de `main.rs` (`error[E0061]: this function takes 9 arguments but 7 were supplied`).
- [ ] Atualizar o call site — em `src/main.rs:619-627`, hoje:
  ```rust
              match uninstall::run_uninstall(
                  &settings_dir,
                  &paths.cache_dir,
                  &home,
                  force,
                  &format!("{APP_NAME} uninstall"),
                  &ipaths,
                  &omarchy_integration::default_omarchy_plugins_dir(&home),
              ) {
  ```
  Substituir por:
  ```rust
              match uninstall::run_uninstall(
                  &settings_dir,
                  &paths.cache_dir,
                  &home,
                  force,
                  &format!("{APP_NAME} uninstall"),
                  &ipaths,
                  &omarchy_integration::default_omarchy_plugins_dir(&home),
                  &omarchy_integration::omarchy_cli_available,
                  &omarchy_integration::run_omarchy_remove_commands,
              ) {
  ```
- [ ] Rodar `cargo test uninstall` e ver passar.
- [ ] Escrever + rodar os 2 testes de "não confirmado" (adicionar ao mesmo `mod tests` de `src/uninstall.rs`, seguindo o mesmo padrão de save/restore de `HOME`):
  ```rust
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

          assert!(plugin_dir.exists(), "diretório do plugin deveria ser mantido");

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
  ```
- [ ] Rodar `cargo test uninstall` e ver passar: 3 testes `ok`.
- [ ] Rodar `cargo clippy --all-targets -- -D warnings` e ver passar.
- [ ] Commit: `git add -A && git commit -m "feat: uninstall so apaga plugin omarchy se remove confirmar"`

## Verificação final do PR

Depois das 8 tasks, antes de abrir o PR:

```bash
cargo test
cargo clippy --all-targets -- -D warnings
```

Ambos precisam terminar limpos (gotcha RTK: se `cargo test` completo não mostrar
`test result:` claramente por causa da reformatação do hook, rodar os filtros
por área — `cargo test platform`, `cargo test update`, `cargo test tui`,
`cargo test waybar_contract`, `cargo test waybar_integration`, `cargo test doctor`,
`cargo test uninstall`, `cargo test omarchy_integration`, `cargo test setup` —
um de cada vez). Em seguida, `superpowers:finishing-a-development-branch` para
decidir merge/PR (a spec prevê PR4 de uma sequência de 5 sobre `master`, então
o destino natural é PR, não merge direto).
