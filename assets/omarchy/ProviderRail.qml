import QtQuick
import qs.Commons
import qs.Ui
import "ServiceCore.js" as Core

// Left rail: provider icons only; Settings glyph anchored at the bottom (UX-013..015).
Item {
  id: root

  property var providers: []
  property string selectedProviderId: ""
  property color foreground: Color.foreground
  property string fontFamily: Style.font.family
  property url iconBase: Qt.resolvedUrl("icons/")

  signal providerSelected(string providerId)
  signal settingsClicked()

  implicitWidth: Style.space(44)
  implicitHeight: railCol.implicitHeight

  function iconUrl(id) {
    var name = Core.iconFileName(id)
    if (!name.length)
      return ""
    return String(root.iconBase) + name
  }

  Column {
    id: railCol
    anchors.top: parent.top
    anchors.bottom: settingsBtn.top
    anchors.horizontalCenter: parent.horizontalCenter
    anchors.bottomMargin: Style.space(8)
    spacing: Style.space(6)
    width: parent.width

    Repeater {
      model: root.providers

      Item {
        id: railItem
        required property var modelData
        width: parent.width
        height: Style.space(32)

        readonly property string pid: modelData && modelData.id ? String(modelData.id) : ""
        readonly property bool selected: pid.length > 0 && pid === root.selectedProviderId
        readonly property bool dimmed: modelData ? Core.chipDimmed(modelData) : true

        Rectangle {
          anchors.centerIn: parent
          width: Style.space(28)
          height: Style.space(28)
          radius: Style.cornerRadius
          color: railItem.selected
              ? Qt.rgba(root.foreground.r, root.foreground.g, root.foreground.b, 0.12)
              : "transparent"
          border.width: railItem.selected ? 1 : 0
          border.color: Qt.rgba(root.foreground.r, root.foreground.g, root.foreground.b, 0.28)
        }

        Image {
          anchors.centerIn: parent
          source: root.iconUrl(railItem.pid)
          width: 16
          height: 16
          sourceSize.width: 16
          sourceSize.height: 16
          fillMode: Image.PreserveAspectFit
          opacity: railItem.dimmed ? 0.45 : 1.0
        }

        MouseArea {
          anchors.fill: parent
          hoverEnabled: true
          cursorShape: Qt.PointingHandCursor
          onClicked: root.providerSelected(railItem.pid)
          onEntered: {
            // Tooltip via Accessible; bar-level tooltips optional for rail.
          }
        }

        Accessible.name: modelData && modelData.name
            ? String(modelData.name)
            : Core.providerDisplayName(railItem.pid)
        Accessible.role: Accessible.Button
        Accessible.onPressAction: root.providerSelected(railItem.pid)
      }
    }
  }

  // UX-052 settings glyph at bottom of rail
  PanelActionButton {
    id: settingsBtn
    anchors.bottom: parent.bottom
    anchors.horizontalCenter: parent.horizontalCenter
    iconText: "󰒓"
    tooltipText: "Settings"
    foreground: root.foreground
    focusable: true
    Accessible.name: "Settings"
    onClicked: root.settingsClicked()
  }
}
