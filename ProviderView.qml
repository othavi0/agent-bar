import QtQuick
import qs.Commons
import qs.Ui
import "CoreView.js" as Core
import "components"

// Single selected-provider content pane, visual design §3.4/§8:
// header -> [age line] -> lead window (large) -> compact rows
// -> state message (non-window modes). Plan 03 removed the meta footer;
// last-success age now lives in one neutral caption shown while the reading
// is retained (UX-028 amended). Staleness changes no colour, no opacity, and
// no layout — only that caption's presence.
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
  // UX-028 (amended): staleness is reported as a fact, never as a fault. The
  // pane states when the reading was taken and stops there — no glyph, no
  // urgent colour, no error text, no Retry (the header's own refresh control
  // already covers the action).
  readonly property bool showsAge: String(root.provider && root.provider.state || "") === "stale"
  readonly property string ageText: {
    var age = Core.formatAgoText(
      root.header.lastSuccessAt ? root.header.lastSuccessAt : "", root.nowMs)
    return age.length ? "Updated " + age : ""
  }
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

    // Age line (UX-028 amended): one caption in the pane's own foreground at
    // the same 0.72 alpha the header tags use. Plain statement of when the
    // reading was taken — the eye reads it as metadata, not as an alert, and
    // assistive tech hears exactly the same words (A11Y-012).
    Text {
      visible: root.showsAge && root.ageText.length > 0
      width: parent.width
      text: root.ageText
      color: Util.alpha(root.foreground, 0.72)
      font.family: root.fontFamily
      font.pixelSize: Style.font.caption
      wrapMode: Text.WordWrap
      textFormat: Text.PlainText
      Accessible.name: text
    }

    // §3.4/§8: one elected lead window rendered large, every other window as
    // a compact row in delivered order. No rule between them — the size
    // difference is the hierarchy (approved mockup).
    Column {
      width: parent.width
      spacing: Style.spacing.xxxl
      visible: root.mode === "windows"

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
      // No stale exception is needed: contentMode routes stale through the
      // ready branch, so it lands on "windows"/"empty_windows" like any other
      // provider holding a reading.
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
  }
}
