# Fase 2 — UI Transversal + Redesign — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Implementar as decisões visuais aprovadas (popup "Camadas", rótulos "{duração} Reset", tempo "2h 30m · 14:59", chip com cue "⌛") + enum sync JS↔Rust, split do ServiceCore.js, `error.retryable` na UI e limpeza rust-core.

**Architecture:** Rótulos nascem no helper Rust (`v2_map.rs` — QML nunca re-deriva). Humanização de tempo é apresentação → vive no JS core (funções puras testáveis). O redesign toca só a camada de componentes (`UsageWindow`, `ProviderView`, `ProviderHeader`); `Service.qml` e o protocolo Quattro ficam intactos. O split do ServiceCore vem POR ÚLTIMO (move-refactor puro sobre conteúdo estável), seguindo a convenção Omarchy de cada QML importar cada lib (sem `.import` JS→JS até o probe provar que funciona).

**Tech Stack:** Rust (serde/time), QML/Qt6, qmltestrunner do Qt6.

## Global Constraints

- Rust/Cargo e QML apenas. Proibido `unwrap()`/`expect()` em produção (clippy deny ativo).
- QML nunca parseia saída crua de provider; strings externas renderizam como plain text.
- Sem motion autoral do plugin; sem meaning só-por-cor (glifo/texto sempre junto).
- Copy do produto em INGLÊS ("resets", "left", "Updated", "Stale").
- IDs de janela NÃO mudam (session/weekly/daily/weekly-model:*); só labels.
- Commits: Conventional Commit, subject em INGLÊS, ≤50 chars.
- Gate Rust: `cargo fmt --check && cargo test && cargo clippy --all-targets -- -D warnings && git diff --check`.
- Gate QML (invocação CORRETA — o binário `qmltestrunner` do PATH é Qt5 e falha em silêncio):
  ```bash
  find assets/omarchy -type f -name '*.qml' -exec qmllint -I /usr/share/omarchy/shell {} +
  omarchy plugin validate assets/omarchy
  QML_XHR_ALLOW_FILE_READ=1 QT_LOGGING_TO_CONSOLE=1 QT_QPA_PLATFORM=offscreen \
    /usr/lib/qt6/bin/qmltestrunner -input tests/qml \
    -import /usr/share/omarchy/shell -import assets/omarchy
  ```
  Baseline atual: 161 passed. Qualquer task que toque QML roda esse gate.
- Falha pré-existente conhecida do `cargo test`: `binary_interactive_update_rejects_non_tty` (corrigida na Task 8; até lá, ignorar só ela).
- Branch: `claude-ajustes`. Read cada arquivo antes de Edit; Edit "string not found" → re-Read.
- `cargo test` aceita UM filtro por invocação (dois nomes = duas invocações).

## File Structure

- `src/providers/v2_map.rs` — labels das janelas (Task 1).
- `tests/fixtures/status-v2/*.json` + `tests/qml/*.qml` — pins de label/ISO (Tasks 1, 4, 5, 6).
- `tests/servicecore_contract.rs` — NOVO: enum sync JS↔Rust (Task 2).
- `assets/omarchy/ServiceCore.js` — format helpers (Task 3), windowGroups/tooltip/cue (Task 4), depois fatiado (Task 7).
- `assets/omarchy/components/UsageWindow.qml` — redesign "Camadas" (Task 5).
- `assets/omarchy/ProviderView.qml` + `components/ProviderHeader.qml` — grupos, stale banner, footer (Task 6).
- `assets/omarchy/Core*.js` (NOVOS, Task 7): `CoreService.js`, `CoreSettings.js`, `CoreView.js`, `CoreScroll.js`, `CoreMaintenance.js`; `tests/qml/TestPalette.js` (test-only sai do bundle).
- `src/status/coordinator.rs`, `src/cache/{coordinator,store}.rs`, `src/cli/mod.rs`, `src/notifications/mod.rs`, `Cargo.toml`, `tests/active_legacy_scan.rs`, `tests/cli.rs` — limpeza (Task 8).
- `CLAUDE.md` — comando QML corrigido (Task 9).

---

### Task 1: Rótulos "{duração} Reset" no helper Rust

Decisão aprovada: "Session"→"5h Reset", "Weekly"→"7d Reset", Amp "Daily"→"1d Reset", Codex `other:{n}` → "{n}m Reset". IDs intactos.

**Files:**
- Modify: `src/providers/v2_map.rs` (produção: linhas ~64, ~166, ~348-355, ~492, ~498; testes: ~828, ~845, ~848, ~863)
- Modify: `src/providers/adapters.rs:982` (teste Grok)
- Modify: `tests/fixtures/status-v2/{money-field,percent-over-100,ready,valid-multi-provider,valid-stale}.json` (campo `"label"`)
- Modify: `docs/specs/v10/03-cli-and-json-contract.md:119` (exemplo ilustrativo)

**Interfaces:**
- Produces: labels novos no JSON de status; consumidos como dado pelo QML (nenhuma mudança QML aqui).

- [ ] **Step 1: Atualizar os asserts de teste para os labels novos (red)**

Em `src/providers/v2_map.rs` (módulo tests) e `src/providers/adapters.rs:982`, trocar TODOS os asserts de label:
- `assert_eq!(windows[0].label(), "Weekly")` → `"7d Reset"` (3 sites em v2_map: ~828, ~848, ~863; 1 em adapters ~982)
- `assert_eq!(windows[0].label(), "Session")` → `"5h Reset"` (v2_map ~845)

- [ ] **Step 2: Rodar e confirmar red**

Run: `cargo test -p agent-bar 2>/dev/null || cargo test` (filtrar: `cargo test codex_` e `cargo test grok_billing_weekly`)
Expected: FAIL — produção ainda emite "Session"/"Weekly".

- [ ] **Step 3: Trocar os labels de produção**

Em `src/providers/v2_map.rs`:

Amp (~linha 64): `UsageWindow::try_new("daily", "Daily", ...)` → `UsageWindow::try_new("daily", "1d Reset", ...)`

Grok (~linha 166): `UsageWindow::try_new("weekly", "Weekly", ...)` → `UsageWindow::try_new("weekly", "7d Reset", ...)`

`codex_window_identity` (~348-355) vira:

```rust
fn codex_window_identity(window_minutes: Option<i64>, ordinal: usize) -> (String, String) {
    match window_minutes {
        Some(10080) => ("weekly".into(), "7d Reset".into()),
        Some(300) => ("session".into(), "5h Reset".into()),
        Some(n) if n > 0 => (format!("other:{n}:{ordinal}"), format!("{n}m Reset")),
        _ => {
            if ordinal == 1 {
                ("session".into(), "5h Reset".into())
            } else {
                ("weekly".into(), "7d Reset".into())
            }
        }
    }
}
```

Claude (~492 e ~498): `claude_window("session", "Session", w)` → `claude_window("session", "5h Reset", w)`; `claude_window("weekly", "Weekly", w)` → `claude_window("weekly", "7d Reset", w)`.

