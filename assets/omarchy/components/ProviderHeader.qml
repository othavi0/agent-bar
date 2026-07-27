import QtQuick
import qs.Commons
import qs.Ui

// Provider content header — name, plan, connection, age, refresh.
// UX-016: deliberately no provider icon here (icon lives only on the rail).
Item {
  id: root

  property string name: ""
  property string plan: ""
  property string connection: ""
  property string lastSuccessAt: ""
  property bool refreshing: false
  property bool showStale: false
  property color foreground: Color.foreground
  property string fontFamily: Style.font.family

  signal refreshClicked()

  width: parent ? parent.width : implicitWidth
  implicitHeight: col.implicitHeight
  height: implicitHeight

  Column {
    id: col
    width: parent.width
    spacing: Style.space(4)

    Row {
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

      Text {
        visible: root.plan.length > 0
        text: root.plan
        color: Qt.darker(root.foreground, 1.2)
        font.family: root.fontFamily
        font.pixelSize: Style.font.caption
        textFormat: Text.PlainText
        Accessible.name: "plan " + root.plan
      }

      Text {
        text: "\u00b7"
        color: Qt.darker(root.foreground, 1.4)
        font.family: root.fontFamily
        font.pixelSize: Style.font.caption
        textFormat: Text.PlainText
      }

      Text {
        // Remaining header text elides instead of shoving layout left/right.
        width: Math.min(implicitWidth, Math.max(Style.space(40), parent.width * 0.28))
        text: root.connection
        color: root.showStale
            ? (root.foreground)
            : Qt.darker(root.foreground, 1.15)
        font.family: root.fontFamily
        font.pixelSize: Style.font.caption
        font.bold: root.showStale
        elide: Text.ElideRight
        textFormat: Text.PlainText
        Accessible.name: root.connection
      }

      Item {
        // Flexible spacer; never negative.
        width: Math.max(Style.space(4),
            parent.width
            - nameLabel.width
            - Style.space(100))
        height: 1
      }

      // UX-051 refresh glyph
      PanelActionButton {
        size: Style.space(22)
        iconText: "󰑐"
        tooltipText: root.refreshing ? "Refreshing\u2026" : "Refresh provider"
        foreground: root.foreground
        enabled: !root.refreshing
        focusable: true
        Accessible.name: "Refresh provider"
        onClicked: root.refreshClicked()
      }
    }

    Text {
      visible: root.lastSuccessAt.length > 0
      width: parent.width
      text: "Updated " + root.lastSuccessAt
          + (root.refreshing ? " \u00b7 refreshing" : "")
      color: Qt.darker(root.foreground, 1.4)
      font.family: root.fontFamily
      font.pixelSize: Style.font.caption
      elide: Text.ElideRight
      textFormat: Text.PlainText
      Accessible.name: text
    }
  }
}
