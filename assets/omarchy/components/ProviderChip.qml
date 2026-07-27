import QtQuick

// Presentational bar chip + Quattro click-target protocol (UX-001..012).
// No I/O ownership or shell construction.
Item {
  id: root

  property var bar: null
  property string providerId: ""
  property string displayName: ""
  property string percentText: "\u2014"
  property string stateCue: ""
  property string tooltipText: ""
  property url iconSource: ""
  property color foreground: "#ffffff"
  property string fontFamily: "monospace"
  property int fontPixelSize: 12
  property bool vertical: false
  property int barSize: 28
  property bool dimmed: false

  signal pressed(int button)

  property var registeredBar: null
  readonly property bool tooltipHovered: mouseArea.containsMouse

  function triggerPress(button) {
    if (root.bar && typeof root.bar.hideTooltip === "function")
      root.bar.hideTooltip(root)
    root.pressed(button)
  }

  function syncClickRegistration() {
    if (registeredBar && typeof registeredBar.unregisterClickTarget === "function")
      registeredBar.unregisterClickTarget(root)
    registeredBar = root.bar
    if (registeredBar && typeof registeredBar.registerClickTarget === "function")
      registeredBar.registerClickTarget(root)
  }

  onBarChanged: syncClickRegistration()
  Component.onCompleted: syncClickRegistration()
  Component.onDestruction: {
    if (registeredBar && typeof registeredBar.unregisterClickTarget === "function")
      registeredBar.unregisterClickTarget(root)
  }

  implicitWidth: chipRow.implicitWidth
  implicitHeight: vertical ? Math.max(12, barSize - 6) : barSize
  width: implicitWidth
  height: implicitHeight

  Row {
    id: chipRow
    anchors.centerIn: parent
    spacing: 4
    opacity: root.dimmed ? 0.55 : 1.0

    Image {
      id: icon
      source: root.iconSource
      visible: source.toString().length > 0
      width: 13
      height: 13
      sourceSize.width: 13
      sourceSize.height: 13
      fillMode: Image.PreserveAspectFit
      anchors.verticalCenter: parent.verticalCenter
      opacity: root.dimmed ? 0.45 : 1.0
      Accessible.name: root.displayName
      Accessible.role: Accessible.Graphic
    }

    Text {
      id: label
      visible: !root.vertical
      text: root.percentText + root.stateCue
      color: root.foreground
      font.family: root.fontFamily
      font.pixelSize: root.fontPixelSize
      anchors.verticalCenter: parent.verticalCenter
      Accessible.name: root.tooltipText
      Accessible.role: Accessible.StaticText
    }
  }

  MouseArea {
    id: mouseArea
    anchors.fill: parent
    acceptedButtons: Qt.LeftButton | Qt.RightButton | Qt.MiddleButton
    hoverEnabled: true
    cursorShape: Qt.PointingHandCursor
    // UX-009: no wheel handler — wheel over a chip is a no-op.
    onEntered: {
      if (root.bar && typeof root.bar.showTooltip === "function")
        root.bar.showTooltip(root, root.tooltipText)
    }
    onExited: {
      if (root.bar && typeof root.bar.hideTooltip === "function")
        root.bar.hideTooltip(root)
    }
    onClicked: function (mouse) {
      root.triggerPress(mouse.button)
    }
  }
}
