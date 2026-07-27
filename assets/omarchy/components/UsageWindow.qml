import QtQuick
import qs.Commons

// One normalized percentage window: label, percent, optional reset, and a
// glanceable track (Operate: density + real data, no decorative chrome).
Item {
  id: root

  property string label: ""
  property string percentText: "\u2014"
  // 0–100 when known; negative when unavailable (hide fill).
  property real percent: -1
  property string resetsAt: ""
  property color foreground: Color.foreground
  property string fontFamily: Style.font.family

  readonly property bool hasPercent: root.percent >= 0 && root.percent <= 100
  readonly property real fillRatio: hasPercent
      ? Math.max(0, Math.min(1, root.percent / 100))
      : 0

  width: parent ? parent.width : implicitWidth
  implicitHeight: col.implicitHeight
  height: implicitHeight

  Column {
    id: col
    width: parent.width
    spacing: Style.space(6)

    Row {
      width: parent.width
      spacing: Style.space(10)

      Text {
        id: labelText
        width: Math.max(Style.space(72), parent.width * 0.38)
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

      Item {
        width: Math.max(0, parent.width - labelText.width - Style.space(100))
        height: 1
      }

      Text {
        visible: root.resetsAt.length > 0
        width: Math.min(implicitWidth, parent.width * 0.32)
        anchors.verticalCenter: parent.verticalCenter
        text: root.resetsAt
        color: Qt.darker(root.foreground, 1.35)
        font.family: root.fontFamily
        font.pixelSize: Style.font.caption
        elide: Text.ElideRight
        horizontalAlignment: Text.AlignRight
        textFormat: Text.PlainText
        Accessible.name: "resets " + root.resetsAt
      }
    }

    // Track always present for ready windows so Amp/Grok/etc. match at a glance.
    Rectangle {
      id: track
      width: parent.width
      height: Style.space(6)
      radius: height / 2
      color: Qt.rgba(root.foreground.r, root.foreground.g, root.foreground.b, 0.12)
      Accessible.ignored: true

      Rectangle {
        anchors.left: parent.left
        anchors.verticalCenter: parent.verticalCenter
        width: Math.max(root.hasPercent && root.fillRatio > 0 ? Style.space(6) : 0,
                        parent.width * root.fillRatio)
        height: parent.height
        radius: parent.radius
        color: root.foreground
        opacity: root.hasPercent ? 0.85 : 0
        visible: root.hasPercent && root.fillRatio > 0
      }
    }
  }

  Accessible.name: {
    var parts = [root.label, root.percentText]
    if (root.resetsAt.length)
      parts.push("resets " + root.resetsAt)
    return parts.join(", ")
  }
  Accessible.role: Accessible.StaticText
}
