# Contrato windowKind + fixes de dado (PR 2/5) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Introduzir `windowKind` (fiveHour/sevenDay/daily/context/other) como fonte
única de verdade pro rótulo de janela, matar o fallback forçado do Codex que
duplicava "Weekly", e dar TUI/Codex-builder paridade de fuso local + countdown
+ dedup — sem tocar QML nem tela alguma (isso é PR 3).

**Fora deste plano:** a ocultação platform-aware dos campos
Separators/Signal/Interval na TUI Config (Seção D do spec) fica no plano 04,
pois depende de `platform::detect()`.

**Architecture:** `WindowKind` nasce em `src/providers/types.rs` (mesmo enum
que hoje vive, incompleto, em `formatters/shared.rs`); cada provider tag a
janela na origem (`window_kind: Some(...)`); `classify_window` continua
derivando fiveHour/sevenDay/other por tolerância de minutos, mas devolve o
enum canônico; `is_duplicate_window` compara `(window_kind, resetsAt,
remaining arredondado)` e é consumido pela TUI (Codex builder ganha rótulo de
duração real pras janelas `other`; QML implementa o mesmo predicado em JS na
PR 3). Mudança 100% aditiva em `QuotaWindow` — `schemaVersion` continua 1.

**Tech Stack:** Rust/cargo, serde (camelCase), time (local-offset), insta
(snapshots), ratatui (TUI).

## Global Constraints

- Rust/cargo only; nunca `unwrap()`/`expect()` em produção — propagar com `?`
  ou `anyhow::bail!`.
- Constantes de identidade vêm de `src/app_identity.rs`, nunca strings
  hardcoded.
- Provider error strings são contrato (testes assertam verbatim) — esta
  mudança não altera nenhuma string de erro existente.
- XML-escape só em `render_pango.rs`; builders nunca escapam.
- Nunca round-trip de `config.jsonc` do Waybar via `serde_json`.
- Testes: `#[tokio::test]` async / `#[test]` sync; sem rede/CLIs vivas/Waybar
  real; `XDG_CONFIG_HOME`/`XDG_CACHE_HOME` setados ANTES de qualquer import
  que leia `src/config.rs`.
- Snapshots via `insta` (Waybar byte-for-byte, terminal sanitizado) —
  atualizar só quando o contrato de display mudar de propósito, com
  justificativa no commit.
- Gotcha RTK: `cargo test` com APENAS UM filtro posicional por invocação.
  Verificação = `cargo test <filtro>` + `cargo clippy --all-targets --
  -D warnings`. `check-types` não existe.
- Commits: Conventional Commits em PT, subject ≤50 chars. ZERO atribuição de
  AI em qualquer texto (commit/PR/comentário).
- Prosa em pt-BR; identificadores/código em inglês.

## Pré-requisito de sequência

Este plano assume que o **plano 01** (limpeza de legado, seção G do spec) já
foi mergeado em `master` antes de começar — em particular a migração de
settings v2→v3 que dropa `waybar.show_percentage`. Se `git log` não mostrar
esse commit, pare e confirme com quem orquestra antes de prosseguir.

---

### Task 1: `WindowKind` + campo `window_kind` em `QuotaWindow`

Maior raio de explosão do plano: `QuotaWindow` não deriva `Default`, então
todo `QuotaWindow { .. }` literal do crate (produção E testes) precisa do
campo novo pra compilar. Produção real (Claude/Codex/Amp/Grok) fica com
`window_kind: None` nesta task — as Tasks 2-4 substituem por `Some(kind)`
real. Todo o resto (fixtures de teste em `formatters/`, `tui/`, `notify.rs`,
`tests/golden.rs`) recebe `window_kind: None,` mecânico, sem mudança de
comportamento.

**Files:**
- Modify: `src/providers/types.rs` (struct `QuotaWindow` linhas 10-25; testes
  do módulo linhas 138-146, 160-176)
- Modify: `src/formatters/shared.rs` (remove enum local linhas 39-44; vira
  re-export)
- Modify (mecânico, só compile-fix — ver Step 5 pra lista completa e exata):
  `src/formatters/builders/amp.rs`, `src/formatters/builders/claude.rs`,
  `src/formatters/builders/codex.rs`, `src/formatters/builders/generic.rs`,
  `src/formatters/builders/grok.rs`, `src/formatters/builders/shared.rs`,
  `src/formatters/codex_helpers.rs`, `src/formatters/json.rs`,
  `src/formatters/terminal.rs`, `src/formatters/view_model.rs`,
  `src/formatters/waybar.rs`, `src/tui/render/detail/mod.rs`,
  `src/tui/render/mod.rs`, `src/tui/render/sidebar.rs`,
  `src/tui/update/mod.rs`, `src/notify.rs`, `tests/golden.rs`
- Modify (produção, `window_kind: None` temporário — Tasks 2-4 substituem):
  `src/providers/claude.rs` (linhas 120-127, 180-187),
  `src/providers/codex/normalize.rs` (linhas 26-34),
  `src/providers/amp.rs` (linhas 94-100, 140-146, 163-169),
  `src/providers/grok.rs` (linhas 295-307)

**Interfaces:**
- Produces: `pub enum WindowKind { FiveHour, SevenDay, Daily, Context, Other }`
  em `src/providers/types.rs` — `#[derive(Debug, Clone, Copy, PartialEq, Eq,
  Serialize)]`, `#[serde(rename_all = "camelCase")]`. Campo `pub window_kind:
  Option<WindowKind>` em `QuotaWindow`, `#[serde(rename = "windowKind",
  skip_serializing_if = "Option::is_none")]`.
- Consumes (Task 2-9): toda construção de `QuotaWindow` no crate.

- [ ] Escrever teste que falha em `src/providers/types.rs` (dentro de `mod
      tests`, dedica um bloco novo — cole logo antes do fechamento do
      arquivo, depois de `stale_reason_omitted_when_none_present_when_some`):

  ```rust
  #[test]
  fn window_kind_serializes_camelcase_variants() {
      let cases = [
          (WindowKind::FiveHour, "fiveHour"),
          (WindowKind::SevenDay, "sevenDay"),
          (WindowKind::Daily, "daily"),
          (WindowKind::Context, "context"),
          (WindowKind::Other, "other"),
      ];
      for (kind, expected) in cases {
          let w = QuotaWindow {
              remaining: 50.0,
              resets_at: None,
              window_minutes: None,
              used: None,
              severity: None,
              window_kind: Some(kind),
          };
          let j = serde_json::to_value(&w).unwrap();
          assert_eq!(j["windowKind"], expected, "kind={kind:?}");
      }
  }

  #[test]
  fn window_kind_omitted_when_none() {
      let w = window(50.0); // helper já existente do módulo
      let j = serde_json::to_value(&w).unwrap();
      assert!(
          j.get("windowKind").is_none(),
          "windowKind deve ser omitido quando None"
      );
  }
  ```

  Ainda não compila: `WindowKind` não existe e o helper `window()` (linha
  138-146) não tem o campo `window_kind`.

- [ ] Rodar `cargo test types` e ver falhar (erro de compilação, não de
      assert): `error[E0433]: failed to resolve: use of undeclared type
      `WindowKind`` (ou similar, `no field `window_kind` on type
      `QuotaWindow``).

- [ ] Implementar o enum + campo em `src/providers/types.rs`. Substituir o
      struct `QuotaWindow` (linhas 10-25):

  ```rust
  #[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
  #[serde(rename_all = "camelCase")]
  pub enum WindowKind {
      FiveHour,
      SevenDay,
      Daily,
      Context,
      Other,
  }

  #[derive(Debug, Clone, PartialEq, Serialize)]
  #[serde(rename_all = "camelCase")]
  pub struct QuotaWindow {
      pub remaining: f64,
      /// Sempre presente no JSON (pode ser null).
      pub resets_at: Option<String>,
      #[serde(skip_serializing_if = "Option::is_none")]
      pub window_minutes: Option<i64>,
      #[serde(skip_serializing_if = "Option::is_none")]
      pub used: Option<f64>,
      /// Severidade vinda da API (`limits[].severity` do Claude). `None` =
      /// calcular localmente por threshold. Omitida do JSON quando ausente
      /// (mantém golden/waybar_contract intactos).
      #[serde(skip_serializing_if = "Option::is_none")]
      pub severity: Option<String>,
      /// Classificação da janela decidida UMA VEZ na origem (provider).
      /// `None` só em dado sintético de teste antigo; produção sempre seta.
      /// Rótulos de UI (TUI/QML) derivam só disso, nunca de magic numbers.
      #[serde(rename = "windowKind", skip_serializing_if = "Option::is_none")]
      pub window_kind: Option<WindowKind>,
  }
  ```

