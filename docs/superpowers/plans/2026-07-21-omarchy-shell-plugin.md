# Plugin omarchy-shell (Omarchy 4) — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Instalar o agent-bar como bar-widget plugin nativo do omarchy-shell (Quickshell) do Omarchy 4, com chips por provider + popup, mantendo o fluxo Waybar intacto.

**Architecture:** O binário Rust continua a fonte única de dados (`agent-bar --format json`, envelope `schemaVersion: 1`). Um plugin QML (`agent-bar.usage`) é embutido no binário via `include_str!`/`include_bytes!` e escrito como drop-in em `~/.config/omarchy/plugins/agent-bar.usage/` pelo `agent-bar setup`, que passa a detectar qual bar existe (Waybar, omarchy-shell, ou ambos). Spec: `docs/superpowers/specs/2026-07-21-omarchy-shell-plugin-design.md`.

**Tech Stack:** Rust (edition atual do repo, anyhow, tempfile, serial_test, insta) + QML (Quickshell, componentes `qs.Ui`/`qs.Commons` do omarchy-shell).

**Desvio do spec (aprovado verificar com o usuário no handoff):** o spec dizia "`agent-bar update` reescreve o plugin". O fluxo de update troca o binário — o binário VELHO em execução escreveria QML da versão antiga. Em vez disso: `setup` é idempotente e reescreve tudo; `update` imprime um hint para re-rodar `setup` quando o drop-in existir (Task 7).

## Global Constraints

- **Rust/cargo only** — sem Node/npm/bun em runtime ou teste.
- **`scripts/agent-bar-open-terminal` permanece script Bash** — embutir o TEXTO via `include_str!` e escrevê-lo como script é permitido; nunca portar para Rust.
- **Nunca mutar desktop vivo em teste** — sempre temp dirs + paths injetados; jamais tocar `~/.config/omarchy` ou `~/.config/waybar` reais.
- **stdout limpo** — o comando `waybar`/`--format json` emite só JSON; mensagens de setup/uninstall usam `term_prompt` (comandos terminal podem escrever texto rico).
- **Nunca `unwrap()`/`expect()` em código de produção** (`lib.rs` já nega via clippy).
- **Identidade via constantes** de `src/app_identity.rs` — zero strings `"agent-bar.usage"` hardcoded fora dela.
- **Id do plugin: `agent-bar.usage`** — prefixo `omarchy.*` é reservado pelo shell.
- **Nomes legacy proibidos**: `qbar`, `agent-bar-omarchy`, `antigravity`, `llm-usage` — não usar em id, path, seletor ou cache key.
- Conventional Commits em PT, subject ≤ 50 chars.
- Verificação: comando mais estreito da matriz do CLAUDE.md §2; ampla só na Task 7.
- **Gotcha RTK**: output do cargo pode vir reformatado; use um filtro posicional por invocação de `cargo test`.
- `#[serial_test::serial]` em todo teste que toca env vars (`XDG_CONFIG_HOME`, `PATH`, `HOME`); restaurar env no fim do teste.

---

### Task 1: Constantes de identidade Omarchy

**Files:**
- Modify: `src/app_identity.rs`

**Interfaces:**
- Produces: `OMARCHY_PLUGIN_ID: &str = "agent-bar.usage"`, `OMARCHY_SHELL_DIR: &str = "/usr/share/omarchy/shell"` (usados pelas Tasks 3, 5, 6).

- [ ] **Step 1: Escrever o teste que falha**

Em `src/app_identity.rs`, dentro do `mod tests` existente:

```rust
    #[test]
    fn omarchy_plugin_id_is_namespaced_and_not_reserved() {
        assert_eq!(OMARCHY_PLUGIN_ID, "agent-bar.usage");
        assert!(!OMARCHY_PLUGIN_ID.starts_with("omarchy."));
        assert!(OMARCHY_PLUGIN_ID.contains('.'));
        assert!(OMARCHY_SHELL_DIR.starts_with('/'));
    }
```

- [ ] **Step 2: Rodar e ver falhar**

Run: `cargo test app_identity`
Expected: erro de compilação `cannot find value OMARCHY_PLUGIN_ID`.

- [ ] **Step 3: Implementação mínima**

Em `src/app_identity.rs`, após a linha `pub const APP_HIDDEN_CLASS` (linha 9):

```rust
/// Id do plugin bar-widget do omarchy-shell (Omarchy 4+). O prefixo
/// `omarchy.*` é reservado pelo shell — terceiros precisam de namespace.
pub const OMARCHY_PLUGIN_ID: &str = "agent-bar.usage";
/// Raiz do shell Quickshell do Omarchy — usada só como sinal de detecção.
pub const OMARCHY_SHELL_DIR: &str = "/usr/share/omarchy/shell";
```

- [ ] **Step 4: Rodar e ver passar**

Run: `cargo test app_identity`
Expected: PASS (2 testes).

- [ ] **Step 5: Commit**

```bash
git add src/app_identity.rs
git commit -m "feat: constantes de identidade omarchy"
```

---

### Task 2: Assets do plugin (manifest.json + Widget.qml)

**Files:**
- Create: `assets/omarchy/manifest.json`
- Create: `assets/omarchy/Widget.qml`