Janelas por modelo (decisão do design): o label vira o NOME do modelo puro ("Opus"/"Sonnet"/display_name) — a lista quieta do popup já contextualiza que são semanais. Substituir o loop legado (~527-537) por:

```rust
    for (suffix, label, field) in [
        ("opus", "Opus", doc.seven_day_opus.as_ref()),
        ("sonnet", "Sonnet", doc.seven_day_sonnet.as_ref()),
    ] {
        if let Some(w) = field {
            let id = weekly_model_id(suffix, 0);
            if let Some(window) = claude_window(&id, label, w) {
                push_window_unique(&mut windows, window);
            }
        }
    }
```

E no caminho `limits[]` (~513-518), o label dinâmico continua `display_name` do modelo (já é "Opus"/"Sonnet" puro) — trocar só o fallback `"Weekly model"` → `"Model"`.

- [ ] **Step 4: Atualizar fixtures JSON e o exemplo do spec**

Nos 5 fixtures listados: `"label": "Session"` → `"label": "5h Reset"`; `"label": "Weekly"` → `"label": "7d Reset"`. Em `docs/specs/v10/03-cli-and-json-contract.md:119`, mesmo ajuste no exemplo. (JSON-020 exige só "English labels" — segue satisfeito.)

- [ ] **Step 5: Rodar suíte inteira e ajustar residuais**

Run: `cargo test`
Expected: verde (exceto o pré-existente da Task 8). Se algum teste de schema/notifications quebrar por label, atualizar o assert (são dados de fixture, não contrato).

- [ ] **Step 6: Commit**

```bash
git add -A src tests/fixtures docs/specs/v10/03-cli-and-json-contract.md
git commit -m 'feat: duration-based window labels'
```

---

### Task 2: Enum sync JS↔Rust (teste de contrato)

`ServiceCore.js` copia à mão `PROVIDER_STATES`/`ACTION_KINDS`/`CLOSED_PROVIDERS`; um state novo no Rust congela o popup em silêncio. Um teste Rust passa a comparar os sets extraídos do JS com os enums.

**Files:**
- Create: `tests/servicecore_contract.rs`

**Interfaces:**
- Consumes: `assets/omarchy/ServiceCore.js` (consts nas linhas ~4-25); `ProviderState`/`ActionKind` (`src/status/schema.rs`), `ProviderId` (`src/cli`).
- Produces: teste `servicecore_enums_match_schema`. A Task 7 atualiza o path do arquivo lido quando os consts migrarem para `CoreService.js`.

- [ ] **Step 1: Escrever o teste**

Criar `tests/servicecore_contract.rs`:

```rust
//! ServiceCore.js hand-copies closed enums from the Rust schema. This test
//! keeps them in lock-step: a new state/action/provider added on one side
//! fails here instead of silently freezing the popup at runtime.

use std::collections::BTreeSet;

fn extract_keys(source: &str, var_name: &str) -> BTreeSet<String> {
    let start = source
        .find(&format!("var {var_name} = {{"))
        .unwrap_or_else(|| panic!("{var_name} not found in ServiceCore.js"));
    let rest = &source[start..];
    let end = rest.find('}').expect("unterminated const object");
    let body = &rest[..end];
    let mut keys = BTreeSet::new();
    for cap in body.split('"').skip(1).step_by(2) {
        // split('"') alternates outside/inside quotes; skip(1).step_by(2)
        // yields the quoted keys only ("ready", "stale", ...).
        if !cap.is_empty() && cap.chars().all(|c| c.is_ascii_lowercase() || c == '_') {
            keys.insert(cap.to_owned());
        }
    }
    keys
}

#[test]
fn servicecore_enums_match_schema() {
    let js = std::fs::read_to_string("assets/omarchy/ServiceCore.js")
        .expect("read ServiceCore.js");

    let states = extract_keys(&js, "PROVIDER_STATES");
    let expected_states: BTreeSet<String> = [
        "ready", "stale", "cli_missing", "unauthenticated",
        "rate_limited", "network_error", "provider_error",
    ]
    .into_iter()
    .map(str::to_owned)
    .collect();
    assert_eq!(states, expected_states, "PROVIDER_STATES drifted from ProviderState");

    let kinds = extract_keys(&js, "ACTION_KINDS");
    let expected_kinds: BTreeSet<String> =
        ["retry", "login", "view_installation"].into_iter().map(str::to_owned).collect();
    assert_eq!(kinds, expected_kinds, "ACTION_KINDS drifted from ActionKind");

    let providers = extract_keys(&js, "CLOSED_PROVIDERS");
    let expected_providers: BTreeSet<String> =
        ["claude", "codex", "amp", "grok"].into_iter().map(str::to_owned).collect();
    assert_eq!(providers, expected_providers, "CLOSED_PROVIDERS drifted from ProviderId");
}
```

Nota: os sets esperados são literais DE PROPÓSITO (espelham `ProviderState::as_str`/`ActionKind::as_str`/`ProviderId` — schema.rs:100-170). Se um enum Rust ganhar variante, o autor atualiza o literal AQUI e o JS — o teste é o lembrete bidirecional. `unwrap/expect` são permitidos em teste.

- [ ] **Step 2: Rodar e confirmar verde (contrato hoje está em sincronia)**

Run: `cargo test --test servicecore_contract`
Expected: PASS (1 teste). Sabotagem rápida para validar o detector: rode uma vez com um typo no expected e confirme FAIL, depois desfaça.

- [ ] **Step 3: Commit**

```bash
git add tests/servicecore_contract.rs
git commit -m 'test: lock ServiceCore enums to Rust schema'
```

---

### Task 3: Helpers de tempo humanizado no ServiceCore

Formato aprovado: countdown compacto + hora absoluta local — "2h 30m · 14:59" (<24h) / "2d 18h · Fri 09:00" (≥24h). E "Updated": "just now" / "5m ago" / "3h ago" / "2d ago".

**Files:**
- Modify: `assets/omarchy/ServiceCore.js` (novas funções na seção de apresentação, antes de `windowDisplayLines` ~1201)
- Create: `tests/qml/tst_Format.qml`

**Interfaces:**
- Produces: `formatResetText(iso, nowMs) -> string` ("" quando inválido/vazio; "now" quando passado), `formatAgoText(iso, nowMs) -> string` ("" quando inválido). Consumidas nas Tasks 4-6.

- [ ] **Step 1: Escrever os testes (red)**

Criar `tests/qml/tst_Format.qml`:

