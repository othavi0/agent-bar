import QtQuick
import qs.Commons
import qs.Ui

// Provider non-window state body: title, plain-text body, allowlisted actions.
Column {
  id: root

  property string title: ""
  property string body: ""
  property var actions: [] // [{ kind, label, target }]
  property color foreground: Color.foreground
  property string fontFamily: Style.font.family
  property bool skeleton: false

  signal actionActivated(string kind, var target)

  width: parent ? parent.width : implicitWidth
  spacing: Style.space(10)

  // UX-026 skeleton placeholders (no plugin-authored motion).
  Column {
    visible: root.skeleton
    width: parent.width
    spacing: Style.space(8)

    Rectangle {
      width: parent.width * 0.55
      height: Style.space(14)
      radius: Style.cornerRadius
      color: Qt.rgba(root.foreground.r, root.foreground.g, root.foreground.b, 0.12)
    }
    Rectangle {
      width: parent.width * 0.9
      height: Style.space(12)
      radius: Style.cornerRadius
      color: Qt.rgba(root.foreground.r, root.foreground.g, root.foreground.b, 0.08)
    }
    Rectangle {
      width: parent.width * 0.7
      height: Style.space(12)
      radius: Style.cornerRadius
      color: Qt.rgba(root.foreground.r, root.foreground.g, root.foreground.b, 0.08)
    }
  }

  Column {
    visible: !root.skeleton
    width: parent.width
    spacing: Style.space(8)

    Text {
      // Bind to available width; avoid implicit-width grow that clips left glyphs.
      width: Math.max(0, parent.width)
      text: root.title
      color: root.foreground
      font.family: root.fontFamily
      font.pixelSize: Style.font.body
      font.bold: true
      wrapMode: Text.Wrap
      horizontalAlignment: Text.AlignLeft
      textFormat: Text.PlainText
      Accessible.name: root.title
      Accessible.role: Accessible.Heading
    }

    Text {
      width: Math.max(0, parent.width)
      visible: root.body.length > 0
      text: root.body
      color: Util.alpha(root.foreground, 0.72)
      font.family: root.fontFamily
      font.pixelSize: Style.font.caption
      wrapMode: Text.Wrap
      horizontalAlignment: Text.AlignLeft
      textFormat: Text.PlainText
      Accessible.name: root.body
      Accessible.role: Accessible.StaticText
    }

    Flow {
      width: Math.max(0, parent.width)
      spacing: Style.space(8)

      Repeater {
        model: root.actions

        Button {
          required property var modelData
          text: modelData && modelData.label ? String(modelData.label) : ""
          foreground: root.foreground
          fontFamily: root.fontFamily
          bordered: true
          focusable: true
          // Keep action labels fully inside content (no left clip).
          leftAlign: true
          Accessible.name: text
          onClicked: root.actionActivated(
            modelData && modelData.kind ? String(modelData.kind) : "",
            modelData ? modelData.target : null
          )
        }
      }
    }
  }
}