**Interfaces:**
- Produces: os dois arquivos que a Task 3 embute via `include_str!`. O manifest usa o placeholder literal `__AGENT_BAR_VERSION__` que a Task 3 substitui por `app_identity::VERSION`.
- Contrato QML consumido do shell (verificado no Omarchy `4.0.0.alpha` local): root `BarWidget` (de `qs.Ui`) recebe `bar`/`moduleName`/`settings` injetados; popup via `PopupCard` (de `qs.Ui`) com `anchorItem`/`owner`/`bar`/`open`/`contentWidth`/`contentHeight`; cores em `bar.foreground`/`bar.urgent`/`Color.*`; execução de comandos via `bar.run(cmd)`.

Não há teste cargo nesta task (arquivos de dados); a verificação é sintática.

- [ ] **Step 1: Criar `assets/omarchy/manifest.json`**

```json
{
  "schemaVersion": 1,
  "id": "agent-bar.usage",
  "name": "agent-bar",
  "version": "__AGENT_BAR_VERSION__",
  "author": "othavi0",
  "license": "MIT",
  "description": "LLM quota monitor (Claude, Codex, Amp, Grok) as native bar chips with a popup panel.",
  "kinds": ["bar-widget"],
  "activation": "on-demand",
  "entryPoints": {
    "barWidget": "Widget.qml"
  },
  "barWidget": {
    "displayName": "agent-bar",
    "description": "Quota chips per provider; left = popup, right = TUI, middle = refresh.",
    "category": "AI",
    "aliases": ["agent-bar"],
    "allowMultiple": false,
    "defaults": {
      "refreshIntervalSec": 60
    },
    "schema": [
      { "key": "refreshIntervalSec", "type": "integer", "label": "Refresh interval (seconds)", "min": 30, "max": 3600, "step": 30, "defaultValue": 60 }
    ]
  }
}
```

- [ ] **Step 2: Criar `assets/omarchy/Widget.qml`**

Conteúdo completo (contrato de dados: `docs/json-output.md`; espelha `severity_color_api` de `src/tui/widgets/severity.rs` — API conhecida vence, senão threshold ≥60/30/10):