```qml
import QtQuick
import QtTest
import "../../assets/omarchy/ServiceCore.js" as Core

TestCase {
  name: "AgentBarFormat"

  // 2026-07-28T15:00:00Z as fixed "now".
  readonly property double nowMs: Date.parse("2026-07-28T15:00:00Z")

  function test_reset_under_1h() {
    // 37 minutes ahead.
    var text = Core.formatResetText("2026-07-28T15:37:00Z", nowMs)
    verify(text.indexOf("37m") === 0, "countdown: " + text)
    verify(text.indexOf("\u00b7") > 0, "has absolute separator: " + text)
  }

  function test_reset_under_24h_has_hours_minutes() {
    var text = Core.formatResetText("2026-07-28T17:30:00Z", nowMs)
    verify(text.indexOf("2h 30m") === 0, text)
  }

  function test_reset_over_24h_uses_days_and_weekday() {
    var text = Core.formatResetText("2026-07-31T09:00:00Z", nowMs)
    verify(text.indexOf("2d 18h") === 0, text)
    // Absolute part carries a weekday token (locale en).
    verify(/[A-Z][a-z]{2} \d\d:\d\d$/.test(text), "weekday absolute: " + text)
  }

  function test_reset_past_is_now() {
    compare(Core.formatResetText("2026-07-28T14:00:00Z", nowMs), "now")
  }

  function test_reset_invalid_is_empty() {
    compare(Core.formatResetText("", nowMs), "")
    compare(Core.formatResetText("garbage", nowMs), "")
    compare(Core.formatResetText(null, nowMs), "")
  }

  function test_ago_variants() {
    compare(Core.formatAgoText("2026-07-28T14:59:30Z", nowMs), "just now")
    compare(Core.formatAgoText("2026-07-28T14:55:00Z", nowMs), "5m ago")
    compare(Core.formatAgoText("2026-07-28T12:00:00Z", nowMs), "3h ago")
    compare(Core.formatAgoText("2026-07-26T12:00:00Z", nowMs), "2d ago")
    compare(Core.formatAgoText("nope", nowMs), "")
  }
}
```

- [ ] **Step 2: Rodar e confirmar red**

Run (gate QML do Global Constraints, ou direto):
```bash
QML_XHR_ALLOW_FILE_READ=1 QT_LOGGING_TO_CONSOLE=1 QT_QPA_PLATFORM=offscreen \
  /usr/lib/qt6/bin/qmltestrunner -input tests/qml/tst_Format.qml \
  -import /usr/share/omarchy/shell -import assets/omarchy
```
Expected: FAIL — `formatResetText` não é função.

- [ ] **Step 3: Implementar em ServiceCore.js**

Inserir antes de `windowDisplayLines` (seção de apresentação):

```javascript
// ---------------------------------------------------------------------------
// Humanized time (UX Fase 2: countdown + absolute local time)
// ---------------------------------------------------------------------------

function parseIsoMs(iso) {
  if (iso === null || iso === undefined)
    return NaN
  var s = String(iso)
  if (!s.length)
    return NaN
  var ms = Date.parse(s)
  return isFinite(ms) ? ms : NaN
}

function countdownText(diffMs) {
  var totalMinutes = Math.floor(diffMs / 60000)
  var days = Math.floor(totalMinutes / 1440)
  var hours = Math.floor((totalMinutes % 1440) / 60)
  var minutes = totalMinutes % 60
  if (days > 0)
    return days + "d " + hours + "h"
  if (hours > 0)
    return hours + "h " + minutes + "m"
  return minutes + "m"
}

// "2h 30m · 14:59" (<24h) | "2d 18h · Fri 09:00" (>=24h) | "now" | "".
function formatResetText(iso, nowMs) {
  var ms = parseIsoMs(iso)
  if (!isFinite(ms))
    return ""
  var diff = ms - nowMs
  if (diff <= 0)
    return "now"
  var date = new Date(ms)
  var absolute = diff >= 86400000
      ? Qt.formatDateTime(date, "ddd hh:mm")
      : Qt.formatDateTime(date, "hh:mm")
  return countdownText(diff) + " \u00b7 " + absolute
}

// "just now" | "5m ago" | "3h ago" | "2d ago" | "".
function formatAgoText(iso, nowMs) {
  var ms = parseIsoMs(iso)
  if (!isFinite(ms))
    return ""
  var diff = Math.max(0, nowMs - ms)
  if (diff < 60000)
    return "just now"
  var minutes = Math.floor(diff / 60000)
  if (minutes < 60)
    return minutes + "m ago"
  var hours = Math.floor(minutes / 60)
  if (hours < 24)
    return hours + "h ago"
  return Math.floor(hours / 24) + "d ago"
}
```

Nota: `Qt.formatDateTime` usa o locale do sistema; o weekday em en é garantido pelo locale C/en dos testes offscreen. Se `test_reset_over_24h_uses_weekday` falhar por locale pt, trocar a implementação do absoluto ≥24h para `WEEKDAYS[date.getDay()] + " " + Qt.formatDateTime(date, "hh:mm")` com `var WEEKDAYS = ["Sun","Mon","Tue","Wed","Thu","Fri","Sat"]` — copy do produto é inglês, não dependa de locale.

- [ ] **Step 4: Rodar e confirmar verde**

Mesmo comando do Step 2. Expected: PASS (7 testes). Rodar também a suíte inteira QML (gate) — nada mais pode quebrar (funções novas, sem call sites).

- [ ] **Step 5: Commit**

```bash
git add assets/omarchy/ServiceCore.js tests/qml/tst_Format.qml
git commit -m 'feat: humanized reset and age formatting'
```

---

### Task 4: windowGroups + tooltip humanizado + cue "⌛"

`windowDisplayLines` passa a emitir `resetText` humanizado e a separar janelas principais (ids `session`/`weekly`/`daily`) das secundárias (modelos etc.). Tooltip do chip usa o novo formato. Cue de stale vira "⌛" (decisão do chip C).

**Files:**
- Modify: `assets/omarchy/ServiceCore.js` (`windowDisplayLines` ~1201, novo `windowGroups`, `chipTooltip` ~950, `chipStateCue` ~928)
- Modify: `tests/qml/tst_BarWidget.qml` (~257-263 pin do ISO; asserts de cue se existirem)
- Modify: `tests/qml/tst_ProviderStates.qml` e `tests/qml/tst_Accessibility.qml` (asserts de `chipStateCue`/`windowDisplayLines` — rodar, ver o que quebra, atualizar para os novos valores)

**Interfaces:**
- Consumes: `formatResetText`/`formatAgoText` (Task 3).
- Produces: `windowDisplayLines(provider, metric, nowMs)` — linhas ganham `resetText` (humanizado; `resetsAt` cru continua no objeto para a11y); `windowGroups(provider, metric, nowMs) -> { primary: [...], secondary: [...] }`; `chipStateCue` stale → `" \u231b"`; `chipTooltip(provider, metric, nowMs)`.

- [ ] **Step 1: Atualizar os testes (red)**

