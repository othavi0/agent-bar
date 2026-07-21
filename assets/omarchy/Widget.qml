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

  // Chamado pelo PopupCard no clique-fora (owner.close()). Sem isto o card
  // faz `open = false` imperativo, o que DESTRÓI o binding open:popupOpen —
  // o popup nunca mais abre. Resetar a fonte da verdade preserva o binding.
  function close() {
    popupOpen = false
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
