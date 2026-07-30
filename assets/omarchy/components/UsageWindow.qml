import QtQuick
import qs.Commons

// One normalized percentage window, "Camadas" hierarchy (Fase 2):
// kicker label -> big numeral + unit -> accent track -> humanized reset line.
Item {
  id: root

  property string label: ""
  property string percentText: "—"
  // 0–100 when known; negative when unavailable (hide fill).
  property real percent: -1
  property string resetText: ""
  property string unitText: "left"
  // Primary windows render large; secondary (per-model) render compact.
  property bool emphasis: true
  property bool dimmed: false
  property color foreground: Color.foreground
  property color accent: Color.accent
  property string fontFamily: Style.font.family

  readonly property bool hasPercent: root.percent >= 0 && root.percent <= 100
  readonly property real fillRatio: hasPercent
      ? Math.max(0, Math.min(1, root.percent / 100))
      : 0

  width: parent ? parent.width : implicitWidth
  implicitHeight: root.emphasis ? bigCol.implicitHeight : compactRow.implicitHeight
  height: implicitHeight
  opacity: root.dimmed ? 0.6 : 1.0

  Column {
    id: bigCol
    visible: root.emphasis
    width: parent.width
    spacing: Style.space(4)

    Text {
      width: parent.width
      text: root.label
      color: Util.alpha(root.foreground, 0.72)
      font.family: root.fontFamily
      font.pixelSize: Style.font.caption
      font.capitalization: Font.AllUppercase
      font.letterSpacing: 1
      elide: Text.ElideRight
      textFormat: Text.PlainText
      Accessible.ignored: true
    }

    Row {
      spacing: Style.space(6)
      Text {
        text: root.percentText
        color: root.foreground
        font.family: root.fontFamily
        font.pixelSize: Math.round(Style.font.body * 1.8)
        font.bold: true
        textFormat: Text.PlainText
        Accessible.ignored: true
      }
      Text {
        anchors.baseline: parent.children[0].baseline
        text: root.unitText
        color: Util.alpha(root.foreground, 0.72)
        font.family: root.fontFamily
        font.pixelSize: Style.font.caption
        textFormat: Text.PlainText
        Accessible.ignored: true
      }
    }

    Rectangle {
      width: parent.width
      height: Style.space(5)
      radius: height / 2
      color: Qt.rgba(root.foreground.r, root.foreground.g, root.foreground.b, 0.12)
      Accessible.ignored: true

      Rectangle {
        anchors.left: parent.left
        anchors.verticalCenter: parent.verticalCenter
        width: Math.max(root.hasPercent && root.fillRatio > 0 ? Style.space(5) : 0,
                        parent.width * root.fillRatio)
        height: parent.height
        radius: parent.radius
        color: root.dimmed ? root.foreground : root.accent
        opacity: root.dimmed ? 0.45 : 0.9
        visible: root.hasPercent && root.fillRatio > 0
      }
    }

    Row {
      visible: root.resetText.length > 0
      spacing: Style.space(4)
      Text {
        text: "resets"
        color: Util.alpha(root.foreground, 0.72)
        font.family: root.fontFamily
        font.pixelSize: Style.font.caption
        textFormat: Text.PlainText
        Accessible.ignored: true
      }
      Text {
        text: root.resetText
        color: root.foreground
        font.family: root.fontFamily
        font.pixelSize: Style.font.caption
        font.bold: true
        textFormat: Text.PlainText
        Accessible.ignored: true
      }
    }
  }

  Row {
    id: compactRow
    visible: !root.emphasis
    width: parent.width
    spacing: Style.space(8)

    Text {
      id: compactLabel
      width: Math.max(0, parent.width * 0.5)
      text: root.label
      color: Util.alpha(root.foreground, 0.72)
      font.family: root.fontFamily
      font.pixelSize: Style.font.caption
      elide: Text.ElideRight
      textFormat: Text.PlainText
      Accessible.ignored: true
    }
    Text {
      width: Math.max(0, parent.width - compactLabel.width - Style.space(8))
      text: root.percentText + " " + root.unitText
      color: root.foreground
      font.family: root.fontFamily
      font.pixelSize: Style.font.caption
      font.bold: true
      elide: Text.ElideRight
      horizontalAlignment: Text.AlignRight
      textFormat: Text.PlainText
      Accessible.ignored: true
    }
  }

  Accessible.name: {
    var parts = [root.label, root.percentText + " " + root.unitText]
    if (root.resetText.length)
      parts.push("resets " + root.resetText)
    return parts.join(", ")
  }
  Accessible.role: Accessible.StaticText
}