Em `tests/qml/tst_BarWidget.qml` `test_tooltip_includes_provider_percent_state_reset` (~257-263): a janela fixture tem `resetsAt: "2026-07-26T22:00:00Z"`. Trocar o assert do ISO cru por: chamar `Core.chipTooltip(provider, "remaining", Date.parse("2026-07-26T20:00:00Z"))` e verificar `tip.indexOf("resets") >= 0` e `tip.indexOf("2h 0m") >= 0` e `tip.indexOf("2026-07-26T22:00:00Z") === -1` (ISO cru NUNCA mais aparece no tooltip).

Adicionar em `tst_ProviderStates.qml`:

```qml
  function test_window_groups_split_primary_and_models() {
    var provider = {
      id: "claude", name: "Claude", state: "ready",
      windows: [
        { id: "session", label: "5h Reset", usedPercent: 31, remainingPercent: 69,
          resetsAt: "2026-07-28T17:59:59Z" },
        { id: "weekly", label: "7d Reset", usedPercent: 6, remainingPercent: 94,
          resetsAt: "2026-07-31T11:59:59Z" },
        { id: "weekly-model:opus", label: "Opus", usedPercent: 2, remainingPercent: 98,
          resetsAt: "2026-07-31T11:59:59Z" }
      ]
    }
    var now = Date.parse("2026-07-28T15:00:00Z")
    var groups = Core.windowGroups(provider, "remaining", now)
    compare(groups.primary.length, 2)
    compare(groups.secondary.length, 1)
    compare(groups.primary[0].label, "5h Reset")
    verify(groups.primary[0].resetText.indexOf("2h 59m") === 0, groups.primary[0].resetText)
    compare(groups.secondary[0].label, "Opus")
  }

  function test_chip_state_cue_stale_is_hourglass() {
    compare(Core.chipStateCue({ state: "stale" }), " \u231b")
  }
```

- [ ] **Step 2: Rodar e confirmar red**

Gate QML. Expected: FAIL nos testes novos/alterados.

- [ ] **Step 3: Implementar**

Em `ServiceCore.js`:

1. `windowDisplayLines(provider, metric, nowMs)` — assinatura ganha `nowMs` (default: `Date.now()` quando `undefined`); cada linha ganha:
```javascript
      resetText: w.resetsAt ? formatResetText(String(w.resetsAt), nowMs === undefined ? Date.now() : nowMs) : ""
```
(mantendo `resetsAt` cru no objeto).

2. Novo `windowGroups` logo após:
```javascript
var PRIMARY_WINDOW_IDS = { "session": true, "weekly": true, "daily": true }

function windowGroups(provider, metric, nowMs) {
  var lines = windowDisplayLines(provider, metric, nowMs)
  var groups = { primary: [], secondary: [] }
  for (var i = 0; i < lines.length; i++) {
    if (PRIMARY_WINDOW_IDS[lines[i].id])
      groups.primary.push(lines[i])
    else
      groups.secondary.push(lines[i])
  }
  return groups
}
```

3. `chipStateCue`: `" stale"` → `" \u231b"` (linhas ~932-933). Demais cues inalterados.

4. `chipTooltip(provider, metric, nowMs)`: no trecho do reset (~958-961), trocar `String(w.resetsAt)` por `formatResetText(String(w.resetsAt), nowMs === undefined ? Date.now() : nowMs)`, e só anexar se o resultado for não-vazio.

5. Call sites QML de `chipTooltip` (BarWidget.qml:95) e `windowDisplayLines` (ProviderView.qml:21) NÃO precisam passar `nowMs` nesta task (default `Date.now()`); a Task 6 liga o tick.

- [ ] **Step 4: Rodar gate QML inteiro e ajustar residuais**

Expected: PASS. Se `tst_Accessibility`/`tst_Screenshots` assertarem cue " stale" antigo, atualizar para " \u231b".

- [ ] **Step 5: Commit**

```bash
git add assets/omarchy/ServiceCore.js tests/qml
git commit -m 'feat: humanized tooltip and window groups'
```

---

### Task 5: Redesign do UsageWindow ("Camadas")

Linha de janela vira: kicker uppercase (label) → numeral grande + unidade → trilha com acento → linha de reset. Suporta `dimmed` (stale).

**Files:**
- Modify: `assets/omarchy/components/UsageWindow.qml` (substituição completa do layout)
- Modify: `tests/qml` que instanciam UsageWindow (rodar gate; `tst_BarWidget.qml:96/235` são fixtures de dados — labels lá podem ficar, são dados)

**Interfaces:**
- Consumes: linhas de `windowGroups` (label, percentText, percent, resetText).
- Produces: props novas: `resetText` (substitui exibição de `resetsAt` cru; prop `resetsAt` REMOVIDA), `unitText` ("left"/"used"), `emphasis` (bool: primária grande vs secundária compacta), `dimmed` (bool), `accent` (color).

- [ ] **Step 1: Verificar o token de acento do tema**

Read `/usr/share/omarchy/shell/Commons/Color.qml` e identificar o token de acento semântico (candidatos: `Color.accent`, `Color.primary`, `Color.blue`). Anotar o nome real; se não houver acento semântico, usar `root.foreground` como fallback e registrar no report.

- [ ] **Step 2: Substituir o layout**

Novo `assets/omarchy/components/UsageWindow.qml` (substituir o arquivo inteiro; preservar o header de comentário adaptado):