- [ ] Atualizar os 2 helpers de teste do próprio arquivo pra adicionar o
      campo. `window()` (linhas 138-146):

  ```rust
  fn window(remaining: f64) -> QuotaWindow {
      QuotaWindow {
          remaining,
          resets_at: Some("2026-06-19T14:00:00Z".into()),
          window_minutes: None,
          used: None,
          severity: None,
          window_kind: None,
      }
  }
  ```

  E o literal standalone em `quota_window_keeps_null_resets_at` (linhas
  162-168):

  ```rust
  let w = QuotaWindow {
      remaining: 100.0,
      resets_at: None,
      window_minutes: Some(300),
      used: None,
      severity: None,
      window_kind: None,
  };
  ```

- [ ] Rodar `cargo test types` — ainda falha (agora por erro de compilação
      em OUTROS arquivos do crate: todo `QuotaWindow { .. }` sem o campo
      novo). Confirmar que a lista de erros bate com os arquivos do Step 5.

- [ ] **Fix mecânico** — adicionar `window_kind: None,` (mesma indentação da
      linha `severity: None,` vizinha) em cada um dos sites abaixo. Nenhuma
      lógica muda, só o campo novo pra satisfazer o compilador:

  `src/notify.rs:262` (helper `win`) e `:403` (literal em
  `honors_provider_used_over_100`):
  ```rust
  fn win(remaining: f64, resets: Option<&str>) -> QuotaWindow {
      QuotaWindow {
          remaining,
          resets_at: resets.map(str::to_string),
          window_minutes: None,
          used: None,
          severity: None,
          window_kind: None,
      }
  }
  ```
  ```rust
  c.primary = Some(QuotaWindow {
      remaining: 0.0,
      resets_at: None,
      window_minutes: None,
      used: Some(232.0),
      severity: None,
      window_kind: None,
  });
  ```

  `src/formatters/shared.rs:315` (literal em
  `to_window_display_honours_provider_used`):
  ```rust
  let w = QuotaWindow {
      remaining: 30.0,
      resets_at: None,
      window_minutes: None,
      used: Some(70.0),
      severity: None,
      window_kind: None,
  };
  ```

  `src/formatters/builders/amp.rs:401` (helper `win`):
  ```rust
  fn win(r: f64) -> QuotaWindow {
      QuotaWindow {
          remaining: r,
          resets_at: Some("2026-06-19T14:00:00Z".into()),
          window_minutes: None,
          used: None,
          severity: None,
          window_kind: None,
      }
  }
  ```

  `src/formatters/builders/claude.rs:195` (helper `win`):
  ```rust
  fn win(r: f64, m: Option<i64>) -> QuotaWindow {
      QuotaWindow {
          remaining: r,
          resets_at: Some("2026-06-19T14:00:00Z".into()),
          window_minutes: m,
          used: None,
          severity: None,
          window_kind: None,
      }
  }
  ```

  `src/formatters/builders/codex.rs:219-225` (closure `w` dentro de
  `entry`):
  ```rust
  fn entry(name: &str, five: f64, seven: f64) -> CodexModelEntry {
      let w = |r: f64| QuotaWindow {
          remaining: r,
          resets_at: Some("2026-06-19T14:00:00Z".into()),
          window_minutes: None,
          used: None,
          severity: None,
          window_kind: None,
      };
      CodexModelEntry {
          name: name.into(),
          windows: ModelWindows {
              five_hour: Some(w(five)),
              seven_day: Some(w(seven)),
              other: None,
          },
          severity: five.min(seven),
      }
  }
  ```

  `src/formatters/builders/generic.rs:99-105` (literal em `quota`):
  ```rust
  primary: Some(QuotaWindow {
      remaining: 60.0,
      resets_at: None,
      window_minutes: None,
      used: None,
      severity: None,
      window_kind: None,
  }),
  ```

  `src/formatters/builders/grok.rs:173-179` (literal em
  `quota_with_primary`):
  ```rust
  primary: Some(QuotaWindow {
      remaining: 87.0,
      resets_at: None,
      window_minutes: None,
      used: Some(13.0),
      severity: None,
      window_kind: None,
  }),
  ```

  `src/formatters/builders/shared.rs:218-224` (literal em
  `model_line_segment_shape`) e `:245-251` (literal em
  `model_line_null_eta_override`):
  ```rust
  let w = QuotaWindow {
      remaining: 75.0,
      resets_at: Some("2026-06-19T14:00:00Z".into()),
      window_minutes: None,
      used: None,
      severity: None,
      window_kind: None,
  };
  ```
  ```rust
  let w = QuotaWindow {
      remaining: 50.0,
      resets_at: None,
      window_minutes: None,
      used: None,
      severity: None,
      window_kind: None,
  };
  ```

  `src/formatters/codex_helpers.rs:112-118` (helper `win`):
  ```rust
  fn win(remaining: f64, minutes: Option<i64>) -> QuotaWindow {
      QuotaWindow {
          remaining,
          resets_at: Some("2026-06-19T14:00:00Z".into()),
          window_minutes: minutes,
          used: None,
          severity: None,
          window_kind: None,
      }
  }
  ```

  `src/formatters/json.rs:44-50` (literal inline no vetor `providers`):
  ```rust
  primary: Some(QuotaWindow {
      remaining: 60.0,
      resets_at: None,
      window_minutes: None,
      used: None,
      severity: None,
      window_kind: None,
  }),
  ```

  `src/formatters/terminal.rs:188-194` (literal em `claude`):
  ```rust
  primary: Some(QuotaWindow {
      remaining: 75.0,
      resets_at: Some("2026-03-28T14:00:00Z".into()),
      window_minutes: Some(300),
      used: None,
      severity: None,
      window_kind: None,
  }),
  ```

  `src/formatters/view_model.rs:56-62` (literal em `codex_quota`):
  ```rust
  QuotaWindow {
      remaining: 80.0,
      resets_at: None,
      window_minutes: Some(300),
      used: None,
      severity: None,
      window_kind: None,
  },
  ```

  `src/formatters/waybar.rs:331-337` (literal em `claude`):
  ```rust
  primary: Some(QuotaWindow {
      remaining,
      resets_at: Some("2026-03-28T14:00:00Z".into()),
      window_minutes: Some(300),
      used: None,
      severity: None,
      window_kind: None,
  }),
  ```

  `src/tui/render/detail/mod.rs:298-304` (helper `window`):
  ```rust
  fn window(remaining: f64, resets_at: Option<&str>, severity: Option<&str>) -> QuotaWindow {
      QuotaWindow {
          remaining,
          resets_at: resets_at.map(|s| s.to_string()),
          window_minutes: Some(300),
          used: Some(100.0 - remaining),
          severity: severity.map(|s| s.to_string()),
          window_kind: None,
      }
  }
  ```

  `src/tui/render/mod.rs:333-339` (literal inline):
  ```rust
  primary: Some(QuotaWindow {
      remaining,
      resets_at: resets_at.map(|s| s.to_string()),
      window_minutes: Some(300),
      used: Some(100.0 - remaining),
      severity: None,
      window_kind: None,
  }),
  ```

  `src/tui/render/sidebar.rs:204-210` (literal em `make_provider`):
  ```rust
  primary: Some(QuotaWindow {
      remaining,
      resets_at: None,
      window_minutes: Some(300),
      used: Some(100.0 - remaining),
      severity: None,
      window_kind: None,
  }),
  ```

  `src/tui/update/mod.rs:108-114` (literal em `test_quota`):
  ```rust
  q.primary = Some(QuotaWindow {
      remaining,
      resets_at: None,
      window_minutes: None,
      used: Some(100.0 - remaining),
      severity: None,
      window_kind: None,
  });
  ```

  `tests/golden.rs:62-70` (helper `qw`, usado por todo o arquivo):
  ```rust
  fn qw(remaining: f64, resets_at: &str, window_minutes: Option<i64>) -> QuotaWindow {
      QuotaWindow {
          remaining,
          resets_at: Some(resets_at.into()),
          window_minutes,
          used: None,
          severity: None,
          window_kind: None,
      }
  }
  ```

  Produção (`window_kind: None` temporário — Tasks 2-4 trocam por
  `Some(kind)`):

  `src/providers/claude.rs:118-127` (`window_from`) e `:178-187`
  (`window_from_limit`):
  ```rust
  fn window_from(raw: &ClaudeWindowRaw) -> QuotaWindow {
      let used = raw.utilization.round();
      QuotaWindow {
          remaining: 100.0 - used,
          resets_at: raw.resets_at.clone().filter(|s| !s.is_empty()),
          window_minutes: None,
          used: None,
          severity: None,
          window_kind: None,
      }
  }
  ```
  ```rust
  fn window_from_limit(l: &ClaudeLimitRaw) -> QuotaWindow {
      let used = l.percent.unwrap_or(0.0).round();
      QuotaWindow {
          remaining: 100.0 - used,
          resets_at: l.resets_at.clone().filter(|s| !s.is_empty()),
          window_minutes: None,
          used: Some(used),
          severity: l.severity.clone(),
          window_kind: None,
      }
  }
  ```

  `src/providers/codex/normalize.rs:26-34` (`to_quota_window`):
  ```rust
  fn to_quota_window(raw: &CodexWindowRaw) -> QuotaWindow {
      QuotaWindow {
          remaining: 100.0 - raw.used_percent.round(),
          resets_at: unix_to_iso(raw.resets_at),
          window_minutes: Some(raw.window_minutes),
          used: None,
          severity: None,
          window_kind: None,
      }
  }
  ```

  `src/providers/amp.rs:94-100`, `:140-146`, `:163-169` (3 literais):
  ```rust
  let window = QuotaWindow {
      remaining: pct,
      resets_at,
      window_minutes: None,
      used: None,
      severity: None,
      window_kind: None,
  };
  ```
  ```rust
  let window = QuotaWindow {
      remaining: pct,
      resets_at: full_at.clone(),
      window_minutes: None,
      used: None,
      severity: None,
      window_kind: None,
  };
  ```
  ```rust
  QuotaWindow {
      remaining: if balance > 0.0 { 100.0 } else { 0.0 },
      resets_at: None,
      window_minutes: None,
      used: None,
      severity: None,
      window_kind: None,
  },
  ```

  `src/providers/grok.rs:295-307` (`build_primary_window`):
  ```rust
  fn build_primary_window(session: &SessionSnap) -> Option<QuotaWindow> {
      let used = session.context_tokens_used?;
      let window = session.context_window_tokens.filter(|w| *w > 0)?;
      let remaining = context_remaining_pct(used, window)?;
      let used_pct = 100.0 * (used as f64) / (window as f64);
      Some(QuotaWindow {
          remaining: remaining.round(),
          resets_at: None,
          window_minutes: None,
          used: Some(used_pct.round()),
          severity: None,
          window_kind: None,
      })
  }
  ```

