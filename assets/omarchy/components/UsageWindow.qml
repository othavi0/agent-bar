import QtQuick
import qs.Commons

// One normalized percentage window row (label · percent · reset).
Item {
  id: root

  property string label: ""
  property string percentText: "\u2014"
  property string resetsAt: ""
  property color foreground: Color.foreground
  property string fontFamily: Style.font.family

  width: parent ? parent.width : implicitWidth
  implicitHeight: row.implicitHeight + Style.space(8)
  height: implicitHeight

  Row {
    id: row
    anchors.left: parent.left
    anchors.right: parent.right
    anchors.verticalCenter: parent.verticalCenter
    spacing: Style.space(10)

    Text {
      width: Math.max(Style.space(80), parent.width * 0.45)
      text: root.label
      color: root.foreground
      font.family: root.fontFamily
      font.pixelSize: Style.font.body
      elide: Text.ElideRight
      textFormat: Text.PlainText
      Accessible.name: root.label
    }

    Text {
      text: root.percentText
      color: root.foreground
      font.family: root.fontFamily
      font.pixelSize: Style.font.body
      font.bold: true
      textFormat: Text.PlainText
      Accessible.name: root.percentText
    }

    Text {
      visible: root.resetsAt.length > 0
      width: Math.max(0, parent.width * 0.35)
      text: root.resetsAt
      color: Qt.darker(root.foreground, 1.35)
      font.family: root.fontFamily
      font.pixelSize: Style.font.caption
      elide: Text.ElideRight
      textFormat: Text.PlainText
      Accessible.name: "resets " + root.resetsAt
    }
  }
}