```qml
import QtQuick
import qs.Commons

// One normalized percentage window, "Camadas" hierarchy (Fase 2):
// kicker label -> big numeral + unit -> accent track -> humanized reset line.
Item {
  id: root

  property string label: ""
  property string percentText: "\u2014"
  // 0–100 when known; negative when unavailable (hide fill).
  property real percent: -1
  property string resetText: ""
  property string unitText: "left"
  // Primary windows render large; secondary (per-model) render compact.
  property bool emphasis: true
  property bool dimmed: false
  property color foreground: Color.foreground
  property color accent: Color.foreground
  property string fontFamily: Style.font.family

  readonly property bool hasPercent: root.percent >= 0 && root.percent <= 100
  readonly property real fillRatio: hasPercent
      ? Math.max(0, Math.min(1, root.percent / 100))
      : 0

  width: parent ? parent.width : implicitWidth
  implicitHeight: root.emphasis ? bigCol.implicitHeight : compactRow.implicitHeight
  height: implicitHeight
  opacity: root.dimmed ? 0.6 : 1.0

  Column {
    id: bigCol
    visible: root.emphasis
    width: parent.width
    spacing: Style.space(4)

    Text {
      width: parent.width
      text: root.label
      color: Qt.darker(root.foreground, 1.35)
      font.family: root.fontFamily
      font.pixelSize: Style.font.caption
      font.capitalization: Font.AllUppercase
      font.letterSpacing: 1
      elide: Text.ElideRight
      textFormat: Text.PlainText
      Accessible.ignored: true
    }

    Row {
      spacing: Style.space(6)
      Text {
        text: root.percentText
        color: root.foreground
        font.family: root.fontFamily
        font.pixelSize: Math.round(Style.font.body * 1.8)
        font.bold: true
        textFormat: Text.PlainText
        Accessible.ignored: true
      }
      Text {
        anchors.baseline: parent.children[0].baseline
        text: root.unitText
        color: Qt.darker(root.foreground, 1.35)
        font.family: root.fontFamily
        font.pixelSize: Style.font.caption
        textFormat: Text.PlainText
        Accessible.ignored: true
      }
    }

    Rectangle {
      width: parent.width
      height: Style.space(5)
      radius: height / 2
      color: Qt.rgba(root.foreground.r, root.foreground.g, root.foreground.b, 0.12)
      Accessible.ignored: true

      Rectangle {
        anchors.left: parent.left
        anchors.verticalCenter: parent.verticalCenter
        width: Math.max(root.hasPercent && root.fillRatio > 0 ? Style.space(5) : 0,
                        parent.width * root.fillRatio)
        height: parent.height
        radius: parent.radius
        color: root.dimmed ? root.foreground : root.accent
        opacity: root.dimmed ? 0.45 : 0.9
        visible: root.hasPercent && root.fillRatio > 0
      }
    }

    Row {
      visible: root.resetText.length > 0
      spacing: Style.space(4)
      Text {
        text: "resets"
        color: Qt.darker(root.foreground, 1.35)
        font.family: root.fontFamily
        font.pixelSize: Style.font.caption
        textFormat: Text.PlainText
        Accessible.ignored: true
      }
      Text {
        text: root.resetText
        color: root.foreground
        font.family: root.fontFamily
        font.pixelSize: Style.font.caption
        font.bold: true
        textFormat: Text.PlainText
        Accessible.ignored: true
      }
    }
  }

  Row {
    id: compactRow
    visible: !root.emphasis
    width: parent.width
    spacing: Style.space(8)

    Text {
      id: compactLabel
      width: Math.max(0, parent.width * 0.5)
      text: root.label
      color: Qt.darker(root.foreground, 1.2)
      font.family: root.fontFamily
      font.pixelSize: Style.font.caption
      elide: Text.ElideRight
      textFormat: Text.PlainText
      Accessible.ignored: true
    }
    Text {
      text: root.percentText + " " + root.unitText
      color: root.foreground
      font.family: root.fontFamily
      font.pixelSize: Style.font.caption
      font.bold: true
      textFormat: Text.PlainText
      Accessible.ignored: true
    }
  }

  Accessible.name: {
    var parts = [root.label, root.percentText + " " + root.unitText]
    if (root.resetText.length)
      parts.push("resets " + root.resetText)
    return parts.join(", ")
  }
  Accessible.role: Accessible.StaticText
}
```

- [ ] **Step 3: Rodar gate QML — ProviderView ainda passa `resetsAt` (prop removida)**

Expected: erros/warnings de prop inexistente ou testes vermelhos — anotar; a Task 6 religa o ProviderView. Se a suíte quebrar DURO aqui, aplicar o mínimo na ProviderView nesta task (trocar `resetsAt: ...` por `resetText: modelData.resetText ? modelData.resetText : ""`), deixando o restante do redesign para a Task 6.

- [ ] **Step 4: Commit**

```bash
git add assets/omarchy/components/UsageWindow.qml assets/omarchy/ProviderView.qml
git commit -m 'feat: layered usage window component'
```

---

### Task 6: ProviderView em grupos + stale banner + footer + retryable

**Files:**
- Modify: `assets/omarchy/ProviderView.qml` (reestrutura completa do corpo)
- Modify: `assets/omarchy/components/ProviderHeader.qml` (remover bloco "Updated" ~104-115; plano vira pill)
- Modify: `assets/omarchy/ServiceCore.js` (`stateActions` passa a considerar `error.retryable` para a ação Retry)
- Modify: `tests/qml/tst_ProviderStates.qml`, `tst_Popup.qml`, `tst_Screenshots.qml` (asserts afetados)

**Interfaces:**
- Consumes: `windowGroups` (Task 4), `formatAgoText` (Task 3), props novas do UsageWindow (Task 5).
- Produces: layout final do popup; `stateActions(provider)` só inclui `retry` quando `provider.error === null || provider.error === undefined || provider.error.retryable === true` (estado ready/stale sempre pode refresh manual pelo header).

- [ ] **Step 1: Testes (red)**

Em `tst_ProviderStates.qml`:

```qml
  function test_state_actions_respect_retryable() {
    var nonRetryable = {
      id: "claude", name: "Claude", state: "provider_error",
      error: { code: "provider_error", message: "x", retryable: false },
      action: { kind: "retry", label: "Retry", target: null }
    }
    var acts = Core.stateActions(nonRetryable)
    var hasRetry = false
    for (var i = 0; i < acts.length; i++)
      if (acts[i].kind === "retry") hasRetry = true
    verify(!hasRetry, "non-retryable error must not offer Retry")

    var retryable = {
      id: "claude", name: "Claude", state: "network_error",
      error: { code: "network_error", message: "x", retryable: true },
      action: { kind: "retry", label: "Retry", target: null }
    }
    acts = Core.stateActions(retryable)
    hasRetry = false
    for (i = 0; i < acts.length; i++)
      if (acts[i].kind === "retry") hasRetry = true
    verify(hasRetry, "retryable error must offer Retry")
  }
```

Rodar gate → red.

- [ ] **Step 2: `stateActions` lê `error.retryable`**

Em `ServiceCore.js` `stateActions` (~1162-1192): Read a função primeiro. Onde ela injeta/permite a ação `retry` derivada do state, adicionar o filtro:

```javascript
  var retryAllowed = !provider || !provider.error
      || provider.error.retryable === undefined
      || provider.error.retryable === true
```

e descartar entradas `kind === "retry"` quando `!retryAllowed`. Login/view_installation não mudam.

- [ ] **Step 3: ProviderHeader enxuto**

Em `components/ProviderHeader.qml`: remover o `Text` "Updated ..." (linhas ~104-115) e a prop `lastSuccessAt`; o plano (linhas 49-57) vira pill:

```qml
      Rectangle {
        visible: root.plan.length > 0
        anchors.verticalCenter: parent.verticalCenter
        width: planText.implicitWidth + Style.space(10)
        height: planText.implicitHeight + Style.space(4)
        radius: height / 2
        color: "transparent"
        border.width: 1
        border.color: Qt.rgba(root.foreground.r, root.foreground.g, root.foreground.b, 0.25)
        Text {
          id: planText
          anchors.centerIn: parent
          text: root.plan
          color: Qt.darker(root.foreground, 1.2)
          font.family: root.fontFamily
          font.pixelSize: Style.font.caption
          textFormat: Text.PlainText
          Accessible.name: "plan " + root.plan
        }
      }
```

Remover também o `Text` de connection do header (a connection desce pro footer). Ajustar o spacer conforme.

- [ ] **Step 4: ProviderView reestruturado**

Substituir o corpo de `ProviderView.qml` mantendo assinatura externa (props/signals). Estrutura nova (código completo):

