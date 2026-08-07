import QtQuick
import qs.Commons
import qs.Ui
import "CoreView.js" as Core
import "components"

// Single selected-provider content pane, visual design §3.4/§8:
// header -> [stale banner] -> lead window (large) -> compact rows
// -> state message (non-window, non-stale modes). Plan 03 removed the meta
// footer; last-success age now lives in the stale banner and is otherwise
// implied structurally (windows render only when ready).
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
  readonly property var windows: Core.windowLayout(provider, displayMetric, nowMs)
  readonly property string severity: Core.providerSeverity(provider)
  readonly property var actions: Core.stateActions(provider)
  // Adjusted (plan 03): was mode === "stale_windows" only, which fed the
  // banner's Retry Repeater and left the no-windows stale mode ("state")
  // without a Retry control. Both stale modes share one provider.state
  // check; dimmed: root.isStale on the windows Column is unaffected since
  // that Column is only visible in the "windows"/"stale_windows" modes,
  // where state === "stale" iff mode === "stale_windows" anyway.
  readonly property bool isStale: String(root.provider && root.provider.state || "") === "stale"
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
      severityText: Core.severityTagText(root.severity)
      severityUrgent: root.severity === "critical"
      foreground: root.foreground
      fontFamily: root.fontFamily
      onRefreshClicked: {
        if (root.provider && root.provider.id)
          root.refreshRequested(String(root.provider.id))
      }
    }

    PanelSeparator {
      width: parent.width
      foreground: root.foreground
    }

    // Stale banner (UX-028): carries last-success age + safe error + Retry.
    // Never color-only (A11Y-012): glyph and words, urgent-tinted per the
    // approved mockup.
    Row {
      visible: root.isStale
      width: parent.width
      spacing: Style.space(8)

      Text {
        text: "󰅐"
        color: Color.urgent
        font.family: root.fontFamily
        font.pixelSize: Style.font.body
        textFormat: Text.PlainText
        Accessible.ignored: true
      }
      Text {
        width: Math.max(0, parent.width - Style.space(120))
        text: {
          var age = Core.formatAgoText(
            root.header.lastSuccessAt ? root.header.lastSuccessAt : "", root.nowMs)
          var line = "Last data " + (age.length ? age : "unknown")
          var err = Core.errorMessage(root.provider)
          if (err.length)
            line += " · " + err
          return line
        }
        color: Color.urgent
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

    // §3.4/§8: one elected lead window rendered large, every other window as
    // a compact row in delivered order. No rule between them — the size
    // difference is the hierarchy (approved mockup).
    Column {
      width: parent.width
      spacing: Style.spacing.xxxl
      visible: root.mode === "windows" || root.mode === "stale_windows"

      Repeater {
        model: root.windows.lead ? [root.windows.lead] : []
        UsageWindow {
          required property var modelData
          width: parent.width
          label: modelData.label
          percentText: modelData.percentText
          percent: modelData.percent !== undefined && modelData.percent !== null
              ? Number(modelData.percent) : -1
          resetCountdown: modelData.resetCountdown ? modelData.resetCountdown : ""
          resetClock: modelData.resetsAt && modelData.resetCountdown
              && modelData.resetCountdown !== "now"
              ? Core.resetClockText(String(modelData.resetsAt),
                                    Qt.locale().timeFormat(Locale.ShortFormat))
              : ""
          resetPhrase: modelData.resetPhrase ? modelData.resetPhrase : ""
          severity: modelData.severity ? modelData.severity : ""
          unitText: root.unitText
          emphasis: true
          dimmed: root.isStale
          foreground: root.foreground
          accent: root.accentColor
          fontFamily: root.fontFamily
        }
      }

      Column {
        width: parent.width
        spacing: Style.spacing.xl
        visible: root.windows.rest.length > 0

        Repeater {
          model: root.windows.rest
          UsageWindow {
            required property var modelData
            width: parent.width
            label: modelData.label
            percentText: modelData.percentText
            percent: modelData.percent !== undefined && modelData.percent !== null
                ? Number(modelData.percent) : -1
            resetCountdown: modelData.resetCountdown ? modelData.resetCountdown : ""
            resetPhrase: modelData.resetPhrase ? modelData.resetPhrase : ""
            severity: modelData.severity ? modelData.severity : ""
            unitText: root.unitText
            emphasis: false
            dimmed: root.isStale
            foreground: root.foreground
            accent: root.accentColor
            fontFamily: root.fontFamily
          }
        }
      }

      Text {
        width: parent.width
        visible: text.length > 0
        text: Core.rateLimitResetsText(root.provider)
        color: Util.alpha(root.foreground, 0.72)
        font.family: root.fontFamily
        font.pixelSize: Style.font.caption
        textFormat: Text.PlainText
        Accessible.name: text
      }
    }

    StateMessage {
      width: parent.width
      visible: (root.mode === "skeleton" || root.mode === "empty_windows" || root.mode === "state")
          && String(root.provider && root.provider.state || "") !== "stale"
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
  }
}