- [ ] Fazer `formatters/shared.rs` reexportar o enum canônico em vez de
      definir o seu próprio. Remover o bloco (linhas 39-44):
      ```rust
      #[derive(Debug, Clone, Copy, PartialEq, Eq)]
      pub enum WindowKind {
          FiveHour,
          SevenDay,
          Other,
      }
      ```
      e substituir a linha 3 (`use crate::providers::types::QuotaWindow;`)
      por um `use` + um `pub use` — o `pub use` é o que faz
      `crate::formatters::shared::WindowKind` continuar resolvendo pra quem
      já importa de lá (`codex_helpers.rs`, `codex/normalize.rs`), sem
      precisar tocar nesses 2 arquivos:
      ```rust
      use crate::providers::types::QuotaWindow;
      pub use crate::providers::types::WindowKind;
      ```
      `classify_window` (linhas 47-60) não muda de corpo,
      só o tipo de retorno passa a resolver pro enum de 5 variantes (mesma
      API pública: assinatura `pub fn classify_window(minutes: Option<i64>)
      -> WindowKind` idêntica).

- [ ] Rodar `cargo test types` e ver passar (todo o crate compila; os 2
      testes novos de serialização passam).

- [ ] Rodar `cargo test formatters` (cobre `classify_window`,
      `to_window_display`, os builders) e `cargo test providers::codex` e
      `cargo test providers::claude` e `cargo test providers::amp` e
      `cargo test providers::grok` — todos verdes (comportamento idêntico,
      só o campo novo em `None`).

- [ ] `cargo clippy --all-targets -- -D warnings` — zero warnings.

- [ ] Commit:
  ```
  git add -A
  git commit -m "feat: adiciona windowKind a QuotaWindow"
  ```

---

### Task 2: Claude seta `window_kind` na origem (legacy + limits[])

**Files:**
- Modify: `src/providers/claude.rs` (import linha 14; `window_from` linhas
  118-127; `window_from_limit` linhas 178-187; `quota_from_limits` linhas
  205-221; `quota_from_usage` linhas 452-464; testes
  `claude_limits_block_takes_precedence_over_legacy` linhas 893-932 e
  `claude_falls_back_to_legacy_when_limits_absent` linhas 934-962)

**Interfaces:**
- Consumes: `WindowKind` de `super::types` (Task 1).
- Produces: `primary`/`secondary`/`models["Opus"|"Sonnet"|"Cowork"|"Fable"]`
  do Claude sempre com `window_kind` correto (nunca `None` em produção).

- [ ] Estender os 2 testes end-to-end existentes com asserts de
      `window_kind` (ainda vão falhar — `window_from`/`window_from_limit`
      não recebem `kind` ainda). Em `claude_limits_block_takes_precedence_over_legacy`
      (depois de `assert_eq!(p.used, Some(11.0));`, `assert_eq!(s.remaining,
      97.0);` e `assert_eq!(models.get("Fable").unwrap().remaining,
      97.0);` respectivamente):
      ```rust
      assert_eq!(p.window_kind, Some(crate::providers::types::WindowKind::FiveHour));
      ```
      ```rust
      assert_eq!(s.window_kind, Some(crate::providers::types::WindowKind::SevenDay));
      ```
      ```rust
      assert_eq!(
          models.get("Fable").unwrap().window_kind,
          Some(crate::providers::types::WindowKind::SevenDay)
      );
      ```
      Em `claude_falls_back_to_legacy_when_limits_absent` (depois de
      `assert_eq!(p.remaining, 75.0);` e `assert_eq!(q.secondary.as_ref().unwrap().remaining, 60.0);`):
      ```rust
      assert_eq!(p.window_kind, Some(crate::providers::types::WindowKind::FiveHour));
      ```
      ```rust
      assert_eq!(
          q.secondary.as_ref().unwrap().window_kind,
          Some(crate::providers::types::WindowKind::SevenDay)
      );
      ```

- [ ] Rodar `cargo test providers::claude` e ver falhar: `assertion failed:
      \`(left == right)\`` — `left: None, right: Some(FiveHour)`.

- [ ] Implementar: adicionar `WindowKind` ao import (linha 14):
      ```rust
      use super::types::{ClaudeQuotaExtra, ExtraUsage, ProviderExtra, ProviderQuota, QuotaWindow, WindowKind};
      ```
      Dar um parâmetro `kind: WindowKind` pras 2 funções construtoras:
      ```rust
      fn window_from(raw: &ClaudeWindowRaw, kind: WindowKind) -> QuotaWindow {
          let used = raw.utilization.round();
          QuotaWindow {
              remaining: 100.0 - used,
              resets_at: raw.resets_at.clone().filter(|s| !s.is_empty()),
              window_minutes: None,
              used: None,
              severity: None,
              window_kind: Some(kind),
          }
      }
      ```
      ```rust
      fn window_from_limit(l: &ClaudeLimitRaw, kind: WindowKind) -> QuotaWindow {
          let used = l.percent.unwrap_or(0.0).round();
          QuotaWindow {
              remaining: 100.0 - used,
              resets_at: l.resets_at.clone().filter(|s| !s.is_empty()),
              window_minutes: None,
              used: Some(used),
              severity: l.severity.clone(),
              window_kind: Some(kind),
          }
      }
      ```
      Atualizar `quota_from_limits` (linhas 205-221) pra passar o kind certo
      por `kind`:
      ```rust
      for l in &u.limits {
          match l.kind.as_str() {
              "session" => primary = Some(window_from_limit(l, WindowKind::FiveHour)),
              "weekly_all" => secondary = Some(window_from_limit(l, WindowKind::SevenDay)),
              "weekly_scoped" => {
                  let name = l
                      .scope
                      .as_ref()
                      .and_then(|s| s.model.as_ref())
                      .and_then(|m| m.display_name.clone());
                  if let Some(name) = name {
                      weekly.insert(name, window_from_limit(l, WindowKind::SevenDay));
                  }
              }
              other => log::debug!("Claude limits[]: kind desconhecido ignorado: {other}"),
          }
      }
      ```
      Atualizar o branch legado de `quota_from_usage` (linhas 452-464):
      ```rust
      let primary = usage.five_hour.as_ref().map(|w| window_from(w, WindowKind::FiveHour));
      let secondary = usage.seven_day.as_ref().map(|w| window_from(w, WindowKind::SevenDay));
      let mut weekly: IndexMap<String, QuotaWindow> = IndexMap::new();
      if let Some(w) = usage.seven_day_opus.as_ref() {
          weekly.insert("Opus".to_string(), window_from(w, WindowKind::SevenDay));
      }
      if let Some(w) = usage.seven_day_sonnet.as_ref() {
          weekly.insert("Sonnet".to_string(), window_from(w, WindowKind::SevenDay));
      }
      if let Some(w) = usage.seven_day_cowork.as_ref() {
          weekly.insert("Cowork".to_string(), window_from(w, WindowKind::SevenDay));
      }
      ```

- [ ] Rodar `cargo test providers::claude` e ver passar.

- [ ] `cargo clippy --all-targets -- -D warnings`.

- [ ] Commit:
  ```
  git add -A
  git commit -m "feat: claude seta windowKind na origem"
  ```

---

### Task 3: Codex — `to_quota_window` seta kind + mata o fallback forçado