```qml
import QtQuick
import qs.Commons
import "ServiceCore.js" as Core
import "components"

// Single selected-provider content pane, "Camadas" (Fase 2):
// header -> [stale banner] -> primary windows (large) -> model list (quiet)
// -> state message (non-window modes) -> meta footer.
Item {
  id: root

  property var provider: null
  property string displayMetric: "remaining"
  property bool refreshing: false
  property color foreground: Color.foreground
  property string fontFamily: Style.font.family

  signal refreshRequested(string providerId)
  signal actionRequested(string providerId, string kind, var target)

  // Re-humanize countdowns while the popup stays open.
  property double nowMs: Date.now()
  Timer {
    interval: 30000
    running: root.visible
    repeat: true
    onTriggered: root.nowMs = Date.now()
  }

  readonly property var header: Core.headerModel(provider, refreshing)
  readonly property string mode: Core.contentMode(provider)
  readonly property var groups: Core.windowGroups(provider, displayMetric, nowMs)
  readonly property var actions: Core.stateActions(provider)
  readonly property bool isStale: root.mode === "stale_windows"
  readonly property string unitText: root.displayMetric === "used" ? "used" : "left"
  readonly property color accentColor: Color.foreground // Task 5 Step 1 decide o token real

  width: parent ? parent.width : implicitWidth
  implicitHeight: body.implicitHeight
  height: implicitHeight

  Column {
    id: body
    width: parent.width
    spacing: Style.space(10)

    ProviderHeader {
      width: parent.width
      name: root.header.name
      plan: root.header.plan
      refreshing: root.header.refreshing
      showStale: root.header.showStale
      foreground: root.foreground
      fontFamily: root.fontFamily
      onRefreshClicked: {
        if (root.provider && root.provider.id)
          root.refreshRequested(String(root.provider.id))
      }
    }

    Rectangle {
      width: parent.width
      height: 1
      color: Qt.rgba(root.foreground.r, root.foreground.g, root.foreground.b, 0.12)
    }

    // Stale banner: glyph + typed message + Retry (never color-only).
    Row {
      visible: root.isStale
      width: parent.width
      spacing: Style.space(8)

      Text {
        text: "\u231b"
        color: root.foreground
        font.family: root.fontFamily
        font.pixelSize: Style.font.body
        textFormat: Text.PlainText
        Accessible.ignored: true
      }
      Text {
        width: Math.max(0, parent.width - Style.space(120))
        text: "Stale \u2014 " + Core.errorMessage(root.provider)
        color: root.foreground
        font.family: root.fontFamily
        font.pixelSize: Style.font.caption
        wrapMode: Text.WordWrap
        textFormat: Text.PlainText
        Accessible.name: text
      }
      Repeater {
        model: root.isStale ? root.actions : []
        Text {
          required property var modelData
          visible: String(modelData.kind || "") === "retry"
          text: modelData.label
          color: root.foreground
          font.family: root.fontFamily
          font.pixelSize: Style.font.caption
          font.underline: true
          textFormat: Text.PlainText
          Accessible.name: text
          Accessible.role: Accessible.Button
          Accessible.onPressAction: activate()
          MouseArea {
            anchors.fill: parent
            cursorShape: Qt.PointingHandCursor
            onClicked: parent.activate()
          }
          function activate() {
            if (!root.provider)
              return
            root.actionRequested(String(root.provider.id),
                                 String(modelData.kind || ""), modelData.target)
          }
        }
      }
    }

    // Primary windows, large.
    Column {
      width: parent.width
      spacing: Style.space(12)
      visible: root.mode === "windows" || root.mode === "stale_windows"

      Repeater {
        model: root.groups.primary
        UsageWindow {
          required property var modelData
          width: parent.width
          label: modelData.label
          percentText: modelData.percentText
          percent: modelData.percent !== undefined && modelData.percent !== null
              ? Number(modelData.percent) : -1
          resetText: modelData.resetText ? modelData.resetText : ""
          unitText: root.unitText
          emphasis: true
          dimmed: root.isStale
          foreground: root.foreground
          accent: root.accentColor
          fontFamily: root.fontFamily
        }
      }
    }

    // Secondary (per-model) windows, quiet list.
    Column {
      width: parent.width
      spacing: Style.space(2)
      visible: (root.mode === "windows" || root.mode === "stale_windows")
          && root.groups.secondary.length > 0

      Rectangle {
        width: parent.width
        height: 1
        color: Qt.rgba(root.foreground.r, root.foreground.g, root.foreground.b, 0.08)
      }

      Repeater {
        model: root.groups.secondary
        UsageWindow {
          required property var modelData
          width: parent.width
          label: modelData.label
          percentText: modelData.percentText
          percent: modelData.percent !== undefined && modelData.percent !== null
              ? Number(modelData.percent) : -1
          resetText: ""
          unitText: root.unitText
          emphasis: false
          dimmed: root.isStale
          foreground: root.foreground
          fontFamily: root.fontFamily
        }
      }
    }

    StateMessage {
      width: parent.width
      visible: root.mode === "skeleton" || root.mode === "empty_windows" || root.mode === "state"
      skeleton: root.mode === "skeleton"
      title: root.mode === "skeleton" ? "" : Core.stateTitle(root.provider)
      body: root.mode === "skeleton" ? "" : Core.stateBody(root.provider)
      actions: root.mode === "skeleton" ? [] : root.actions
      foreground: root.foreground
      fontFamily: root.fontFamily
      onActionActivated: function (kind, target) {
        if (!root.provider)
          return
        root.actionRequested(String(root.provider.id), kind, target)
      }
    }

    // Meta footer: age + source left, connection right (was header noise).
    Row {
      width: parent.width
      visible: root.mode === "windows" || root.mode === "stale_windows"

      Text {
        id: footerLeft
        width: Math.max(0, parent.width * 0.6)
        text: {
          var parts = []
          var age = Core.formatAgoText(
            root.header.lastSuccessAt ? root.header.lastSuccessAt : "", root.nowMs)
          if (age.length)
            parts.push("Updated " + age)
          if (root.provider && root.provider.source)
            parts.push(String(root.provider.source) === "cache" ? "Cache" : "Live")
          if (root.header.refreshing)
            parts.push("refreshing\u2026")
          return parts.join(" \u00b7 ")
        }
        color: Qt.darker(root.foreground, 1.4)
        font.family: root.fontFamily
        font.pixelSize: Style.font.caption
        elide: Text.ElideRight
        textFormat: Text.PlainText
        Accessible.name: text
      }
      Text {
        width: Math.max(0, parent.width - footerLeft.width)
        text: root.header.connection
        color: Qt.darker(root.foreground, root.header.showStale ? 1.0 : 1.15)
        font.family: root.fontFamily
        font.pixelSize: Style.font.caption
        font.bold: root.header.showStale
        horizontalAlignment: Text.AlignRight
        elide: Text.ElideRight
        textFormat: Text.PlainText
        Accessible.name: text
      }
    }
  }
}
```

