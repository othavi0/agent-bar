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
      width: parent.width
      spacing: Style.space(8)

      Text {
        id: nameLabel
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
        text: root.connection
        color: root.showStale
            ? (root.foreground)
            : Qt.darker(root.foreground, 1.15)
        font.family: root.fontFamily
        font.pixelSize: Style.font.caption
        font.bold: root.showStale
        textFormat: Text.PlainText
        Accessible.name: root.connection
      }

      Item {
        // Push refresh control to the trailing edge when space allows.
        width: Math.max(Style.space(8), parent.width - nameLabel.implicitWidth - Style.space(120))
        height: 1
      }

      // UX-051 refresh glyph
      PanelActionButton {
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