Este é o fix do bug central da auditoria (Codex duplicado). O fallback
incondicional forçava `primary→fiveHour`/`secondary→sevenDay` mesmo quando
`classify_window` já tinha classificado como `other` — resultado: a mesma
janela aparecia 2x (uma em `other`, outra forçada em `fiveHour`/`sevenDay`).

**Files:**
- Modify: `src/providers/codex/normalize.rs` (`to_quota_window` linhas
  26-34; `place_window` linhas 66-73; `build_model_windows` — remove
  fallback linhas 88-98 e linhas 125-134)
- Modify: `src/providers/codex/mod.rs` (teste `unrecognized_window_uses_fallback_mapping`
  linhas 520-547 — reescrito e renomeado)

**Interfaces:**
- Consumes: `classify_window`/`WindowKind` de `formatters::shared` (já
  importados na linha 8 do arquivo).
- Produces: toda `ModelWindows` do Codex (`five_hour`/`seven_day`/`other[]`)
  com `window_kind` correto e SEM duplicação — janela fora de tolerância
  fica só em `other`, nunca também forçada em `five_hour`/`seven_day`.

- [ ] Reescrever o teste que documentava o bug (linhas 520-547) pro novo
      comportamento — renomeado porque o fallback que ele documentava não
      existe mais. Apagar o teste antigo POR NOME: localizar a linha
      `#[test]` que precede `fn unrecognized_window_uses_fallback_mapping`
      e apagar tudo até o `}` que fecha essa função, colando o teste novo
      no lugar (não confiar só no range de linhas citado):
      ```rust
      #[test]
      fn unrecognized_window_stays_other_no_fallback() {
          // 60 min = "other" via classify_window; SEM fallback, fica só
          // em `other` (nunca forçado em fiveHour/sevenDay) — é o fix do
          // bug de duplicação do Codex (auditoria 2026-07-21).
          let mut buckets = IndexMap::new();
          buckets.insert(
              "b1".to_string(),
              CodexLimitBucket {
                  limit_id: "b1".into(),
                  limit_name: None,
                  primary: Some(win(10.0, 60, future_unix())),
                  secondary: Some(win(20.0, 60, future_unix())),
              },
          );
          let limits = CodexRateLimits {
              buckets: Some(buckets),
              ..Default::default()
          };
          let q = build_codex_quota(&limits, base());
          let md = codex_extra(&q).models_detailed.as_ref().unwrap();
          let model = md.values().next().unwrap();
          assert!(model.five_hour.is_none(), "60min não deve ir pra fiveHour");
          assert!(model.seven_day.is_none(), "60min não deve ir pra sevenDay");
          let other = model.other.as_ref().expect("other deve ter as 2 janelas");
          assert_eq!(other.len(), 2, "primary+secondary de 60min, ambas em other");
          for w in other {
              assert_eq!(
                  w.window_kind,
                  Some(crate::formatters::shared::WindowKind::Other)
              );
          }
      }
      ```

- [ ] Rodar `cargo test providers::codex` e ver falhar: o teste antigo
      esperava `model.five_hour.is_some()` — com o nome novo, o compilador
      vai reclamar de método/campo inexistente até o rename ser aplicado
      (o teste antigo já não existe mais no arquivo após o Step acima), e
      os asserts novos falham porque o fallback ainda está no código
      (`model.five_hour.is_none()` falha, pois hoje é `Some`).

- [ ] Implementar: `to_quota_window` passa a setar `window_kind` via
      `classify_window` (linhas 26-34):
      ```rust
      fn to_quota_window(raw: &CodexWindowRaw) -> QuotaWindow {
          QuotaWindow {
              remaining: 100.0 - raw.used_percent.round(),
              resets_at: unix_to_iso(raw.resets_at),
              window_minutes: Some(raw.window_minutes),
              used: None,
              severity: None,
              window_kind: Some(classify_window(Some(raw.window_minutes))),
          }
      }
      ```
      `place_window` deixa de chamar `classify_window` de novo — reusa o
      `window_kind` que `to_quota_window` já calculou (kind decidido UMA
      única vez, linhas 66-73):
      ```rust
      fn place_window(windows: &mut ModelWindows, raw: &CodexWindowRaw) {
          let qw = to_quota_window(raw);
          match qw.window_kind {
              Some(WindowKind::FiveHour) if windows.five_hour.is_none() => {
                  windows.five_hour = Some(qw)
              }
              Some(WindowKind::SevenDay) if windows.seven_day.is_none() => {
                  windows.seven_day = Some(qw)
              }
              _ => windows.other.get_or_insert_with(Vec::new).push(qw),
          }
      }
      ```
      Remover o fallback forçado dentro do loop de buckets (linhas 88-98 —
      o comentário `// Fallback de mapeamento quando as durações não
      classificam limpo.` e os dois `if windows.five_hour.is_none() { .. }`
      / `if windows.seven_day.is_none() { .. }` que seguem): antes de
      apagar, reler o arquivo com Read e localizar o bloco pelo
      comentário `// Fallback de mapeamento` (não confiar só no range de
      linhas citado — pode ter deslocado). Apagar as linhas inteiras,
      deixando o código ir direto de `place_window(&mut windows, raw);`
      (fim do loop de `bucket.primary`/`bucket.secondary`) pro check `if
      windows.five_hour.is_none() && windows.seven_day.is_none() && ...`
      que decide o `continue`.
      Remover o fallback equivalente no branch legado (linhas 125-134 — os
      dois blocos `if windows.five_hour.is_none() { .. }` / `if
      windows.seven_day.is_none() { .. }` logo antes de `models.insert("Codex".to_string(),
      windows);`): apagar, deixando o `for raw in [...] { place_window(&mut
      windows, raw); }` seguido direto por `models.insert(...)`.

- [ ] Rodar `cargo test providers::codex` e ver passar (incluindo os outros
      testes de tolerância — `tolerates_five_hour_within_90min`,
      `tolerates_seven_day_within_1440min` — que continuam verdes porque
      `place_window` já classificava certo pra esses casos independente do
      fallback).

- [ ] `cargo clippy --all-targets -- -D warnings`.

- [ ] Commit:
  ```
  git add -A
  git commit -m "fix: codex para de forcar fallback five/seven"
  ```

---

### Task 4: Amp (`Daily`) e Grok (`Context`)

**Files:**
- Modify: `src/providers/amp.rs` (import linha 17; 3 literais linhas 94-100,
  140-146, 163-169; testes `parses_full_output` linha 356 e
  `fixture_free_pct_parses_primary` linha 407)
- Modify: `src/providers/grok.rs` (import linha 17; `build_primary_window`
  linhas 295-307; teste `happy_path_remaining_90` linha 648)

**Interfaces:**
- Consumes: `WindowKind` de `super::types` (Task 1).
- Produces: toda janela do Amp (`primary`, `models["Free Tier"]`,
  `models["Credits"]`) com `window_kind: Some(WindowKind::Daily)`; a janela
  de contexto do Grok (`primary`) com `window_kind:
  Some(WindowKind::Context)`.

- [ ] Estender 2 testes existentes do Amp com o assert de `window_kind`
      (ainda falha — campo é `None`). Em `parses_full_output` (depois de
      `assert_eq!(models["Credits"].remaining, 100.0);`):
      ```rust
      assert_eq!(
          q.primary.as_ref().unwrap().window_kind,
          Some(crate::providers::types::WindowKind::Daily)
      );
      assert_eq!(
          models["Credits"].window_kind,
          Some(crate::providers::types::WindowKind::Daily)
      );
      ```
      Em `fixture_free_pct_parses_primary` (depois de
      `assert_eq!(models["Credits"].remaining, 100.0);`):
      ```rust
      assert_eq!(
          models["Free Tier"].window_kind,
          Some(crate::providers::types::WindowKind::Daily)
      );
      ```

- [ ] Estender `happy_path_remaining_90` do Grok (depois de
      `assert_eq!(primary.used, Some(10.0));`):
      ```rust
      assert_eq!(
          primary.window_kind,
          Some(crate::providers::types::WindowKind::Context)
      );
      ```

- [ ] Rodar `cargo test providers::amp` e `cargo test providers::grok` e
      ver falhar (`left: None, right: Some(Daily)` / `Some(Context)`).

- [ ] Implementar Amp: adicionar `WindowKind` ao import (linha 17):
      ```rust
      use super::types::{AmpQuotaExtra, ProviderExtra, ProviderQuota, QuotaWindow, WindowKind};
      ```
      Setar `window_kind: Some(WindowKind::Daily)` nos 3 literais (linhas
      94-100, 140-146, 163-169):
      ```rust
      let window = QuotaWindow {
          remaining: pct,
          resets_at,
          window_minutes: None,
          used: None,
          severity: None,
          window_kind: Some(WindowKind::Daily),
      };
      ```
      ```rust
      let window = QuotaWindow {
          remaining: pct,
          resets_at: full_at.clone(),
          window_minutes: None,
          used: None,
          severity: None,
          window_kind: Some(WindowKind::Daily),
      };
      ```
      ```rust
      QuotaWindow {
          remaining: if balance > 0.0 { 100.0 } else { 0.0 },
          resets_at: None,
          window_minutes: None,
          used: None,
          severity: None,
          window_kind: Some(WindowKind::Daily),
      },
      ```