Nota: `headerModel` continua fornecendo `lastSuccessAt`/`connection` — nada muda no ServiceCore aqui além do Step 2.

- [ ] **Step 5: Rodar gate QML inteiro e atualizar testes afetados**

`tst_Popup.qml`/`tst_ProviderStates.qml` que procuram o texto "Stale" isolado ou a linha "Updated " no header: atualizar para o banner ("Stale \u2014 ...") e o footer. `tst_Screenshots.qml:99/106/114`: atualizar os `bodyText` de exemplo para os labels novos ("5h Reset 58% left · Max plan" etc.). Expected final: PASS.

- [ ] **Step 6: Commit**

```bash
git add assets/omarchy tests/qml
git commit -m 'feat: layered provider view with meta footer'
```

---

### Task 7: Split do ServiceCore.js

Fatiar por responsabilidade seguindo o inventário; convenção Omarchy (cada QML importa cada lib; sem `.import` JS→JS — sem precedente no shell). Funções test-only saem do bundle.

Módulos e conteúdo (line ranges do inventário pré-Fase-2; após Tasks 3-6 as linhas deslocam — localizar por NOME de função):

| Módulo novo | Concerns | Funções |
|---|---|---|
| `CoreService.js` | envelope+consts, IPC/probe, pending, lanes | `CLOSED_PROVIDERS`, `ACTION_KINDS`, `PROVIDER_STATES`, `isFinitePercent`, `validateProvider`, `parseStatusEnvelope`, `shouldApplyGeneration`, `health`, `isClosedProvider`, `isArrayLike`, `refreshResult`, `parseVersionStdout`, `emptyPending`, `clonePending`, `pendingIsEmpty`, `unionForced`, `takePending`, `statusArgv`, `canStartLane`, `requestPopup`, `closePopup`, `dismissPopup`, `foreignPopupOpen`, `popupOwnerId`, `popupOpenForOwner`, `popupView` |
| `CoreSettings.js` | settings | todo o bloco (d) (`settingsClosed`…`settingsArgvApplyStdin`) |
| `CoreMaintenance.js` | maintenance | todo o bloco (e) (`maintenanceIdle`…`maintenanceIntention`) |
| `CoreView.js` | chip+popup presentation + format | bloco (g) + (h) + `parseIsoMs`/`countdownText`/`formatResetText`/`formatAgoText`/`windowGroups`/`PRIMARY_WINDOW_IDS` |
| `CoreScroll.js` | a11y/scroll | todo o bloco (i) |
| `tests/qml/TestPalette.js` | test-only | `requiredScreenshotNames`, `themePalette` (FORA de assets/) |

Dependências cross-módulo e resolução:
- `CoreSettings` usa `CLOSED_PROVIDERS` e `defaultSettings` → `defaultSettings` MOVE para `CoreService.js` (vira primitivo compartilhado); `CoreSettings.js` e `CoreView.js` NÃO duplicam consts: QML sempre chama `Settings.validateSettingsDraft(draft, Service.defaultSettings())`? NÃO — para não mudar assinaturas: `CoreSettings.js` e `CoreView.js` declaram `.import "CoreService.js" as Kernel` — o Step 1 PROVA esse mecanismo antes de qualquer movimentação; se o probe falhar, fallback documentado: duplicar APENAS `isArrayLike` (5 linhas) e mover `CLOSED_PROVIDERS`+`defaultSettings`+`isClosedProvider` para o topo de cada módulo dependente com um comentário "mirror of CoreService — guarded by tests/servicecore_contract.rs", e estender o teste da Task 2 para validar TODOS os arquivos que declararem os consts.

**Files:**
- Create: `assets/omarchy/CoreService.js`, `CoreSettings.js`, `CoreMaintenance.js`, `CoreView.js`, `CoreScroll.js`; `tests/qml/TestPalette.js`
- Modify: todos os QML de assets/ e tests/qml (imports por alias — tabela de consumo no inventário; ex.: `BarWidget.qml` importa `CoreService.js as Service` + `CoreView.js as View`)
- Delete: `assets/omarchy/ServiceCore.js` (ao final, quando zero referências)
- Modify: `tests/servicecore_contract.rs` (path → `assets/omarchy/CoreService.js`)

**Interfaces:**
- Produces: mesma API pública, endereçada por módulo. Nenhuma função muda de corpo — split é MOVE puro.

- [ ] **Step 1: Probe do `.import` JS→JS**

Criar `tests/qml/tst_JsImportProbe.qml` + dois JS mínimos em `tests/qml/probe/` (`ProbeKernel.js` com `.pragma library` e `function two() { return 2 }`; `ProbeUser.js` com `.pragma library`, `.import "probe/ProbeKernel.js" as K` e `function four() { return K.two() * 2 }`). Teste: `compare(User.four(), 4)`. Rodar. Anotar PASS/FAIL no report — decide o mecanismo (import vs mirror). Remover o probe após a decisão (não shippa).

- [ ] **Step 2: Criar os 5 módulos + TestPalette movendo funções POR NOME**

Cortar-e-colar dos blocos (sem editar corpos). Cada módulo abre com `.pragma library` e um comentário de uma linha declarando o concern. Conforme o resultado do Step 1: `CoreSettings.js`/`CoreView.js`/`CoreMaintenance.js` usam `.import "CoreService.js" as Kernel` e prefixam `Kernel.` nas chamadas cross-módulo (`isClosedProvider`, `isArrayLike`, `CLOSED_PROVIDERS`, `defaultSettings`) — OU aplicam o fallback de espelho documentado acima.

- [ ] **Step 3: Reapontar os imports QML**

Tabela (do inventário): Service.qml → Service+Settings+Maintenance+View (4 aliases); BarWidget.qml → Service+View; Popup.qml → Service+View+Scroll; ProviderRail.qml → View; ProviderView.qml → View; SettingsView.qml → View; MaintenanceView.qml → Maintenance; components/FocusController.qml → Scroll. Testes: cada tst importa o(s) módulo(s) que consome (lista por arquivo no inventário); `tst_Screenshots.qml`/`tst_Accessibility.qml` importam também `TestPalette.js` local.

- [ ] **Step 4: Atualizar `tests/servicecore_contract.rs`**

Path do arquivo → `assets/omarchy/CoreService.js`. Se o fallback de espelho foi usado, estender: iterar sobre `assets/omarchy/Core*.js` e validar os consts em TODO arquivo que os declarar.

- [ ] **Step 5: Deletar ServiceCore.js e rodar TUDO**

`rg -l 'ServiceCore' assets tests` deve retornar vazio → deletar o arquivo. Rodar gate QML COMPLETO + `omarchy plugin validate assets/omarchy` + `cargo test` (contrato Task 2). Expected: 100% verde, mesmos totais de testes (161+novos das tasks anteriores).

- [ ] **Step 6: Commit**

```bash
git add -A assets/omarchy tests
git commit -m 'refactor: split ServiceCore into concern modules'
```

