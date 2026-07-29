import QtQuick
import qs.Commons
import "CoreView.js" as Core
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
  // Popup open state (Popup.qml's contentLoader keeps this instance alive
  // across close/open, so a child `visible` prop never gates the tick —
  // the owner must drive this explicitly).
  property bool active: true

  signal refreshRequested(string providerId)
  signal actionRequested(string providerId, string kind, var target)

  // Re-humanize countdowns while the popup stays open.
  property double nowMs: Date.now()
  // Test hook: expose whether the tick is actually running.
  property alias nowTickRunning: nowTimer.running
  onActiveChanged: if (active) nowMs = Date.now()
  Timer {
    id: nowTimer
    interval: 30000
    running: root.active
    repeat: true
    onTriggered: root.nowMs = Date.now()
  }

  readonly property var header: Core.headerModel(provider, refreshing)
  readonly property string mode: Core.contentMode(provider)
  readonly property var groups: Core.windowGroups(provider, displayMetric, nowMs)
  readonly property var actions: Core.stateActions(provider)
  readonly property bool isStale: root.mode === "stale_windows"
  readonly property string unitText: root.displayMetric === "used" ? "used" : "left"
  readonly property color accentColor: Color.accent

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
        text: "⌛"
        color: root.foreground
        font.family: root.fontFamily
        font.pixelSize: Style.font.body
        textFormat: Text.PlainText
        Accessible.ignored: true
      }
      Text {
        width: Math.max(0, parent.width - Style.space(120))
        text: "Stale — " + Core.errorMessage(root.provider)
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
            parts.push("refreshing…")
          return parts.join(" · ")
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