- [ ] Implementar Grok: adicionar `WindowKind` ao import (linha 17):
      ```rust
      use super::types::{GrokQuotaExtra, ProviderExtra, ProviderQuota, QuotaWindow, WindowKind};
      ```
      Setar `window_kind: Some(WindowKind::Context)` em
      `build_primary_window` (linhas 295-307):
      ```rust
      fn build_primary_window(session: &SessionSnap) -> Option<QuotaWindow> {
          let used = session.context_tokens_used?;
          let window = session.context_window_tokens.filter(|w| *w > 0)?;
          let remaining = context_remaining_pct(used, window)?;
          let used_pct = 100.0 * (used as f64) / (window as f64);
          Some(QuotaWindow {
              remaining: remaining.round(),
              resets_at: None,
              window_minutes: None,
              used: Some(used_pct.round()),
              severity: None,
              window_kind: Some(WindowKind::Context),
          })
      }
      ```

- [ ] Rodar `cargo test providers::amp` e `cargo test providers::grok` e
      ver passar.

- [ ] `cargo clippy --all-targets -- -D warnings`.

- [ ] Commit:
  ```
  git add -A
  git commit -m "feat: amp/grok setam windowKind (daily/context)"
  ```

---

### Task 5: `classify_window` → `WindowKind` canônico + `is_duplicate_window`

**Files:**
- Modify: `src/formatters/shared.rs` (imports linha 3; enum local removido
  na Task 1 — aqui só falta adicionar `is_duplicate_window`; novo bloco de
  testes)

**Interfaces:**
- Produces: `pub fn is_duplicate_window(a: &QuotaWindow, b: &QuotaWindow) ->
  bool` — true quando `(window_kind, resets_at, remaining.round())`
  coincidem nas duas janelas; `window_kind` ausente (`None`) em qualquer
  lado nunca é considerado duplicata (dado incompleto não deduplica).
- Consumes (Task 7): TUI (`src/tui/render/detail/windows.rs`).

- [ ] Escrever teste que falha, no bloco `mod tests` existente de
      `formatters/shared.rs` (depois de `to_window_display_honours_provider_used`):
      ```rust
      #[test]
      fn is_duplicate_window_same_kind_reset_and_rounded_remaining() {
          let a = QuotaWindow {
              remaining: 60.4,
              resets_at: Some("2026-06-26T12:00:00Z".into()),
              window_minutes: Some(10080),
              used: None,
              severity: None,
              window_kind: Some(WindowKind::SevenDay),
          };
          let b = QuotaWindow {
              remaining: 59.6,
              resets_at: Some("2026-06-26T12:00:00Z".into()),
              window_minutes: Some(300),
              used: None,
              severity: None,
              window_kind: Some(WindowKind::SevenDay),
          };
          // 60.4.round() == 59.6.round() == 60 — remaining "igual" mesmo
          // vindo de fontes com precisão diferente.
          assert!(is_duplicate_window(&a, &b));
      }

      #[test]
      fn is_duplicate_window_different_kind_is_not_duplicate() {
          let mut a = QuotaWindow {
              remaining: 60.0,
              resets_at: Some("2026-06-26T12:00:00Z".into()),
              window_minutes: Some(300),
              used: None,
              severity: None,
              window_kind: Some(WindowKind::FiveHour),
          };
          let b = QuotaWindow {
              window_kind: Some(WindowKind::SevenDay),
              ..a.clone()
          };
          assert!(!is_duplicate_window(&a, &b));
          a.window_kind = None;
          let c = QuotaWindow {
              window_kind: None,
              ..b.clone()
          };
          assert!(!is_duplicate_window(&a, &c), "window_kind None nunca duplica");
      }

      #[test]
      fn is_duplicate_window_different_reset_is_not_duplicate() {
          let a = QuotaWindow {
              remaining: 60.0,
              resets_at: Some("2026-06-26T12:00:00Z".into()),
              window_minutes: Some(10080),
              used: None,
              severity: None,
              window_kind: Some(WindowKind::SevenDay),
          };
          let b = QuotaWindow {
              resets_at: Some("2026-06-27T12:00:00Z".into()),
              ..a.clone()
          };
          assert!(!is_duplicate_window(&a, &b));
      }
      ```

- [ ] Rodar `cargo test formatters` e ver falhar: `error[E0425]: cannot
      find function `is_duplicate_window` in this scope`.

- [ ] Implementar em `src/formatters/shared.rs`, logo depois de
      `classify_window` (linha 60):
      ```rust
      /// Dedup display-level (TUI/QML): duas janelas são a "mesma" quando
      /// `windowKind`, `resetsAt` e `remaining` arredondado coincidem —
      /// mata o Weekly triplicado do Codex sem tocar no JSON (que continua
      /// emitindo primary/secondary/models cru, contrato intacto).
      /// `window_kind` ausente em qualquer lado nunca deduplica (dado
      /// incompleto não é assumido igual).
      pub fn is_duplicate_window(a: &QuotaWindow, b: &QuotaWindow) -> bool {
          match (a.window_kind, b.window_kind) {
              (Some(ka), Some(kb)) if ka == kb => {
                  a.resets_at == b.resets_at && a.remaining.round() == b.remaining.round()
              }
              _ => false,
          }
      }
      ```

- [ ] Rodar `cargo test formatters` e ver passar.

- [ ] `cargo clippy --all-targets -- -D warnings`.

- [ ] Commit:
  ```
  git add -A
  git commit -m "feat: is_duplicate_window para dedup display-level"
  ```

---

### Task 6: Builder do Codex renderiza janelas `other`

Sem o fallback (Task 3), uma janela de duração não-padrão fica só em
`ModelWindows.other` — hoje o builder terminal/waybar do Codex nunca lê
esse campo, então ela vira invisível. Rótulo pela duração real (ex.: "1h
window"), sempre visível independente da `WindowPolicy` (não é nem
`FiveHour` nem `SevenDay`, então nenhuma das duas políticas deveria
escondê-la).

**Files:**
- Modify: `src/formatters/builders/codex.rs` (novo helper + chamada dentro
  de `build_codex`, inserido depois do bloco `if policy !=
  WindowPolicy::FiveHour { .. }`, linha 117-118, antes do bloco `if let
  Some(eu) = get_codex_extra(p)...` linha 120; novo teste no módulo)
- tests/golden.rs — avaliado e mantido sem mudança (ver step: fixture não
  muda neste commit; caso other coberto por teste unitário no builder)

**Interfaces:**
- Consumes: `ModelWindows.other: Option<Vec<QuotaWindow>>` (já existe desde
  antes deste plano).
- Produces: linha extra por janela `other`, rotulada por duração
  (`"{h}h window"` / `"{m}m window"` / `"{h}h {m}m window"`).

- [ ] Escrever teste que falha, dentro de `mod tests` de
      `src/formatters/builders/codex.rs` (depois de
      `empty_models_shows_placeholder`):
      ```rust
      fn entry_with_other(name: &str, other_minutes: i64, remaining: f64) -> CodexModelEntry {
          CodexModelEntry {
              name: name.into(),
              windows: ModelWindows {
                  five_hour: None,
                  seven_day: None,
                  other: Some(vec![QuotaWindow {
                      remaining,
                      resets_at: Some("2026-06-19T14:00:00Z".into()),
                      window_minutes: Some(other_minutes),
                      used: None,
                      severity: None,
                      window_kind: Some(crate::providers::types::WindowKind::Other),
                  }]),
              },
              severity: remaining,
          }
      }

      #[test]
      fn other_window_renders_with_duration_label() {
          let vm = CodexViewModel {
              models: vec![entry_with_other("gpt-5", 60, 40.0)],
              policy: WindowPolicy::Both,
          };
          let out = render_pango(&build_codex(&clk(), &quota(), &vm, &opts(None)));
          assert!(out.contains("1h window"), "esperava rótulo de duração:\n{out}");
          assert!(out.contains("40%"));
      }

      #[test]
      fn other_window_label_formats_by_minutes() {
          assert_eq!(other_window_label(Some(60)), "1h window");
          assert_eq!(other_window_label(Some(90)), "1h 30m window");
          assert_eq!(other_window_label(Some(45)), "45m window");
          assert_eq!(other_window_label(None), "window");
      }
      ```

- [ ] Rodar `cargo test formatters::builders::codex` e ver falhar:
      `error[E0425]: cannot find function `other_window_label``.