```qml
import QtQuick
import QtQuick.Layouts
import Quickshell.Io
import qs.Commons
import qs.Ui

// agent-bar — chips de quota por provider + popup nativo.
// Fonte de dados: `agent-bar --format json` (envelope schemaVersion 1,
// ver docs/json-output.md no repo do agent-bar). Este arquivo é escrito
// pelo `agent-bar setup` e version-locked com o binário.
BarWidget {
  id: root
  moduleName: "agent-bar.usage"

  property bool popupOpen: false
  property var payload: null
  property bool stale: false
  property bool forceNext: false

  readonly property color fg: bar ? bar.foreground : Color.foreground
  readonly property color urgent: bar ? bar.urgent : Color.urgent
  readonly property string fontFamily: bar ? bar.fontFamily : "monospace"
  readonly property var providers: payload && Array.isArray(payload.providers) ? payload.providers : []
  readonly property int refreshIntervalSec: Math.max(30, Number(setting("refreshIntervalSec", 60)))

  function pathFromUrl(url) { return String(url).replace(/^file:\/\//, "") }
  readonly property string helperPath: pathFromUrl(Qt.resolvedUrl("scripts/agent-bar-open-terminal"))

  // Espelha severity_color_api (severity.rs): API conhecida vence o
  // threshold local; desconhecida/ausente cai em >=60 ok / 30-59 low /
  // 10-29 warn / <10 critical.
  function severityBucket(w) {
    if (!w) return "ok"
    var api = String(w.severity || "").toLowerCase()
    if (api === "normal" || api === "ok") return "ok"
    if (api === "warning" || api === "elevated" || api === "high") return "warn"
    if (api === "critical" || api === "exceeded" || api === "blocked") return "critical"
    var pct = Number(w.remaining)
    if (!isFinite(pct)) return "ok"
    if (pct < 10) return "critical"
    if (pct < 30) return "warn"
    if (pct < 60) return "low"
    return "ok"
  }

  function severityColor(bucket) {
    if (bucket === "critical") return urgent
    if (bucket === "warn") return Qt.tint(fg, Qt.rgba(urgent.r, urgent.g, urgent.b, 0.55))
    if (bucket === "low") return Qt.tint(fg, Qt.rgba(urgent.r, urgent.g, urgent.b, 0.30))
    return fg
  }

  function iconFile(id) {
    if (id === "claude") return "icons/claude-code-icon.png"
    if (id === "codex") return "icons/codex-icon.png"
    if (id === "amp") return "icons/amp-icon.svg"
    if (id === "grok") return "icons/grok-icon.svg"
    return ""
  }

  function chipLabel(p) {
    if (!p.available) return "–"
    var w = p.primary
    if (!w || !isFinite(Number(w.remaining))) return ""
    return Math.round(Number(w.remaining)) + "%"
  }

  // 300min ~ "5h", 10080min ~ "Weekly" (tolerâncias de classify_window).
  function windowLabel(w, fallback) {
    var m = Number(w && w.windowMinutes)
    if (isFinite(m) && m > 0) {
      if (Math.abs(m - 300) <= 60) return "5h"
      if (Math.abs(m - 10080) <= 1440) return "Weekly"
    }
    return fallback
  }

  function fmtReset(iso) {
    if (!iso) return ""
    var d = new Date(iso)
    if (isNaN(d.getTime())) return ""
    return Qt.formatDateTime(d, "ddd HH:mm")
  }

  function applyPayload(raw) {
    try {
      var data = JSON.parse(String(raw || ""))
      if (!data || data.schemaVersion !== 1 || !Array.isArray(data.providers))
        throw new Error("unexpected envelope")
      root.payload = data
      root.stale = false
    } catch (e) {
      root.stale = true // mantém o último payload bom
    }
  }

  function refresh(force) {
    if (fetchProc.running) return
    root.forceNext = !!force
    fetchProc.running = true
  }

  function openTui() {
    if (bar) bar.run(bar.shellQuote(root.helperPath) + " agent-bar menu")
  }

  implicitWidth: root.vertical ? root.barSize : chips.implicitWidth + 12
  implicitHeight: root.vertical ? chips.implicitHeight + 12 : root.barSize
  opacity: root.stale ? 0.55 : 1.0

  Process {
    id: fetchProc
    command: ["bash", "-lc", root.forceNext ? "agent-bar --format json --refresh" : "agent-bar --format json"]
    stdout: StdioCollector {
      waitForEnd: true
      onStreamFinished: {
        root.applyPayload(text)
        root.forceNext = false
      }
    }
  }

  Timer {
    interval: root.refreshIntervalSec * 1000
    running: true
    repeat: true
    triggeredOnStart: true
    onTriggered: root.refresh(false)
  }

  Grid {
    id: chips
    anchors.centerIn: parent
    columns: root.vertical ? 1 : Math.max(1, root.providers.length)
    columnSpacing: 10
    rowSpacing: 6

    Repeater {
      model: root.providers

      Item {
        id: chip
        required property var modelData
        readonly property string bucket: root.severityBucket(modelData.primary)
        readonly property string label: root.chipLabel(modelData)

        width: chipRow.implicitWidth
        height: root.vertical ? root.barSize - 6 : root.barSize

        Row {
          id: chipRow
          anchors.centerIn: parent
          spacing: 4

          Image {
            source: root.iconFile(chip.modelData.provider) ? Qt.resolvedUrl(root.iconFile(chip.modelData.provider)) : ""
            visible: source !== ""
            width: 13
            height: 13
            sourceSize.width: 13
            sourceSize.height: 13
            fillMode: Image.PreserveAspectFit
            anchors.verticalCenter: parent.verticalCenter
            opacity: chip.modelData.available ? 1.0 : 0.45
          }

          Text {
            visible: !root.vertical && chip.label !== ""
            text: chip.label
            color: root.severityColor(chip.bucket)
            font.family: root.fontFamily
            font.pixelSize: 12
            anchors.verticalCenter: parent.verticalCenter
          }
        }

        MouseArea {
          anchors.fill: parent
          acceptedButtons: Qt.LeftButton | Qt.RightButton | Qt.MiddleButton
          hoverEnabled: true
          onEntered: if (root.bar) root.bar.showTooltip(chip, chip.modelData.displayName + (chip.label ? " · " + chip.label : "") + (chip.modelData.available ? "" : " · " + String(chip.modelData.error || "unavailable")))
          onExited: if (root.bar) root.bar.hideTooltip(chip)
          onClicked: function(mouse) {
            if (mouse.button === Qt.RightButton) root.openTui()
            else if (mouse.button === Qt.MiddleButton) root.refresh(true)
            else root.popupOpen = !root.popupOpen
          }
        }
      }
    }

    Text {
      visible: root.providers.length === 0
      text: "agent-bar"
      color: root.fg
      font.family: root.fontFamily
      font.pixelSize: 12
    }
  }

  PopupCard {
    id: popup
    anchorItem: root
    owner: root
    bar: root.bar
    open: root.popupOpen
    contentWidth: Style.space(300)
    contentHeight: Math.min(popupCol.implicitHeight + Style.space(20), Style.space(480))

    Flickable {
      anchors.fill: parent
      contentHeight: popupCol.implicitHeight
      clip: true

      ColumnLayout {
        id: popupCol
        width: parent.width
        spacing: 10

        Repeater {
          model: root.providers

          ColumnLayout {
            id: section
            required property var modelData
            Layout.fillWidth: true
            spacing: 4

            RowLayout {
              Layout.fillWidth: true
              spacing: 6

              Image {
                source: root.iconFile(section.modelData.provider) ? Qt.resolvedUrl(root.iconFile(section.modelData.provider)) : ""
                visible: source !== ""
                width: 14; height: 14
                sourceSize.width: 14; sourceSize.height: 14
                fillMode: Image.PreserveAspectFit
              }
              Text {
                text: section.modelData.displayName
                color: root.fg
                font.family: root.fontFamily
                font.pixelSize: 13
                font.bold: true
              }
              Text {
                Layout.fillWidth: true
                text: String(section.modelData.plan || section.modelData.account || "")
                color: Qt.darker(root.fg, 1.45)
                font.family: root.fontFamily
                font.pixelSize: 11
                elide: Text.ElideRight
              }
            }

            Text {
              visible: !section.modelData.available
              text: String(section.modelData.error || "Unavailable")
              color: Qt.darker(root.fg, 1.3)
              font.family: root.fontFamily
              font.pixelSize: 11
              wrapMode: Text.WordWrap
              Layout.fillWidth: true
            }

            QuotaRow { window: section.modelData.primary; name: root.windowLabel(section.modelData.primary, "Session") }
            QuotaRow { window: section.modelData.secondary; name: root.windowLabel(section.modelData.secondary, "Weekly") }

            Repeater {
              // delegate sem required properties → `modelData` implícito do
              // contexto é a chave (nome do modelo)
              model: section.modelData.models ? Object.keys(section.modelData.models) : []
              QuotaRow {
                window: section.modelData.models[modelData]
                name: modelData
              }
            }

            PanelSeparator { Layout.fillWidth: true; foreground: root.fg; strength: 0.18 }
          }
        }

        Text {
          text: root.stale ? "stale — last fetch failed" : (root.payload ? "fetched " + root.fmtReset(root.payload.fetchedAt) : "loading…")
          color: Qt.darker(root.fg, 1.45)
          font.family: root.fontFamily
          font.pixelSize: 10
        }

        Text {
          text: "right-click: TUI · middle-click: refresh"
          color: Qt.darker(root.fg, 1.6)
          font.family: root.fontFamily
          font.pixelSize: 10
        }
      }
    }
  }

  component QuotaRow: RowLayout {
    id: row
    property var window: null
    property string name: ""
    visible: !!window
    Layout.fillWidth: true
    spacing: 6

    Text {
      text: row.name
      color: Qt.darker(root.fg, 1.3)
      font.family: root.fontFamily
      font.pixelSize: 11
      Layout.preferredWidth: 64
      elide: Text.ElideRight
    }

    Item {
      Layout.fillWidth: true
      height: 6

      Rectangle {
        anchors.fill: parent
        radius: 3
        color: Qt.rgba(root.fg.r, root.fg.g, root.fg.b, 0.18)
      }
      Rectangle {
        readonly property real used: row.window && isFinite(Number(row.window.remaining)) ? (100 - Math.max(0, Math.min(100, Number(row.window.remaining)))) / 100 : 0
        width: parent.width * used
        height: parent.height
        radius: 3
        color: root.severityColor(root.severityBucket(row.window))
      }
    }

    Text {
      readonly property string resetText: root.fmtReset(row.window ? row.window.resetsAt : "")
      text: (row.window && isFinite(Number(row.window.remaining)) ? Math.round(Number(row.window.remaining)) + "% left" : "") + (resetText ? " · " + resetText : "")
      color: Qt.darker(root.fg, 1.3)
      font.family: root.fontFamily
      font.pixelSize: 10
    }
  }
}
```

