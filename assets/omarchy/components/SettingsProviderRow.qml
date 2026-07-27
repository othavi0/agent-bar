import QtQuick
import qs.Commons
import qs.Ui

// One provider row: icon, English name, enable toggle, up/down order.
Item {
  id: root

  property string providerId: ""
  property string displayName: ""
  property url iconSource: ""
  property bool enabled: true
  property bool locked: false
  property bool canMoveUp: true
  property bool canMoveDown: true
  property color foreground: Color.foreground
  property string fontFamily: Style.font.family

  signal enableToggled()
  signal moveUp()
  signal moveDown()

  width: parent ? parent.width : implicitWidth
  implicitHeight: Style.space(40)
  height: implicitHeight

  Row {
    anchors.fill: parent
    spacing: Style.space(8)

    Image {
      anchors.verticalCenter: parent.verticalCenter
      source: root.iconSource
      width: 16
      height: 16
      sourceSize.width: 16
      sourceSize.height: 16
      fillMode: Image.PreserveAspectFit
      opacity: root.enabled ? 1.0 : 0.45
    }

    Text {
      anchors.verticalCenter: parent.verticalCenter
      width: Math.max(Style.space(80), parent.width * 0.35)
      text: root.displayName
      color: root.foreground
      font.family: root.fontFamily
      font.pixelSize: Style.font.body
      elide: Text.ElideRight
      textFormat: Text.PlainText
      Accessible.name: root.displayName
    }

    Item { width: Style.space(8); height: 1 }

    // Native toggle via qs.Ui.Toggle would be tall; use compact ButtonGroup-like text switch.
    Button {
      anchors.verticalCenter: parent.verticalCenter
      text: root.enabled ? "On" : "Off"
      selected: root.enabled
      bordered: true
      focusable: true
      enabled: !root.locked
      foreground: root.foreground
      fontFamily: root.fontFamily
      Accessible.name: root.displayName + " " + (root.enabled ? "enabled" : "disabled")
      onClicked: {
        if (!root.locked)
          root.enableToggled()
      }
    }

    Item {
      width: Math.max(Style.space(4), parent.width * 0.1)
      height: 1
    }

    // UX-034 native chevrons (Quattro dropdown uses 󰅀 down; 󰅃 up)
    PanelActionButton {
      anchors.verticalCenter: parent.verticalCenter
      iconText: "󰅃"
      tooltipText: "Move up"
      foreground: root.foreground
      enabled: !root.locked && root.canMoveUp
      focusable: true
      Accessible.name: "Move " + root.displayName + " up"
      onClicked: root.moveUp()
    }

    PanelActionButton {
      anchors.verticalCenter: parent.verticalCenter
      iconText: "󰅀"
      tooltipText: "Move down"
      foreground: root.foreground
      enabled: !root.locked && root.canMoveDown
      focusable: true
      Accessible.name: "Move " + root.displayName + " down"
      onClicked: root.moveDown()
    }
  }
}