- [ ] Implementar. Helper de rótulo (colar antes de `pub fn build_codex`,
      depois dos imports):
      ```rust
      /// Rótulo de janela `other` (nem 5h nem 7d) pela duração real — o
      /// fallback forçado que escondia isso morreu na Task 3. "1h
      /// window"/"45m window"/"1h 30m window"; sem `window_minutes` (não
      /// deveria acontecer em dado real) → "window", nunca panic.
      fn other_window_label(minutes: Option<i64>) -> String {
          match minutes {
              Some(m) if m > 0 => {
                  let h = m / 60;
                  let mm = m % 60;
                  match (h, mm) {
                      (0, _) => format!("{mm}m window"),
                      (_, 0) => format!("{h}h window"),
                      _ => format!("{h}h {mm}m window"),
                  }
              }
              _ => "window".to_string(),
          }
      }
      ```
      Renderizar as janelas `other` de cada model, inserido logo depois do
      bloco `if policy != WindowPolicy::FiveHour { .. }` (linha 117-118) e
      antes de `if let Some(eu) = get_codex_extra(p)...` (linha 120):
      ```rust
      for model in models {
          if let Some(others) = model.windows.other.as_ref() {
              for w in others {
                  lines.push(vline(ColorToken::Green));
                  lines.push(label_line(
                      &other_window_label(w.window_minutes),
                      options.label_color,
                      ColorToken::Green,
                  ));
                  lines.push(model_line(
                      clock,
                      &model.name,
                      Some(w),
                      model_len,
                      mode,
                      ColorToken::Green,
                      Some("N/A"),
                  ));
              }
          }
      }
      ```

- [ ] Rodar `cargo test formatters::builders::codex` e ver passar.

