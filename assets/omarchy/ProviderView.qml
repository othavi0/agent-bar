import QtQuick
import qs.Commons
import "ServiceCore.js" as Core
import "components"

// Single selected-provider content pane (header + windows or state message).
Item {
  id: root

  property var provider: null
  property string displayMetric: "remaining"
  property bool refreshing: false
  property color foreground: Color.foreground
  property string fontFamily: Style.font.family

  signal refreshRequested(string providerId)
  signal actionRequested(string providerId, string kind, var target)

  readonly property var header: Core.headerModel(provider, refreshing)
  readonly property string mode: Core.contentMode(provider)
  readonly property var windowLines: Core.windowDisplayLines(provider, displayMetric)
  readonly property var actions: Core.stateActions(provider)

  width: parent ? parent.width : implicitWidth
  // Size to content only — stretching to parent.height caused a binding loop
  // and fought Popup content-fit height.
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
      connection: root.header.connection
      lastSuccessAt: root.header.lastSuccessAt ? root.header.lastSuccessAt : ""
      refreshing: root.header.refreshing
      showStale: root.header.showStale
      foreground: root.foreground
      fontFamily: root.fontFamily
      onRefreshClicked: {
        if (root.provider && root.provider.id)
          root.refreshRequested(String(root.provider.id))
      }
    }

    // Full-width separator (UX-019)
    Rectangle {
      width: parent.width
      height: 1
      color: Qt.rgba(root.foreground.r, root.foreground.g, root.foreground.b, 0.12)
    }

    // Ready / stale windows
    Column {
      width: parent.width
      spacing: Style.space(4)
      visible: root.mode === "windows" || root.mode === "stale_windows"

      Text {
        visible: root.mode === "stale_windows"
        width: parent.width
        text: "Stale"
        color: root.foreground
        font.family: root.fontFamily
        font.pixelSize: Style.font.caption
        font.bold: true
        textFormat: Text.PlainText
      }

      Text {
        visible: root.mode === "stale_windows" && Core.errorMessage(root.provider).length > 0
        width: parent.width
        text: Core.errorMessage(root.provider)
        color: Qt.darker(root.foreground, 1.25)
        font.family: root.fontFamily
        font.pixelSize: Style.font.caption
        wrapMode: Text.WordWrap
        textFormat: Text.PlainText
      }

      Repeater {
        model: root.windowLines
        UsageWindow {
          required property var modelData
          width: parent.width
          label: modelData.label
          percentText: modelData.percentText
          percent: modelData.percent !== undefined && modelData.percent !== null
              ? Number(modelData.percent)
              : -1
          resetsAt: modelData.resetsAt ? modelData.resetsAt : ""
          foreground: root.foreground
          fontFamily: root.fontFamily
        }
      }

      // Stale may still offer retry
      Flow {
        visible: root.mode === "stale_windows" && root.actions.length > 0
        width: parent.width
        spacing: Style.space(8)
        Repeater {
          model: root.actions
          // Lightweight text button without qs.Ui dependency for actions on stale windows
          Text {
            required property var modelData
            text: modelData.label
            color: root.foreground
            font.family: root.fontFamily
            font.pixelSize: Style.font.body
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
              root.actionRequested(
                String(root.provider.id),
                String(modelData.kind || ""),
                modelData.target
              )
            }
          }
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
  }
}
