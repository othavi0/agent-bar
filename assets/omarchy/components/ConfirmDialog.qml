import QtQuick
import qs.Commons
import qs.Ui

// Modal confirmation overlay for update / uninstall (UX-043..047).
Item {
  id: root

  property bool opened: false
  property string title: ""
  property string message: ""
  property string cancelText: "Cancel"
  property string confirmText: "Confirm"
  property bool destructive: false
  property bool confirmEnabled: true
  property color foreground: Color.foreground
  property string fontFamily: Style.font.family

  // Optional extra content slot (e.g. purge checkbox row).
  default property alias extraContent: extraCol.data

  signal canceled()
  signal confirmed()

  // Full-screen wash behind the card. Not control chrome and not text, so it
  // carries no host token — declared once here with the one raw alpha this
  // file is allowed (see tst_Tokens.qml textAlphaExceptions).
  readonly property color scrimColor: Util.alpha(Color.foreground, 0.45)

  anchors.fill: parent
  visible: opened
  z: 100

  Rectangle {
    anchors.fill: parent
    color: root.scrimColor
    MouseArea {
      anchors.fill: parent
      onClicked: root.canceled()
    }
  }

  Rectangle {
    id: card
    anchors.centerIn: parent
    width: Math.min(parent.width - Style.space(32), Style.space(380))
    height: col.implicitHeight + Style.space(28)
    radius: Style.cornerRadius
    color: Color.popups.background
    border.width: 1
    // Destructive keeps its urgent-tinted border (UX-044: danger actions stay
    // visually separated); a Style state token would erase that signal, so
    // only the non-destructive branch moves to the shared token.
    border.color: root.destructive
        ? Util.alpha(Color.urgent, 0.55)
        : Style.normalBorderColor

    MouseArea {
      anchors.fill: parent
      onClicked: {} // swallow
    }

    Column {
      id: col
      anchors.left: parent.left
      anchors.right: parent.right
      anchors.top: parent.top
      anchors.margins: Style.space(14)
      spacing: Style.space(10)

      Text {
        width: parent.width
        visible: root.title.length > 0
        text: root.title
        color: root.foreground
        font.family: root.fontFamily
        font.pixelSize: Style.font.body
        font.bold: true
        wrapMode: Text.WordWrap
        textFormat: Text.PlainText
      }

      Text {
        width: parent.width
        text: root.message
        color: Util.alpha(root.foreground, 0.72)
        font.family: root.fontFamily
        font.pixelSize: Style.font.caption
        wrapMode: Text.WordWrap
        textFormat: Text.PlainText
      }

      Column {
        id: extraCol
        width: parent.width
        spacing: Style.space(6)
      }

      Row {
        anchors.right: parent.right
        spacing: Style.space(8)

        Button {
          text: root.cancelText
          bordered: true
          focusable: true
          foreground: root.foreground
          fontFamily: root.fontFamily
          onClicked: root.canceled()
        }

        Button {
          text: root.confirmText
          bordered: true
          focusable: true
          enabled: root.confirmEnabled
          foreground: root.destructive ? Color.urgent : root.foreground
          fontFamily: root.fontFamily
          Accessible.name: root.confirmText
          onClicked: root.confirmed()
        }
      }
    }
  }
}