- [ ] **Step 3: Validar sintaxe do manifest**

Run: `python3 -m json.tool assets/omarchy/manifest.json > /dev/null && echo MANIFEST_OK`
Expected: `MANIFEST_OK`

(Não pipe grep proxiado pra dentro de arquivo — regra RTK. QML não tem linter garantido no CI; a validação estrutural real acontece na verificação manual final.)

- [ ] **Step 4: Commit**

```bash
git add assets/omarchy/manifest.json assets/omarchy/Widget.qml
git commit -m "feat: assets QML do plugin omarchy"
```

---

### Task 3: `src/omarchy_integration.rs` (embed, detect, install, remove)

**Files:**
- Create: `src/omarchy_integration.rs`
- Modify: `src/lib.rs` (registrar módulo)

**Interfaces:**
- Consumes: `app_identity::{OMARCHY_PLUGIN_ID, OMARCHY_SHELL_DIR, VERSION, TERMINAL_HELPER_NAME}`; arquivos da Task 2.
- Produces (usados pelas Tasks 5-7):
  - `pub fn default_omarchy_plugins_dir(home: &Path) -> PathBuf`
  - `pub fn omarchy_shell_present(shell_dir: &Path, path_var: Option<&OsStr>) -> bool`
  - `pub fn detect_omarchy_shell() -> bool`
  - `pub fn omarchy_cli_available() -> bool`
  - `pub struct InstalledOmarchyPlugin { pub plugin_dir: PathBuf }`
  - `pub fn install_omarchy_plugin(plugins_dir: &Path) -> anyhow::Result<InstalledOmarchyPlugin>`
  - `pub fn remove_omarchy_plugin(plugins_dir: &Path) -> std::io::Result<bool>`
  - `pub fn run_omarchy_enable_commands() -> Vec<String>` (warnings)
  - `pub fn run_omarchy_remove_commands() -> Vec<String>` (warnings)

- [ ] **Step 1: Registrar o módulo em `src/lib.rs`**

Após a linha `pub mod notify;`:

```rust
pub mod omarchy_integration;
```

- [ ] **Step 2: Escrever os testes que falham**

Criar `src/omarchy_integration.rs` começando pelos testes (o corpo vem no Step 4):

```rust
//! Integração com o omarchy-shell (Omarchy 4+): escreve o plugin bar-widget
//! `agent-bar.usage` como drop-in em `<plugins_dir>/agent-bar.usage/`.
//!
//! Os arquivos do plugin são EMBUTIDOS no binário (include_str!/include_bytes!)
//! de propósito: o QML fica version-locked com o schema de `--format json`
//! do mesmo binário. Contrato: docs/omarchy-shell.md.

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
        assert_eq!(dir, std::path::PathBuf::from("/tmp/xdg-test/omarchy/plugins"));
        std::env::remove_var("XDG_CONFIG_HOME");
        let dir = default_omarchy_plugins_dir(std::path::Path::new("/home/u"));
        assert_eq!(dir, std::path::PathBuf::from("/home/u/.config/omarchy/plugins"));
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
```

