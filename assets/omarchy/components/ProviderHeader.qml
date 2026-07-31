import QtQuick
import qs.Commons
import qs.Ui

// Provider content header — name, plan pill, refresh only (Fase 2 slim-down).
// Connection state is implied structurally (UX-017); last-success age
// appears in the stale banner, not here.
// UX-016: deliberately no provider icon here (icon lives only on the rail).
Item {
  id: root

  property string name: ""
  property string plan: ""
  property bool refreshing: false
  property bool showStale: false
  property color foreground: Color.foreground
  property string fontFamily: Style.font.family

  signal refreshClicked()

  width: parent ? parent.width : implicitWidth
  implicitHeight: row.implicitHeight
  height: implicitHeight

  Row {
    id: row
    width: Math.max(0, parent.width)
    spacing: Style.space(8)

    Text {
      id: nameLabel
      // Cap name so the row never forces content wider than the pane.
      width: Math.min(implicitWidth, Math.max(Style.space(48), parent.width * 0.42))
      text: root.name
      color: root.foreground
      font.family: root.fontFamily
      font.pixelSize: Style.font.body
      font.bold: true
      elide: Text.ElideRight
      textFormat: Text.PlainText
      Accessible.name: root.name
      Accessible.role: Accessible.Heading
    }

    Rectangle {
      visible: root.plan.length > 0
      anchors.verticalCenter: parent.verticalCenter
      width: planText.implicitWidth + Style.space(10)
      height: planText.implicitHeight + Style.space(4)
      radius: Style.cornerRadius
      color: "transparent"
      border.width: 1
      border.color: Style.normalBorderColor
      Text {
        id: planText
        anchors.centerIn: parent
        text: root.plan
        color: Util.alpha(root.foreground, 0.72)
        font.family: root.fontFamily
        font.pixelSize: Style.font.caption
        font.capitalization: Font.AllUppercase
        font.letterSpacing: 0.5
        textFormat: Text.PlainText
        Accessible.name: "plan " + root.plan
      }
    }

    Item {
      // Flexible spacer; never negative. Pushes the refresh glyph right.
      width: Math.max(Style.space(4),
          parent.width
          - nameLabel.width
          - Style.space(60))
      height: 1
    }

    // UX-051 refresh glyph
    PanelActionButton {
      size: Style.space(22)
      iconText: "󰑐"
      tooltipText: root.refreshing ? "Refreshing…" : "Refresh provider"
      foreground: root.foreground
      enabled: !root.refreshing
      focusable: true
      Accessible.name: "Refresh provider"
      onClicked: root.refreshClicked()
    }
  }
}
