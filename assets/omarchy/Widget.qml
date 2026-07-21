import QtQuick
import QtQuick.Layouts
import Quickshell.Io
import qs.Commons
import qs.Ui

// agent-bar — chips de quota por provider + popup nativo (usage + settings).
// Fonte de dados: `agent-bar --format json` (envelope schemaVersion 1,
// ver docs/json-output.md no repo do agent-bar). Settings editáveis via
// `agent-bar config show|apply`. Este arquivo é escrito pelo
// `agent-bar setup` e version-locked com o binário.
BarWidget {
  id: root
  moduleName: "agent-bar.usage"

  property bool popupOpen: false
  property var payload: null
  property bool stale: false
  property bool forceNext: false

  property bool settingsMode: false
  property var draftSettings: ({
    providers: [],
    providerOrder: [],
    displayMode: "remaining",
    notifyEnabled: true,
    refreshIntervalSec: 60
  })
  // null = config show ainda não retornou → sem filtro (mostra todos)
  property var enabledIds: null
  property string displayMode: "remaining"
  property string settingsStatusText: ""
  property bool settingsBusy: false

  readonly property color fg: bar ? bar.foreground : Color.foreground
  readonly property color urgent: bar ? bar.urgent : Color.urgent
  readonly property color dim: Qt.darker(fg, 1.45)
  readonly property string fontFamily: bar ? bar.fontFamily : "monospace"
  readonly property var providers: payload && Array.isArray(payload.providers) ? payload.providers : []
  // Clamp aos limites do schema do manifest (min 30 / max 3600) — o editor
  // do shell respeita o schema, mas shell.json editado à mão não.
  readonly property int refreshIntervalSec: Math.min(3600, Math.max(30, Number(setting("refreshIntervalSec", 60)) || 60))
  readonly property var knownProviderIds: ["claude", "codex", "amp", "grok"]

  // Chamado pelo PopupCard no clique-fora (owner.close()). Sem isto o card
  // faz `open = false` imperativo, o que DESTRÓI o binding open:popupOpen —
  // o popup nunca mais abre. Resetar a fonte da verdade preserva o binding.
  function close() {
    popupOpen = false
    settingsMode = false
    settingsBusy = false
  }

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

  function providerLabel(id) {
    if (id === "claude") return "Claude"
    if (id === "codex") return "Codex"
    if (id === "amp") return "Amp"
    if (id === "grok") return "Grok"
    return String(id || "")
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

  // 300min ~ "5h", 10080min ~ "Weekly" (tolerâncias de classify_window).
  function windowLabel(w, fallback) {
    var m = Number(w && w.windowMinutes)
    if (isFinite(m) && m > 0) {
      if (Math.abs(m - 300) <= 90) return "5h"
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
    // Util.shellQuote, não bar.shellQuote: o README do bar documenta
    // bar.shellQuote(), mas o Bar do 4.0.0.alpha não expõe a função —
    // widgets first-party usam Util (Workspaces.qml faz igual).
    if (bar) bar.run(Util.shellQuote(root.helperPath) + " agent-bar menu")
  }

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

  // apply via tempfile: Process.write existe, mas fechar stdin (EOF) para
  // `config apply --json -` não é API estável no Quickshell do Omarchy
  // 4.0.0.alpha — mktemp + --file evita o problema.
  function applyShellScript(blob) {
    return "f=$(mktemp) && printf '%s' " + Util.shellQuote(String(blob || ""))
      + " >\"$f\" && agent-bar config apply --file \"$f\"; e=$?; rm -f \"$f\"; exit $e"
  }

  function runConfigApply(blob) {
    if (configApplyProc.running) return
    configApplyProc.command = ["bash", "-lc", root.applyShellScript(blob)]
    configApplyProc.running = true
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
    root.runConfigApply(blob)
  }

  function onConfigApplyFinished(text, exitCode) {
    settingsBusy = false
    if (exitCode !== 0) {
      settingsStatusText = "apply failed"
      return
    }
    try {
      var data = JSON.parse(String(text || ""))
      if (!applyConfigView(data)) throw new Error("bad apply result")
    } catch (e) {
      settingsStatusText = "apply failed"
      return
    }
    // interval → shell.json (só se apply OK)
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
    var order = (draftSettings.providerOrder || []).filter(function(x) { return p.indexOf(x) >= 0 })
    p.forEach(function(x) { if (order.indexOf(x) < 0) order.push(x) })
    draftSettings = Object.assign({}, draftSettings, { providers: p, providerOrder: order })
  }

  function moveProvider(id, dir) {
    var p = (draftSettings.providers || []).slice()
    var idx = p.indexOf(id)
    if (idx < 0) return
    var j = idx + dir
    if (j < 0 || j >= p.length) return
    var tmp = p[idx]
    p[idx] = p[j]
    p[j] = tmp
    draftSettings = Object.assign({}, draftSettings, { providers: p, providerOrder: p.slice() })
  }

  function setDisplayMode(mode) {
    var m = mode === "used" ? "used" : "remaining"
    draftSettings = Object.assign({}, draftSettings, { displayMode: m })
  }

  function setNotifyEnabled(on) {
    draftSettings = Object.assign({}, draftSettings, { notifyEnabled: !!on })
  }

  function setRefreshInterval(sec) {
    var n = Math.min(3600, Math.max(30, Number(sec) || 60))
    draftSettings = Object.assign({}, draftSettings, { refreshIntervalSec: n })
  }

  // enabled first (draft order), then disabled known ids
  function settingsProviderRows() {
    var known = root.knownProviderIds
    var enabled = draftSettings.providers || []
    var rows = []
    for (var i = 0; i < enabled.length; i++) {
      if (known.indexOf(enabled[i]) >= 0)
        rows.push({ id: enabled[i], on: true, index: i, count: enabled.length })
    }
    for (var j = 0; j < known.length; j++) {
      if (enabled.indexOf(known[j]) < 0)
        rows.push({ id: known[j], on: false, index: -1, count: enabled.length })
    }
    return rows
  }

  implicitWidth: root.vertical ? root.barSize : chips.implicitWidth + 12
  implicitHeight: root.vertical ? chips.implicitHeight + 12 : root.barSize
  opacity: root.stale ? 0.55 : 1.0

  Component.onCompleted: {
    if (!configShowProc.running) configShowProc.running = true
  }

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
    command: ["bash", "-lc", "true"]
    stdout: StdioCollector {
      id: applyOut
      waitForEnd: true
    }
    onExited: function(exitCode, exitStatus) {
      root.onConfigApplyFinished(applyOut.text, exitCode)
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
    columns: root.vertical ? 1 : Math.max(1, root.visibleProviders().length)
    columnSpacing: 10
    rowSpacing: 6

    Repeater {
      model: root.visibleProviders()

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
          opacity: chip.modelData.staleReason ? 0.55 : 1.0

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
            if (mouse.button === Qt.RightButton) root.openSettings()
            else if (mouse.button === Qt.MiddleButton) root.refresh(true)
            else {
              root.showUsage()
              root.popupOpen = !root.popupOpen
            }
          }
        }
      }
    }

    Text {
      visible: root.visibleProviders().length === 0
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
    contentWidth: Style.space(370)
    contentHeight: Math.min(popupCol.implicitHeight + Style.space(20), Style.space(560))

    Flickable {
      anchors.fill: parent
      contentHeight: popupCol.implicitHeight
      clip: true

      ColumnLayout {
        id: popupCol
        width: parent.width
        spacing: 10

        // ── Settings header ──────────────────────────────────────────
        RowLayout {
          visible: root.settingsMode
          Layout.fillWidth: true
          spacing: 8

          Button {
            text: "← Usage"
            foreground: root.fg
            fontFamily: root.fontFamily
            fontSize: 10
            horizontalPadding: 8
            verticalPadding: 4
            onClicked: root.showUsage()
          }

          Text {
            text: "Settings"
            color: root.fg
            font.family: root.fontFamily
            font.pixelSize: 13
            font.bold: true
            Layout.fillWidth: true
            Layout.alignment: Qt.AlignVCenter
          }

          Button {
            text: root.settingsBusy ? "Saving…" : "Save"
            foreground: root.fg
            accent: Color.accent
            fontFamily: root.fontFamily
            fontSize: 10
            horizontalPadding: 8
            verticalPadding: 4
            active: true
            enabled: !root.settingsBusy
            onClicked: root.saveSettings()
          }
        }

        PanelSeparator {
          visible: root.settingsMode
          Layout.fillWidth: true
          foreground: root.fg
          strength: 0.18
        }

        // ── Usage sections ───────────────────────────────────────────
        Repeater {
          model: root.settingsMode ? [] : root.visibleProviders()

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
                color: root.dim
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

        // ── Settings body ────────────────────────────────────────────
        ColumnLayout {
          visible: root.settingsMode
          Layout.fillWidth: true
          spacing: 12

          PanelSectionHeader {
            Layout.fillWidth: true
            text: "Providers"
            foreground: root.fg
            fontFamily: root.fontFamily
            fontSize: 11
          }

          Repeater {
            model: root.settingsMode ? root.settingsProviderRows() : []

            ColumnLayout {
              id: provRow
              required property var modelData
              Layout.fillWidth: true
              spacing: 4

              Toggle {
                Layout.fillWidth: true
                label: root.providerLabel(provRow.modelData.id)
                description: checked ? "Shown in bar chips" : "Hidden from the bar"
                checked: provRow.modelData.on
                foreground: root.fg
                accent: Color.accent
                fontFamily: root.fontFamily
                onClicked: root.setProviderEnabled(provRow.modelData.id, !checked)
              }

              RowLayout {
                visible: provRow.modelData.on
                Layout.fillWidth: true
                spacing: 6

                Item { Layout.fillWidth: true }

                Button {
                  text: "↑"
                  enabled: provRow.modelData.index > 0
                  foreground: root.fg
                  fontFamily: root.fontFamily
                  fontSize: 11
                  horizontalPadding: 8
                  verticalPadding: 3
                  onClicked: root.moveProvider(provRow.modelData.id, -1)
                }
                Button {
                  text: "↓"
                  enabled: provRow.modelData.index >= 0 && provRow.modelData.index < provRow.modelData.count - 1
                  foreground: root.fg
                  fontFamily: root.fontFamily
                  fontSize: 11
                  horizontalPadding: 8
                  verticalPadding: 3
                  onClicked: root.moveProvider(provRow.modelData.id, 1)
                }
              }
            }
          }

          PanelSectionHeader {
            Layout.fillWidth: true
            text: "Display"
            foreground: root.fg
            fontFamily: root.fontFamily
            fontSize: 11
          }

          RowLayout {
            Layout.fillWidth: true
            spacing: 6

            Button {
              text: "Remaining"
              Layout.fillWidth: true
              foreground: root.fg
              accent: Color.accent
              fontFamily: root.fontFamily
              fontSize: 11
              horizontalPadding: 8
              verticalPadding: 5
              active: (root.draftSettings.displayMode || "remaining") !== "used"
              onClicked: root.setDisplayMode("remaining")
            }
            Button {
              text: "Used"
              Layout.fillWidth: true
              foreground: root.fg
              accent: Color.accent
              fontFamily: root.fontFamily
              fontSize: 11
              horizontalPadding: 8
              verticalPadding: 5
              active: root.draftSettings.displayMode === "used"
              onClicked: root.setDisplayMode("used")
            }
          }

          PanelSectionHeader {
            Layout.fillWidth: true
            text: "Alerts"
            foreground: root.fg
            fontFamily: root.fontFamily
            fontSize: 11
          }

          Toggle {
            Layout.fillWidth: true
            label: "Desktop notifications"
            description: checked ? "notify-send on quota thresholds" : "Notifications off"
            checked: !!root.draftSettings.notifyEnabled
            foreground: root.fg
            accent: Color.accent
            fontFamily: root.fontFamily
            onClicked: root.setNotifyEnabled(!checked)
          }

          PanelSectionHeader {
            Layout.fillWidth: true
            text: "Refresh"
            foreground: root.fg
            fontFamily: root.fontFamily
            fontSize: 11
          }

          NumberField {
            Layout.fillWidth: true
            label: "Interval (seconds)"
            value: Number(root.draftSettings.refreshIntervalSec || 60)
            from: 30
            to: 3600
            stepSize: 30
            fieldWidth: parent.width
            foreground: root.fg
            accent: Color.accent
            fontFamily: root.fontFamily
            onModified: function(value) { root.setRefreshInterval(value) }
          }

          Text {
            visible: root.settingsStatusText !== ""
            Layout.fillWidth: true
            text: root.settingsStatusText
            color: root.dim
            font.family: root.fontFamily
            font.pixelSize: 10
            horizontalAlignment: Text.AlignHCenter
            wrapMode: Text.WordWrap
          }

          Text {
            Layout.fillWidth: true
            text: "s saves · esc closes"
            color: root.dim
            font.family: root.fontFamily
            font.pixelSize: 10
            horizontalAlignment: Text.AlignHCenter
          }
        }

        // ── Usage footer ─────────────────────────────────────────────
        Text {
          visible: !root.settingsMode
          text: root.stale ? "stale — last fetch failed" : (root.payload ? "fetched " + root.fmtReset(root.payload.fetchedAt) : "loading…")
          color: root.dim
          font.family: root.fontFamily
          font.pixelSize: 10
        }

        Text {
          visible: !root.settingsMode
          text: "right-click: settings · middle: refresh"
          color: Qt.darker(root.fg, 1.6)
          font.family: root.fontFamily
          font.pixelSize: 10
        }

        Text {
          visible: !root.settingsMode
          text: "Abrir menu (TUI)"
          color: Color.accent
          font.family: root.fontFamily
          font.pixelSize: 11
          font.underline: true

          MouseArea {
            anchors.fill: parent
            cursorShape: Qt.PointingHandCursor
            onClicked: root.openTui()
          }
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