- [ ] **Step 3: Rodar e ver falhar**

Run: `cargo test omarchy_integration`
Expected: erro de compilação (funções não definidas).

- [ ] **Step 4: Implementação**

Corpo do módulo (acima do `mod tests`):

```rust
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

/// Sinal de omarchy-shell: raiz QML instalada E CLI `omarchy` no PATH.
/// Ambos exigidos — só o dir pode ser resíduo de pacote; só a CLI pode
/// ser um Omarchy < 4 sem shell.
pub fn omarchy_shell_present(shell_dir: &Path, path_var: Option<&OsStr>) -> bool {
    if !shell_dir.is_dir() {
        return false;
    }
    let Some(path_var) = path_var else {
        return false;
    };
    std::env::split_paths(path_var).any(|dir| dir.join("omarchy").is_file())
}

pub fn detect_omarchy_shell() -> bool {
    omarchy_shell_present(
        Path::new(OMARCHY_SHELL_DIR),
        std::env::var_os("PATH").as_deref(),
    )
}

/// Só a CLI (usado pelo uninstall best-effort, que não exige o shell dir).
pub fn omarchy_cli_available() -> bool {
    std::env::var_os("PATH")
        .map(|p| std::env::split_paths(&p).any(|dir| dir.join("omarchy").is_file()))
        .unwrap_or(false)
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
        vec!["plugin", "rescan"],
        vec!["plugin", "enable", OMARCHY_PLUGIN_ID, "--yes"],
        vec!["bar", "plugin", "add", OMARCHY_PLUGIN_ID, "--yes"],
    ]
    .iter()
    .filter_map(|args| run_omarchy(args))
    .collect()
}

/// Desativa/remove no shell (bar remove + plugin remove). Best-effort.
pub fn run_omarchy_remove_commands() -> Vec<String> {
    [
        vec!["bar", "plugin", "remove", OMARCHY_PLUGIN_ID, "--yes"],
        vec!["plugin", "remove", OMARCHY_PLUGIN_ID, "--yes"],
    ]
    .iter()
    .filter_map(|args| run_omarchy(args))
    .collect()
}
```

- [ ] **Step 5: Rodar e aceitar o snapshot**

Run: `cargo test omarchy_integration`
Expected: 5 testes PASS + 1 snapshot novo pendente. Revisar `src/snapshots/agent_bar__omarchy_integration__tests__omarchy_manifest.snap` manualmente (conferir id, placeholder substituído) e então:

Run: `cargo insta accept`

(Exceção consciente à nota "goldens nunca via insta accept" de `tests/golden.rs`: aquela regra protege snapshots Waybar byte-for-byte pré-existentes; este é um snapshot NOVO, revisado no diff antes do accept.)

Run: `cargo test omarchy_integration`
Expected: PASS (6 testes).

- [ ] **Step 6: Clippy focado**

Run: `cargo clippy --all-targets -- -D warnings`
Expected: sem warnings.

- [ ] **Step 7: Commit**

```bash
git add src/lib.rs src/omarchy_integration.rs src/snapshots/
git commit -m "feat: módulo de integração omarchy-shell"
```

---

### Task 4: Flag CLI `--omarchy-plugins-dir`

**Files:**
- Modify: `src/cli.rs`

**Interfaces:**
- Produces: `CliOptions.omarchy_plugins_dir: Option<String>` (consumido pela Task 5 no `main.rs`).

- [ ] **Step 1: Escrever os testes que falham**

No `mod tests` de `src/cli.rs` (perto do teste `command_doctor_dry_run_yes`, linha ~824):

```rust
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
```

- [ ] **Step 2: Rodar e ver falhar**

Run: `cargo test cli`
Expected: erro de compilação `no field omarchy_plugins_dir`.

- [ ] **Step 3: Implementação**

Em `src/cli.rs`:

1. Struct `CliOptions` (após `pub waybar_dir: Option<String>,` linha 52):

```rust
    pub omarchy_plugins_dir: Option<String>,
```

2. `impl Default` (após `waybar_dir: None,` linha 73):

```rust
            omarchy_plugins_dir: None,
```

3. `parse_args` (após o braço `"--waybar-dir"`, linha ~280-285):

```rust
            "--omarchy-plugins-dir" => {
                let val = require_next_arg(args, i, "--omarchy-plugins-dir")?;
                opts.omarchy_plugins_dir = Some(val.to_string());
                i += 1;
            }
```

4. Help (`build_help`, após a linha de `--waybar-dir`, linha ~655):

```rust
        opt_line("--omarchy-plugins-dir <path>", "Omarchy plugin target (setup)", no_color)
```

(Seguir exatamente o formato/joins das linhas vizinhas de `opt_line` — há testes de help.)

- [ ] **Step 4: Rodar e ver passar**

Run: `cargo test cli`
Expected: PASS (incluindo os testes de help existentes — se um snapshot/assert de help falhar, ajustar a string esperada no mesmo commit).

- [ ] **Step 5: Commit**

```bash
git add src/cli.rs
git commit -m "feat: flag --omarchy-plugins-dir no setup"
```

---

### Task 5: Setup detecta bar e instala o plugin

**Files:**
- Modify: `src/setup.rs`
- Modify: `src/main.rs:186-215` (braço `Command::Setup`)