- [ ] Golden: estender `codex_healthy()` em `tests/golden.rs` (linhas
      135-165) com uma janela `other` na entrada `"Codex"` de
      `models_detailed`, pra provar que o golden (waybar/terminal) também
      passa a mostrar essa janela:
      ```rust
      fn codex_healthy() -> ProviderQuota {
          let mut models_detailed = BTreeMap::new();
          models_detailed.insert(
              "Codex".to_string(),
              ModelWindows {
                  five_hour: Some(qw(60.0, FIXED_RESET, Some(300))),
                  seven_day: Some(qw(85.0, FIXED_RESET, Some(10080))),
                  other: None,
              },
          );
          let mut models = IndexMap::new();
          models.insert("Codex".to_string(), qw(60.0, FIXED_RESET, Some(300)));

          ProviderQuota {
              provider: "codex".into(),
              display_name: "Codex".into(),
              available: true,
              account: None,
              plan: Some("Pro".into()),
              plan_type: Some("pro".into()),
              primary: Some(qw(60.0, FIXED_RESET, Some(300))),
              secondary: Some(qw(85.0, FIXED_RESET, Some(10080))),
              models: Some(models),
              extra: Some(ProviderExtra::Codex(CodexQuotaExtra {
                  models_detailed: Some(models_detailed),
                  extra_usage: None,
              })),
              error: None,
              stale_reason: None,
          }
      }
      ```
      Este fixture NÃO muda neste commit — ele já não tinha janela `other`
      e continua sem. A cobertura de golden pra `other` fica coberta pelo
      teste unitário `other_window_renders_with_duration_label` acima (o
      golden runner `tests/golden.rs` roda por `insta` com snapshots
      colados do TS e não é o lugar certo pra introduzir um caso NOVO sem
      um `.snap` de referência do TS pra comparar — ver comentário de
      topo do arquivo: "os goldens NUNCA são gravados via `cargo insta
      accept`"). Não editar `tests/golden.rs` nesta task.

- [ ] `cargo test --test golden` — continua verde (nada mudou de fato no
      arquivo).

- [ ] `cargo clippy --all-targets -- -D warnings`.

- [ ] Commit:
  ```
  git add -A
  git commit -m "feat: builder codex renderiza janelas other"
  ```

---

### Task 7: TUI — fuso local, countdown e dedup no Detail

**Files:**
- Modify: `src/tui/render/detail/format.rs` (assinatura de `fmt_reset`
  linhas 67-79; novo helper de weekday)
- Modify: `src/tui/render/detail/windows.rs` (`window_line` linhas 21-39;
  `model_window_line` linhas 47-69; `window_lines` linhas 80-105)
- Modify: `src/tui/render/detail/mod.rs` (`render_full` linha 64; testes:
  helper `window` já ajustado na Task 1, novos testes de `fmt_reset` e
  dedup; snapshots existentes atualizados de propósito)
- Modify: `src/formatters/shared.rs` (visibilidade de `parse_iso`, linha
  183, de privada pra `pub(crate)`)

**Interfaces:**
- Consumes: `format_eta`/`parse_iso` de `formatters::shared` (Task 5 já
  deixou `WindowKind` acessível); `is_duplicate_window` (Task 5);
  `Clock` de `formatters::clock`; `state.last_update`/`state.local_offset`
  (já existentes em `AppState`, mesmo padrão de
  `chart::render_chart_section`).
- Produces: `fmt_reset(clock: &Clock, resets_at: Option<&str>, remaining:
  f64) -> String` — `"Xh Ym · HH:MM"` no mesmo dia local, `"{d}d {h}h · seg
  HH:MM"` (abrev. PT do weekday) quando o reset cai em outro dia local;
  `window_lines(clock: &Clock, q, provider_usage, content_width)` (novo 1º
  parâmetro) já deduplica linhas de `q.models` que colidem com
  `primary`/`secondary` via `is_duplicate_window`.

Desvio documentado do contrato: `fmt_reset` reaproveita `parse_iso` +
`format_eta` e converte fuso inline; NÃO chama `format_reset_time` porque o
formato pedido (weekday PT, sem parênteses) difere do dele — aprovado pelo
orquestrador.

- [ ] Tornar `parse_iso` acessível fora de `formatters::shared` — mudar de
      `fn parse_iso` pra `pub(crate) fn parse_iso` (linha 183 de
      `src/formatters/shared.rs`):
      ```rust
      pub(crate) fn parse_iso(iso: &str) -> Option<OffsetDateTime> {
          OffsetDateTime::parse(iso, &Rfc3339).ok()
      }
      ```

- [ ] Escrever teste que falha em `src/tui/render/detail/mod.rs`, dentro de
      `mod tests` (perto de `truncate_name_ellipsizes_long_names`, antes da
      seção "Snapshots"):
      ```rust
      #[test]
      fn fmt_reset_same_local_day_shows_countdown_and_time() {
          let clock = crate::formatters::clock::Clock {
              now: time::macros::datetime!(2026-06-19 12:00:00 UTC),
              local_offset: time::UtcOffset::from_hms(3, 0, 0).unwrap(),
          };
          // 14:05 UTC + 03:00 = 17:05 local, mesmo dia local que "now" (+03:00 = 15:00)
          let out = super::format::fmt_reset(&clock, Some("2026-06-19T14:05:00Z"), 50.0);
          assert_eq!(out, "2h 05m \u{b7} 17:05");
      }

      #[test]
      fn fmt_reset_different_local_day_shows_weekday_abbrev() {
          let clock = crate::formatters::clock::Clock {
              now: time::macros::datetime!(2026-06-19 23:30:00 UTC), // sex 23:30 UTC → sáb 02:30 local
              local_offset: time::UtcOffset::from_hms(3, 0, 0).unwrap(),
          };
          // reset seg 2026-06-22T13:00:00Z + 03:00 = seg 16:00 local (dia diferente do "now" local)
          let out = super::format::fmt_reset(&clock, Some("2026-06-22T13:00:00Z"), 30.0);
          assert_eq!(out, "2d 13h \u{b7} seg 16:00");
      }

      #[test]
      fn fmt_reset_full_and_unknown_passthrough() {
          let clock = crate::formatters::clock::Clock {
              now: time::macros::datetime!(2026-06-19 12:00:00 UTC),
              local_offset: time::UtcOffset::UTC,
          };
          assert_eq!(
              super::format::fmt_reset(&clock, Some("2026-06-19T14:00:00Z"), 100.0),
              "Full"
          );
          assert_eq!(super::format::fmt_reset(&clock, None, 50.0), "?");
      }

      #[test]
      fn window_lines_dedups_model_matching_secondary() {
          use crate::providers::types::WindowKind;
          let clock = crate::formatters::clock::Clock {
              now: time::macros::datetime!(2026-06-19 12:00:00 UTC),
              local_offset: time::UtcOffset::UTC,
          };
          let mut q = minimal_quota("codex");
          q.secondary = Some(QuotaWindow {
              remaining: 85.0,
              resets_at: Some("2026-06-26T12:00:00Z".into()),
              window_minutes: Some(10080),
              used: None,
              severity: None,
              window_kind: Some(WindowKind::SevenDay),
          });
          let mut models = IndexMap::new();
          models.insert(
              "Codex".to_string(),
              QuotaWindow {
                  remaining: 85.0,
                  resets_at: Some("2026-06-26T12:00:00Z".into()),
                  window_minutes: Some(10080),
                  used: None,
                  severity: None,
                  window_kind: Some(WindowKind::SevenDay),
              },
          );
          q.models = Some(models);
          let lines = super::windows::window_lines(&clock, &q, None, 100);
          // só "semana" (secondary) — o model "Codex" duplica e some.
          assert_eq!(lines.len(), 1, "model duplicado deveria sumir");
      }
      ```

- [ ] Rodar `cargo test detail` e ver falhar: `error[E0425]: cannot find
      function `fmt_reset` in module `super::format`` (assinatura ainda
      antiga, 1 argumento) e `error[E0061]: this function takes 3
      arguments but 4 were supplied` pra `window_lines`.

- [ ] Implementar `fmt_reset` em `src/tui/render/detail/format.rs`. Primeiro
      o import (linha 3, junto do `use` existente):
      ```rust
      use crate::formatters::clock::Clock;
      use crate::formatters::shared::{format_eta, parse_iso};
      use crate::usage::{ModelUsage, ProviderUsage};
      ```
      Depois substituir o bloco da função inteira (linhas 67-79):
      ```rust
      /// Abreviação PT (3 letras) do weekday — usada só quando o reset cai
      /// em dia local diferente de "agora" (janela de 7 dias, tipicamente).
      fn weekday_pt(w: time::Weekday) -> &'static str {
          use time::Weekday::*;
          match w {
              Monday => "seg",
              Tuesday => "ter",
              Wednesday => "qua",
              Thursday => "qui",
              Friday => "sex",
              Saturday => "s\u{e1}b",
              Sunday => "dom",
          }
      }

      /// Countdown + horário de reset em fuso LOCAL — substitui o slice cru
      /// de UTC (bug da auditoria: TUI mostrava o reset em UTC). "Xh Ym ·
      /// HH:MM" no mesmo dia local; "{d}d {h}h · seg HH:MM" (weekday
      /// abreviado) quando o reset cai em outro dia local (janela de 7
      /// dias). `Full`/`?` (via `format_eta`) passam direto — sem hora.
      pub(super) fn fmt_reset(clock: &Clock, resets_at: Option<&str>, remaining: f64) -> String {
          let eta = format_eta(clock, resets_at, remaining);
          if eta == "Full" || eta == "?" {
              return eta;
          }
          let Some(iso) = resets_at else { return eta };
          let Some(dt) = parse_iso(iso) else { return eta };
          let local = dt.to_offset(clock.local_offset);
          let today_local = clock.now.to_offset(clock.local_offset).date();
          if local.date() == today_local {
              format!("{eta} \u{b7} {:02}:{:02}", local.hour(), local.minute())
          } else {
              format!(
                  "{eta} \u{b7} {} {:02}:{:02}",
                  weekday_pt(local.weekday()),
                  local.hour(),
                  local.minute()
              )
          }
      }
      ```
      (remove a função antiga de mesmo nome, que fazia só o slice de
      `resets_at.split('T').nth(1)...`.)

- [ ] Implementar em `src/tui/render/detail/windows.rs`: threading do
      `clock` e dedup. Import (linhas 1-15):
      ```rust
      use ratatui::style::Style;
      use ratatui::text::{Line, Span};

      use crate::formatters::clock::Clock;
      use crate::formatters::shared::is_duplicate_window;
      use crate::providers::types::{ProviderQuota, QuotaWindow};
      use crate::theme::ColorToken;
      use crate::tui::theme_bridge::to_ratatui;
      use crate::tui::widgets::quota_gauge::gauge_spans;
      use crate::tui::widgets::severity::severity_color_api;
      use crate::usage::ProviderUsage;

      use super::format::{
          derive_bar_width, find_model_usage, fmt_reset, truncate_name, LABEL_W, WINDOW_SUFFIX_W,
      };
      ```
      `window_line` (linhas 21-39) e `model_window_line` (linhas 47-69)
      ganham `clock: &Clock` como 1º parâmetro:
      ```rust
      fn window_line(clock: &Clock, label: &str, w: &QuotaWindow, gauge_w: usize) -> Line<'static> {
          let color = severity_color_api(w.severity.as_deref(), Some(w.remaining));
          let reset_str = fmt_reset(clock, w.resets_at.as_deref(), w.remaining);
          let name = truncate_name(label, LABEL_W);
          let mut spans = vec![Span::styled(
              format!(" {name:<LABEL_W$} "),
              Style::default().fg(to_ratatui(ColorToken::Muted)),
          )];
          spans.extend(gauge_spans(w.remaining, gauge_w, color));
          spans.push(Span::styled(
              format!(" {:>4.0}%", w.remaining),
              Style::default().fg(to_ratatui(ColorToken::TextBright)),
          ));
          spans.push(Span::styled(
              format!("  \u{2192} {reset_str}"),
              Style::default().fg(to_ratatui(ColorToken::Comment)),
          ));
          Line::from(spans)
      }
      ```
      ```rust
      fn model_window_line(
          clock: &Clock,
          name: &str,
          w: &QuotaWindow,
          gauge_w: usize,
          content_width: u16,
          provider_usage: Option<&ProviderUsage>,
      ) -> Line<'static> {
          let mut line = window_line(clock, name, w, gauge_w);
          if let Some(cost) = provider_usage
              .and_then(|pu| find_model_usage(&pu.by_model, name))
              .and_then(|mu| mu.cost.as_ref())
          {
              let cost_span = format!("  ${:.2}", cost.usd);
              let current_w: usize = line.spans.iter().map(|s| s.content.chars().count()).sum();
              if current_w + cost_span.chars().count() <= content_width as usize {
                  line.spans.push(Span::styled(
                      cost_span,
                      Style::default().fg(to_ratatui(ColorToken::Comment)),
                  ));
              }
          }
          line
      }
      ```
      `window_lines` (linhas 80-105) ganha `clock` e passa a deduplicar
      `q.models` contra `primary`/`secondary`:
      ```rust
      pub(super) fn window_lines(
          clock: &Clock,
          q: &ProviderQuota,
          provider_usage: Option<&ProviderUsage>,
          content_width: u16,
      ) -> Vec<Line<'static>> {
          let bar_width = derive_bar_width(content_width, WINDOW_SUFFIX_W);
          let mut lines = Vec::new();
          if let Some(primary) = &q.primary {
              lines.push(window_line(clock, "sess\u{e3}o", primary, bar_width));
          }
          if let Some(secondary) = &q.secondary {
              lines.push(window_line(clock, "semana", secondary, bar_width));
          }
          if let Some(models) = &q.models {
              for (name, w) in models {
                  let dup_primary = q.primary.as_ref().is_some_and(|p| is_duplicate_window(w, p));
                  let dup_secondary =
                      q.secondary.as_ref().is_some_and(|s| is_duplicate_window(w, s));
                  if dup_primary || dup_secondary {
                      continue;
                  }
                  lines.push(model_window_line(
                      clock,
                      name,
                      w,
                      bar_width,
                      content_width,
                      provider_usage,
                  ));
              }
          }
          lines
      }
      ```

- [ ] Recalibrar `WINDOW_SUFFIX_W` em `src/tui/render/detail/format.rs`
      (linha 16) — o suffix cresceu (countdown + hora, ou countdown +
      weekday + hora); orçamento antigo (9 chars pro reset) estoura com o
      texto novo. Substituir:
      ```rust
      pub(super) const WINDOW_SUFFIX_W: usize = 1 + 4 + 1 + 2 + 1 + 20; // pct(" NNN%"=6) + reset("  → "+countdown até "99d 23h · qua 23:59"=20)
      ```

- [ ] Atualizar o único call site de `window_lines`, em
      `src/tui/render/detail/mod.rs` — `render_full` (linha 64). Substituir:
      ```rust
      let clock = crate::formatters::clock::Clock {
          now: state
              .last_update
              .unwrap_or(time::OffsetDateTime::UNIX_EPOCH),
          local_offset: state.local_offset,
      };
      let windows = window_lines(&clock, q, provider_usage, area.width);
      ```
      (mesmo padrão de `now` já usado por `render_chart_section` — nunca
      `OffsetDateTime::now_utc()` dentro de render, pra manter snapshots
      determinísticos.)

- [ ] Rodar `cargo test detail` e ver passar os 4 testes novos. Os
      snapshots existentes (`detail_claude_full`,
      `detail_chart_absorbs_extra_height_no_blank_gap`,
      `detail_collapse_short_terminal`, `detail_amp_credits`,
      `detail_codex_logged_out`, `detail_narrow_80`,
      `detail_extra_usage_disabled`, `detail_extra_usage_no_limit`,
      `detail_provider_error_shows_icon_and_message`) vão FALHAR — o texto
      do reset mudou de propósito (UTC cru → countdown + hora local).
      Confirmar, pra cada snapshot que falhar, que o diff é exatamente essa
      mudança (countdown/hora), nada mais:
      ```
      cargo insta test --review -- detail
      ```
      Revisar cada diff mostrado (aceitar só se o ÚNICO delta for o texto
      do reset — countdown + `· HH:MM`/weekday no lugar do HH:MM cru).

- [ ] `cargo test detail` de novo — todos verdes.

- [ ] `cargo clippy --all-targets -- -D warnings`.

- [ ] Commit (a mensagem documenta a mudança de propósito dos snapshots
      pro histórico do repo):
  ```
  git add -A
  git commit -m "feat: tui mostra reset em fuso local com countdown"
  ```

---

### Task 8: `config show` ganha `menuAnimations` (read-only)

**Files:**
- Modify: `src/config_cmd.rs` (struct `ConfigView` linhas 13-19;
  `view_from_settings` linhas 55-65; teste `show_json_wire_format` linhas
  252-261)

**Interfaces:**
- Consumes: `Settings.menu.animations: bool` (já existe em
  `src/settings.rs`, default `true`).
- Produces: `ConfigView.menu_animations: bool` → JSON `menuAnimations`
  (aditivo, `schemaVersion` continua 1). `config apply` não aceita esse
  campo — `ConfigPatch` não ganha o campo, então `serde_json` ignora
  silenciosamente qualquer `menuAnimations` recebido (sem
  `deny_unknown_fields` no struct).

- [ ] Escrever teste que falha, estendendo `show_json_wire_format` (linhas
      252-261):
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
          assert_eq!(j["menuAnimations"], true, "default do Settings.menu.animations");
      }

      #[test]
      fn apply_ignores_menu_animations_field() {
          let dir = tempdir().unwrap();
          let p = paths_in(dir.path());
          let v = apply_json(
              &p,
              r#"{"schemaVersion":1,"displayMode":"used","menuAnimations":false}"#,
          )
          .unwrap();
          assert_eq!(v.display_mode, DisplayMode::Used);
          // campo ignorado: não existe em ConfigPatch, apply não falha, e o
          // valor real de menu.animations em settings.json não é tocado.
          let loaded = crate::settings::load(&p);
          assert!(loaded.menu.animations, "apply não deve mexer em menu.animations");
      }
      ```

- [ ] Rodar `cargo test config_cmd` e ver falhar: `error[E0609]: no field
      `menu_animations` on type `ConfigView``.

- [ ] Implementar. `ConfigView` (linhas 13-19):
      ```rust
      #[derive(Debug, Clone, PartialEq, Serialize)]
      #[serde(rename_all = "camelCase")]
      pub struct ConfigView {
          pub schema_version: u32,
          pub providers: Vec<String>,
          pub provider_order: Vec<String>,
          pub display_mode: DisplayMode,
          pub notify: NotifyView,
          pub menu_animations: bool,
      }
      ```
      `view_from_settings` (linhas 55-65):
      ```rust
      pub fn view_from_settings(s: &Settings) -> ConfigView {
          ConfigView {
              schema_version: CONFIG_SCHEMA_VERSION,
              providers: s.waybar.providers.clone(),
              provider_order: s.waybar.provider_order.clone(),
              display_mode: s.waybar.display_mode,
              notify: NotifyView {
                  enabled: s.notify.enabled,
              },
              menu_animations: s.menu.animations,
          }
      }
      ```
      (`ConfigPatch`, linhas 40-48, não ganha campo nenhum — é assim que
      `apply` "ignora" `menuAnimations".)