---

### Task 8: Limpeza rust-core

**Files:**
- Modify: `src/status/coordinator.rs:221`, `src/cache/store.rs:121`, `src/cli/mod.rs:651`, `src/notifications/mod.rs:15,193`
- Modify: `src/cache/coordinator.rs` (remover `retain_forced` :87-94, `ForcedTargets::union` :29-38, `ForcedTargets::contains` :40-45 e o teste `all_dominates_forced_union` :146-156; MANTER `begin_collection`/`complete_collection`/`bypass_accepts`/`start_generation`/`complete_generation` — usados em produção)
- Modify: `Cargo.toml` (remover `anyhow`), `tests/active_legacy_scan.rs` (remover entry `anyhow` :344 + endurecer o teste)
- Modify: `tests/cli.rs:655-676` (isolamento do teste non-tty)

**Interfaces:** nenhuma nova — remoções e um teste endurecido.

- [ ] **Step 1: Endurecer o teste anti-dormant (red com anyhow presente)**

Em `tests/active_legacy_scan.rs`, dentro do teste `active_legacy_scan_cargo_and_install_contract` (ou função nova `dependencies_are_actually_used`), após o check de owners, adicionar verificação de USO real:

```rust
#[test]
fn dependencies_are_actually_used_in_src() {
    let deps = parse_direct_deps_of_section("dependencies");
    let mut sources = String::new();
    for entry in walkdir_rs_files("src") {
        sources.push_str(&std::fs::read_to_string(&entry).expect("read src file"));
        sources.push('\n');
    }
    for dep in deps {
        let ident = dep.replace('-', "_");
        assert!(
            sources.contains(&ident),
            "dependency '{dep}' is declared in Cargo.toml but never referenced in src/ — remove it or use it"
        );
    }
}
```

Adapte aos helpers reais do arquivo (Read primeiro; `parse_direct_deps` já existe em ~:367-390 — reutilizar; escrever `walkdir_rs_files` com `std::fs::read_dir` recursivo se não houver util). Expected red: `anyhow` declarado e nunca referenciado.

- [ ] **Step 2: Remover anyhow**

`Cargo.toml`: deletar a linha do `anyhow`. `tests/active_legacy_scan.rs:344`: deletar a entry do map. `cargo build` + re-rodar o teste do Step 1 → verde.

- [ ] **Step 3: Remover os 4 `let _ =` vestigiais + subsistema forced-targets**

- `src/status/coordinator.rs:221`: deletar `let _ = ForcedTargets::empty();` (e o import de `ForcedTargets` se ficar órfão — o compilador avisa).
- `src/cache/store.rs:121`: deletar `let _ = now;` — se `now` virar parâmetro sem uso, o compilador aponta; renomear o parâmetro para `_now` OU remover o parâmetro se nenhum call site precisar (Read a função e os call sites primeiro; escolher a menor mudança).
- `src/cli/mod.rs:651`: deletar `let _ = Clock::now_utc(&clock);`.
- `src/notifications/mod.rs:193`: deletar `let _ = DisplayMetric::Remaining;` + remover `DisplayMetric` do import na linha 15.
- `src/cache/coordinator.rs`: remover `retain_forced`, `ForcedTargets::union`, `ForcedTargets::contains` e o teste `all_dominates_forced_union`. `ForcedTargets::empty` e o campo `pending_forced` — Read o arquivo: se após as remoções `pending_forced` nunca for populado, simplificar `complete_collection` para não retornar pending (e ajustar o call site `status/coordinator.rs:204-207`, removendo o `log::debug!` morto). Manter o tipo `ForcedTargets` só se algo ainda o referenciar; senão, remover por completo.

- [ ] **Step 4: Isolar o teste non-tty (red conhecido → verde)**

Em `tests/cli.rs` `binary_interactive_update_rejects_non_tty` (~655-676), adicionar isolamento como o teste vizinho `binary_doctor_clean_backs_up_and_removes_owned_legacy` (:640-650):

```rust
    let dir = tempdir().unwrap();
    let home = dir.path();
    let mut child = StdCommand::new(&bin)
        .arg("update")
        .env("HOME", home)
        .env("XDG_STATE_HOME", home.join("state"))
        .env("XDG_CACHE_HOME", home.join("cache"))
        .env("XDG_CONFIG_HOME", home.join("config"))
        .stdin(std::process::Stdio::piped())
        ...
```

Rodar `cargo test --test cli binary_interactive_update_rejects_non_tty` → agora PASS nesta máquina (exit 3 VALIDATION, não mais 5 PLUGIN).

- [ ] **Step 5: Gate Rust completo**

`cargo fmt --check && cargo test && cargo clippy --all-targets -- -D warnings && git diff --check` — TUDO verde, incluindo o ex-pré-existente.

- [ ] **Step 6: Commit**

```bash
git add -A src tests Cargo.toml Cargo.lock
git commit -m 'chore: remove dead code and dormant dependency'
```

---

### Task 9: Docs + gate integral + checkpoint QA ao vivo

**Files:**
- Modify: `CLAUDE.md` (seção Verification — comando QML)

- [ ] **Step 1: Corrigir o comando QML no CLAUDE.md**

Substituir o bloco `QT_QPA_PLATFORM=offscreen qmltestrunner ...` por:

```bash
QML_XHR_ALLOW_FILE_READ=1 QT_LOGGING_TO_CONSOLE=1 QT_QPA_PLATFORM=offscreen \
  /usr/lib/qt6/bin/qmltestrunner \
  -input tests/qml \
  -import /usr/share/omarchy/shell \
  -import assets/omarchy \
  -o -,txt
```

com a nota: "The bare `qmltestrunner` on Arch resolves to the Qt5 binary and fails silently (output goes to journald when not a TTY)."

- [ ] **Step 2: Gate integral (Rust + QML) na árvore final**

Todos os comandos dos Global Constraints. Expected: 100% verde, zero exceções.

- [ ] **Step 3: Commit**

```bash
git add CLAUDE.md
git commit -m 'docs: fix QML test runner invocation'
```

- [ ] **Step 4: PARAR — checkpoint com o usuário para QA ao vivo**

Reportar gate + diff da fase (`git log --oneline <base>..HEAD`). Pedir autorização explícita para atualizar o plugin instalado (binário + QML novos — desta vez os ASSETS mudam, então o update é a árvore `assets/omarchy` inteira + binário, com backup) e validar na barra real: popup "Camadas" com dados reais, tooltip humanizado, chip com ⌛ quando aplicável, screenshot lado a lado com o mockup aprovado. NÃO prosseguir sem resposta.

---

## Fora deste plano (fases seguintes)

- Codex freshness/sandbox/cap/dedup — Fase 3.
- Amp classifier/parse-miss; Grok period type/expiry/dead code — Fase 4.
- Paridade com o widget nativo + emenda de contrato + settings schema Quattro — Fase 5.