**Interfaces:**
- Consumes: `omarchy_integration::{install_omarchy_plugin, run_omarchy_enable_commands, detect_omarchy_shell, default_omarchy_plugins_dir}` (Task 3), `CliOptions.omarchy_plugins_dir` (Task 4).
- Produces: `SetupConfig` ganha `pub omarchy: Option<OmarchySetupOptions>` e `pub skip_waybar: bool`; `pub struct OmarchySetupOptions { pub plugins_dir: PathBuf, pub run_cli: bool }`; `pub fn waybar_present(path_var: Option<&OsStr>) -> bool`.

- [ ] **Step 1: Escrever os testes que falham**

No `mod tests` de `src/setup.rs`:

```rust
    #[test]
    fn waybar_present_checks_path() {
        let bin = tempdir().unwrap();
        let path_var = std::ffi::OsString::from(bin.path());
        assert!(!waybar_present(Some(&path_var)));
        std::fs::write(bin.path().join("waybar"), b"").unwrap();
        assert!(waybar_present(Some(&path_var)));
        assert!(!waybar_present(None));
    }

    #[test]
    #[serial_test::serial]
    fn setup_omarchy_only_installs_plugin_and_skips_waybar() {
        let dest = tempdir().unwrap();
        let plugins = tempdir().unwrap();
        let s = load(&Paths {
            cache_dir: dest.path().join("c"),
            config_dir: dest.path().join("cfg"),
            claude_credentials: PathBuf::new(),
            codex_auth: PathBuf::new(),
            codex_sessions: PathBuf::new(),
            amp_settings: PathBuf::new(),
            amp_threads: PathBuf::new(),
            grok_home: PathBuf::new(),
            grok_auth: PathBuf::new(),
        });
        let cfg = SetupConfig {
            asset_paths: None,
            integration_paths: None,
            repo_root: None,
            home: dest.path().to_path_buf(),
            skip_reload: true,
            system_install: true,
            omarchy: Some(OmarchySetupOptions {
                plugins_dir: plugins.path().to_path_buf(),
                run_cli: false, // NUNCA roda `omarchy` real em teste
            }),
            skip_waybar: true,
        };
        let ok = run_setup(&s, cfg, false, false).unwrap();
        assert!(ok);
        let plugin_dir = plugins
            .path()
            .join(crate::app_identity::OMARCHY_PLUGIN_ID);
        assert!(plugin_dir.join("manifest.json").exists());
        assert!(plugin_dir.join("Widget.qml").exists());
        // fluxo waybar não rodou: nenhum config.jsonc/style.css criado
        assert!(!dest.path().join("config.jsonc").exists());
    }
```

E atualizar o teste existente `setup_system_install_skips_symlink_uses_path_appbin` adicionando os campos novos ao literal `SetupConfig`:

```rust
            omarchy: None,
            skip_waybar: false,
```

- [ ] **Step 2: Rodar e ver falhar**

Run: `cargo test setup`
Expected: erro de compilação (campos/tipos não existem).

- [ ] **Step 3: Implementação em `src/setup.rs`**

1. Import no topo:

```rust
use crate::omarchy_integration::{install_omarchy_plugin, run_omarchy_enable_commands};
```

2. Detecção de waybar (após `reload_waybar`, ~linha 27):

```rust
/// Waybar presente = binário `waybar` em `path_var`. Sinal para o setup
/// decidir se o fluxo Waybar roda (em Omarchy 4 puro, não há waybar).
pub fn waybar_present(path_var: Option<&std::ffi::OsStr>) -> bool {
    let Some(path_var) = path_var else {
        return false;
    };
    std::env::split_paths(path_var).any(|dir| dir.join("waybar").is_file())
}
```

3. Struct nova + campos em `SetupConfig` (após `pub system_install: bool,`):

```rust
/// Instalação do plugin omarchy-shell dentro do setup.
pub struct OmarchySetupOptions {
    /// Destino dos plugins (`~/.config/omarchy/plugins` em produção;
    /// temp dir em teste via `--omarchy-plugins-dir`).
    pub plugins_dir: PathBuf,
    /// Roda `omarchy plugin rescan/enable` + `bar plugin add` após
    /// escrever os arquivos. SEMPRE false em testes.
    pub run_cli: bool,
}
```

```rust
    /// `Some` = instala o plugin do omarchy-shell.
    pub omarchy: Option<OmarchySetupOptions>,
    /// `true` = pula o fluxo Waybar inteiro (assets + wiring + reload).
    /// Usado quando só o omarchy-shell foi detectado.
    pub skip_waybar: bool,
```

4. Em `run_setup`, envolver os passos Waybar (passos 4, 6 e 7 atuais — instalação de assets, `apply_waybar_integration` e `reload_waybar`; linhas 100-144) em `if !cfg.skip_waybar { ... }`. O passo 5 (symlink) fica FORA do gate — instalação dev precisa do binário no PATH também para o QML. Os `term_prompt::status` de Icons/Helper (linhas 148-149) entram no mesmo gate (as variáveis `installed`/`asset` só existem nele).

5. Ainda em `run_setup`, após o bloco Waybar e antes do passo 8:

```rust
    // Omarchy-shell: escreve o drop-in e (fora de testes) ativa via CLI.
    if let Some(om) = &cfg.omarchy {
        let installed = install_omarchy_plugin(&om.plugins_dir)?;
        term_prompt::status("Omarchy", &installed.plugin_dir.to_string_lossy());
        if om.run_cli {
            for warning in run_omarchy_enable_commands() {
                term_prompt::status("Aviso", &warning);
            }
        }
    }
```