- [ ] Rodar `cargo test config_cmd` e ver passar.

- [ ] `cargo clippy --all-targets -- -D warnings`.

- [ ] Commit:
  ```
  git add -A
  git commit -m "feat: config show expoe menuAnimations read-only"
  ```

---

### Task 9: `docs/json-output.md` — `windowKind` + shapes de `extra`

**Files:**
- Modify: `docs/json-output.md` (envelope de exemplo linhas 24-42; tabela
  de campos linhas 46-59; definição de `Window` linhas 61-69; nova seção
  depois de "Stability", antes de "Quickshell example")

**Interfaces:**
- Nenhuma (documentação pura — verificação é `git diff --check`).

- [ ] Atualizar a definição de `Window` (linhas 61-69) pra incluir
      `windowKind`:
      ```markdown
      `Window`: `{ remaining: number, used?: number|null, resetsAt: string|null, windowMinutes?: number|null, severity?: string, windowKind?: string }`.
      `remaining`/`used` are percentages (0-100). `used` is only present when a provider
      reports a distinct "used" metric that is not simply `100 - remaining` (it can exceed 100 with overage).
      `severity` is optional (`Option<String>`, omitted when absent) and comes from the
      provider's own API — today only Claude populates it, from `limits[].severity`.
      Known values: `normal`/`ok`/`warning`/`elevated`/`high`/`critical`/`exceeded`/`blocked`.
      Consumers should fall back to a local threshold on `remaining` (≥60/30/10) when
      `severity` is absent or unrecognized — this mirrors `severity_color_api` in
      `src/tui/widgets/severity.rs`.
      `windowKind` is one of `fiveHour`/`sevenDay`/`daily`/`context`/`other`, decided once
      by the provider at fetch time (never a client-side magic-number guess). It replaces
      any window-duration heuristic a consumer might have written against `windowMinutes`
      — `fiveHour`/`sevenDay` map to Claude's and Codex's own quota tiers, `daily` is Amp's
      free-tier reset cadence, `context` is Grok's context-window usage (no reset — see
      `contextTokensUsed`/`contextWindowTokens` in `extra` instead), `other` is any window
      that doesn't fit those tiers (e.g. a Codex bucket with a non-standard duration).
      Omitted when the provider hasn't classified the window (should not happen in
      production; only seen in hand-built test fixtures).
      ```

- [ ] Adicionar `windowKind` ao exemplo do envelope (linha 34):
      ```markdown
      "primary":   { "remaining": 30, "used": 70, "resetsAt": "2026-06-17T20:09:59Z", "windowMinutes": 300, "windowKind": "fiveHour" },
      ```

- [ ] Adicionar uma nova seção depois de "## Stability" (antes de "##
      Quickshell example"), documentando os shapes de `extra` por provider
      (hoje só descritos implicitamente pelo `ProviderExtra` untagged em
      Rust):
      ```markdown
      ## `extra` shapes by provider

      `extra` is untagged (no variant key in the JSON — see Stability above:
      unstable, no `schemaVersion` bump on change). Shape depends on `provider`:

      - **`claude`** (`ClaudeQuotaExtra`): `{ weeklyModels?: Record<string, Window>,
        extraUsage?: { enabled: boolean, remaining: number, limit: number, used: number } }`.
        `weeklyModels` keys are model display names (e.g. `"Opus"`, `"Sonnet"`,
        `"Cowork"`, or a `weekly_scoped` display name from `limits[]`). `extraUsage`
        mirrors Claude's `spend`/legacy `extra_usage` block — `limit === -1` means
        unlimited (Codex-style sentinel, see below); a real `$0` limit with `enabled:
        true` is not expected from Claude today.
      - **`codex`** (`CodexQuotaExtra`): `{ modelsDetailed?: Record<string, ModelWindows>,
        extraUsage?: { enabled, remaining, limit, used } }`. `ModelWindows` is `{
        fiveHour?: Window, sevenDay?: Window, other?: Window[] }` — `other` holds any
        window that didn't classify as `fiveHour`/`sevenDay` (non-standard bucket
        duration; see `windowKind: "other"` above). `extraUsage.limit === -1` means
        unlimited credits; `0` means a real (informational) balance with no configured
        cap.
      - **`amp`** (`AmpQuotaExtra`): `{ meta?: Record<string, string> }` — free-form
        key/value pairs scraped from `amp usage` output (e.g. `freeRemaining`,
        `freeTotal`, `replenishRate`, `bonus`, `creditsBalance`, `creditsReplenish`, or
        `raw0`..`raw3` when the CLI's text format wasn't recognized). No fixed key set —
        treat as display-only strings, never parse them back into numbers.
      - **`grok`** (`GrokQuotaExtra`): `{ sessionsToday?: number, turnsToday?: number,
        contextTokensUsed?: number, contextWindowTokens?: number, recentModel?: string }`.
        Grok's `primary` window (`windowKind: "context"`) has no `resetsAt` — context
        usage doesn't reset on a timer, it resets when the session/thread does.
      ```

- [ ] Verificar: `git diff --check` (whitespace) na matriz de docs do
      CLAUDE.md.

- [ ] `git diff --stat docs/json-output.md` — conferir visualmente que só
      as seções pretendidas mudaram.

- [ ] Commit:
  ```
  git add -A
  git commit -m "docs: windowKind e shapes de extra por provider"
  ```

---

## Nota de fechamento (não é task)

Ao final da Task 9, invocar `superpowers:finishing-a-development-branch`
pra decidir merge/PR — não esperar pedido explícito (regra global do
dono). Antes disso, rodar a verificação ampla da matriz (mudança tocou
múltiplos contratos): `cargo test && cargo clippy --all-targets --
-D warnings`.