6. Ajustar a nota inicial (linha 87-89) para refletir o alvo:

```rust
    let target = match (&cfg.omarchy, cfg.skip_waybar) {
        (Some(_), true) => "integração omarchy-shell",
        (Some(_), false) => "integração Waybar + omarchy-shell",
        (None, _) => "integração Waybar",
    };
    term_prompt::note(&format!(
        "{APP_NAME} setup — instalando icons, helper e {target}"
    ));
```

- [ ] **Step 4: Wiring em `src/main.rs` (braço `Command::Setup`, linhas 186-215)**

Substituir a construção do `cfg` por:

```rust
            let omarchy_forced = opts.omarchy_plugins_dir.as_ref().map(PathBuf::from);
            let omarchy_detected = omarchy_integration::detect_omarchy_shell();
            let omarchy = match (omarchy_forced, omarchy_detected) {
                (Some(dir), _) => Some(setup::OmarchySetupOptions {
                    plugins_dir: dir,
                    run_cli: false, // dir injetado = teste/CI: não toca o shell vivo
                }),
                (None, true) => Some(setup::OmarchySetupOptions {
                    plugins_dir: omarchy_integration::default_omarchy_plugins_dir(&home),
                    run_cli: true,
                }),
                (None, false) => None,
            };
            let skip_waybar =
                omarchy.is_some() && !setup::waybar_present(std::env::var_os("PATH").as_deref());
            let cfg = setup::SetupConfig {
                asset_paths: Some(asset_paths),
                integration_paths: Some(ipaths),
                repo_root: None,
                home,
                skip_reload: false,
                system_install: runtime::is_system_install(),
                omarchy,
                skip_waybar,
            };
```

(`home` já é construído acima no mesmo braço, linha 197-199 — mover a construção para antes se necessário. Adicionar `omarchy_integration` ao `use` do topo do main se os módulos são referenciados por caminho completo `agent_bar::...`, seguindo o padrão dos imports existentes.)

Há um segundo local que constrói `SetupConfig` (linha ~380, fluxo interno do update/first-run): adicionar `omarchy: None, skip_waybar: false` lá — comportamento inalterado.

- [ ] **Step 5: Rodar e ver passar**

Run: `cargo test setup`
Expected: PASS (teste antigo atualizado + 2 novos).

Run: `cargo clippy --all-targets -- -D warnings`
Expected: limpo.

- [ ] **Step 6: Commit**

```bash
git add src/setup.rs src/main.rs
git commit -m "feat: setup detecta omarchy-shell"
```

---

### Task 6: Uninstall remove o plugin

**Files:**
- Modify: `src/uninstall.rs`
- Modify: `src/main.rs` (call sites de `run_uninstall` — comandos `Uninstall` e `Remove`)

**Interfaces:**
- Consumes: `omarchy_integration::{default_omarchy_plugins_dir, remove_omarchy_plugin, run_omarchy_remove_commands, omarchy_cli_available, OMARCHY_PLUGIN_ID}`.
- Produces: `run_uninstall` ganha o parâmetro `omarchy_plugins_dir: &Path` (posição: após `integration_paths`).

- [ ] **Step 1: Implementação em `src/uninstall.rs`**

1. Imports:

```rust
use crate::app_identity::{APP_NAME, OMARCHY_PLUGIN_ID};
use crate::omarchy_integration::{
    omarchy_cli_available, remove_omarchy_plugin, run_omarchy_remove_commands,
};
```

2. Assinatura de `run_uninstall` — adicionar parâmetro final:

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

3. Na nota inicial (linha 45-56), acrescentar o path do plugin à lista:

```rust
        omarchy_plugins_dir.join(OMARCHY_PLUGIN_ID).display(),
```

(com o `• {}` correspondente na format string).

4. Após o passo 4 (remoção de paths individuais, linha ~78), antes do reload:

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

- [ ] **Step 2: Atualizar call sites no `main.rs`**

Localizar com `rg -n 'run_uninstall' src/main.rs` (comandos `Uninstall` e `Remove`). Em cada um, passar o novo argumento:

```rust
                &omarchy_integration::default_omarchy_plugins_dir(&home),
```

(`home` já existe nesses braços; se não, construir igual ao braço Setup.)

- [ ] **Step 3: Verificar**

Run: `cargo build`
Expected: compila sem erros/warnings.

Run: `cargo test omarchy_integration`
Expected: PASS (cobre `remove_omarchy_plugin`; `run_uninstall` segue sem teste direto — padrão atual do arquivo, risco declarado no report final).

Run: `cargo clippy --all-targets -- -D warnings`
Expected: limpo.

- [ ] **Step 4: Commit**

```bash
git add src/uninstall.rs src/main.rs
git commit -m "feat: uninstall remove plugin omarchy"
```

---

### Task 7: Hint no update, docs e verificação ampla

**Files:**
- Modify: `src/main.rs` (braço `Command::Update`, linha ~292)
- Modify: `docs/integration.md`
- Create: `docs/omarchy-shell.md`
- Modify: `README.md` (seção de features/instalação)

**Interfaces:**
- Consumes: `omarchy_integration::{default_omarchy_plugins_dir, OMARCHY_PLUGIN_ID}`.

- [ ] **Step 1: Hint pós-update**

No braço `Command::Update` do `main.rs`, no caminho de sucesso do update (após o report de sucesso existente — localizar com `rg -n 'Command::Update' src/main.rs` e ler o braço):

```rust
            // O binário novo traz QML novo — o drop-in só atualiza via setup.
            let home = std::env::var_os("HOME").map(PathBuf::from).unwrap_or_default();
            let plugin_dir = omarchy_integration::default_omarchy_plugins_dir(&home)
                .join(app_identity::OMARCHY_PLUGIN_ID);
            if plugin_dir.exists() {
                term_prompt::note(&format!(
                    "Plugin omarchy-shell detectado. Rode `{} setup` para atualizá-lo.",
                    app_identity::APP_NAME
                ));
            }
```

(Ajustar paths de import ao padrão do arquivo. Se o braço Update delega tudo a `update::run_managed_update`, colocar o hint no `main.rs` logo após o `Ok` do resultado.)

- [ ] **Step 2: Criar `docs/omarchy-shell.md`**

```markdown
# Omarchy-shell plugin (Omarchy 4+)

O Omarchy 4 substituiu a Waybar pelo `omarchy-shell` (Quickshell). O
agent-bar se integra como bar-widget plugin de terceiro.

## O que o setup instala

`agent-bar setup` detecta o omarchy-shell (CLI `omarchy` no PATH +
`/usr/share/omarchy/shell/`) e escreve o drop-in:

```
~/.config/omarchy/plugins/agent-bar.usage/
  manifest.json          # id agent-bar.usage, version = versão do binário
  Widget.qml             # chips + popup (consome `agent-bar --format json`)
  icons/                 # ícones dos providers
  scripts/agent-bar-open-terminal
```

Depois roda `omarchy plugin rescan`, `omarchy plugin enable agent-bar.usage`
e `omarchy bar plugin add agent-bar.usage` (best-effort: falhas viram aviso
e os comandos podem ser rodados manualmente).

Se a Waybar também estiver instalada, o fluxo Waybar clássico roda junto.

## Widget

- Um chip por provider (ícone + % restante do limite primário), cores do
  tema do shell, severidade espelhando o TUI (≥60 ok / 30-59 / 10-29 / <10).
- Clique esquerdo: popup nativo (janelas primária/secundária, breakdown por
  modelo, reset, plan/conta).
- Clique direito: abre o TUI (`agent-bar menu`) em terminal flutuante.
- Clique do meio: refresh forçado (`--refresh`).
- Setting `refreshIntervalSec` (default 60, min 30) via
  `omarchy bar plugin set` ou `shell.json`.

## Dados

O QML roda `agent-bar --format json` (contrato em
[`json-output.md`](json-output.md)). Os arquivos QML são embutidos no
binário — version-locked com o schema. Após `agent-bar update`, re-rode
`agent-bar setup` para atualizar o drop-in (o update imprime esse hint).

## Remoção

`agent-bar uninstall`/`remove` desregistra o widget
(`omarchy bar plugin remove` + `omarchy plugin remove`, best-effort) e
apaga o diretório do plugin.

## Testes

Fluxo coberto por `cargo test omarchy_integration` e `cargo test setup`
com temp dirs (`--omarchy-plugins-dir`). O QML não tem harness
automatizado: mudanças visuais exigem verificação manual no desktop.
```

- [ ] **Step 3: Atualizar `docs/integration.md` e `README.md`**

Em `docs/integration.md`, adicionar ao final:

```markdown
## Omarchy 4 (omarchy-shell)

No Omarchy 4 a Waybar foi substituída pelo omarchy-shell (Quickshell).
`agent-bar setup` detecta e instala o plugin nativo — ver
[`omarchy-shell.md`](omarchy-shell.md).
```

No `README.md`, na lista de features/compatibilidade, adicionar uma linha
(seguir o formato das vizinhas):

```markdown
- **Omarchy 4 (omarchy-shell)**: bar-widget plugin nativo com chips + popup — `agent-bar setup` detecta e instala. Waybar segue suportada.
```

- [ ] **Step 4: Verificação ampla (handoff)**

Run: `git diff --check`
Expected: sem whitespace errors.

Run: `cargo test`
Expected: PASS total.

Run: `cargo clippy --all-targets -- -D warnings`
Expected: limpo.

- [ ] **Step 5: Commit**

```bash
git add src/main.rs docs/omarchy-shell.md docs/integration.md README.md
git commit -m "docs: contrato do plugin omarchy-shell"
```

---

## Pós-plano (fora das tasks — requer o usuário)

Verificação perceptual no desktop real (Omarchy 4.0.0.alpha do usuário), **com aprovação explícita** (hard rule: não mutar desktop vivo sem aprovação):

1. `agent-bar setup` real (instala o drop-in + ativa no shell).
2. Chips visíveis na barra com os 4 providers; comparar screenshot lado a
   lado com o widget `omarchy.model-usage` (padrão irmão).
3. Popup abre com dados reais (conferir percentuais contra `agent-bar --format json`).
4. Clique direito abre o TUI; meio força refresh.
5. Se algo falhar no QML: iterar com `omarchy-shell` logs (journalctl/stderr
   do quickshell) — nunca declarar "concluído" só com o Rust verde
   ("implementado, não verificado" até o passo 2-4 passar).
