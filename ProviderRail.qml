import QtQuick
import QtQuick.Layouts
import qs.Commons
import qs.Ui
import "CoreView.js" as Core

// Left rail — stack: providers → spacer → Settings.
// No own frame (§6): the card border already bounds the popup, so the slot
// stack shares the popup's inset token and fits the rail width exactly.
// Selected = brainstorming “A” plate (soft fill + thin border), only on the
// active provider id — never a sticky focus ring on Claude.
Item {
  id: root

  property var providers: []
  property string selectedProviderId: ""
  property color foreground: Color.foreground
  property string fontFamily: Style.font.family
  property url iconBase: Qt.resolvedUrl("icons/")

  signal providerSelected(string providerId)
  signal settingsClicked()

  readonly property int slotSize: Style.space(32)
  readonly property int iconSize: 16
  readonly property int stackGap: Style.space(8)
  readonly property int spacerMin: Style.space(4)

  // No own frame (§6) — the slot fits exactly, horizontal inset is 0.
  readonly property int railWidth: slotSize

  readonly property int minStackHeight: {
    var n = providers && providers.length ? providers.length : 0
    var slots = n + 1
    var gaps = n + 1 // n providers + spacer + settings → n+2 items → n+1 gaps
    return Style.spacing.popupPadding * 2
        + slots * slotSize
        + gaps * stackGap
        + spacerMin
  }

  implicitWidth: railWidth
  width: railWidth
  implicitHeight: minStackHeight

  property var _railFocusItems: []

  function iconUrl(id) {
    var name = Core.iconFileName(id)
    if (!name.length)
      return ""
    return String(root.iconBase) + name
  }

  function collectFocusTargets() {
    var out = []
    for (var i = 0; i < _railFocusItems.length; i++) {
      if (_railFocusItems[i])
        out.push(_railFocusItems[i])
    }
    if (settingsItem)
      out.push(settingsItem)
    return out
  }

  function registerRailItem(item) {
    var next = _railFocusItems.slice()
    if (next.indexOf(item) < 0)
      next.push(item)
    _railFocusItems = next
  }

  function unregisterRailItem(item) {
    var next = []
    for (var i = 0; i < _railFocusItems.length; i++) {
      if (_railFocusItems[i] !== item)
        next.push(_railFocusItems[i])
    }
    _railFocusItems = next
  }

  ColumnLayout {
    id: stack
    anchors.fill: parent
    anchors.topMargin: Style.spacing.popupPadding
    anchors.bottomMargin: Style.spacing.popupPadding
    spacing: root.stackGap

    Repeater {
      model: root.providers

      Item {
        id: railItem
        // Prefer index lookup — more reliable than modelData.id for JS arrays.
        required property int index
        required property var modelData

        Layout.fillWidth: true
        Layout.preferredHeight: root.slotSize
        Layout.maximumHeight: root.slotSize
        Layout.alignment: Qt.AlignHCenter
        activeFocusOnTab: true
        enabled: true
        clip: true

        readonly property var entry: {
          if (root.providers && index >= 0 && index < root.providers.length)
            return root.providers[index]
          return modelData
        }
        readonly property string pid: entry && entry.id ? String(entry.id) : ""
        readonly property bool selected: {
          var sel = String(root.selectedProviderId || "").trim()
          var id = railItem.pid
          return id.length > 0 && sel.length > 0 && id === sel
        }
        readonly property bool dimmed: entry ? Core.chipDimmed(entry) : true

        function focusActivate() {
          root.providerSelected(railItem.pid)
        }

        Component.onCompleted: root.registerRailItem(railItem)
        Component.onDestruction: root.unregisterRailItem(railItem)

        // Active provider: soft fill + neutral border only (no accent tick).
        Rectangle {
          anchors.fill: parent
          radius: Style.cornerRadius
          color: railItem.selected
              ? Style.selectedFill
              : "transparent"
          border.width: railItem.selected ? Style.selectedBorderWidth : 0
          border.color: railItem.selected
              ? Style.selectedBorderColor
              : "transparent"
        }

        Image {
          anchors.centerIn: parent
          source: root.iconUrl(railItem.pid)
          width: root.iconSize
          height: root.iconSize
          sourceSize.width: root.iconSize
          sourceSize.height: root.iconSize
          fillMode: Image.PreserveAspectFit
          opacity: railItem.dimmed ? 0.4 : (railItem.selected ? 1.0 : 0.65)
        }

        MouseArea {
          anchors.fill: parent
          hoverEnabled: true
          cursorShape: Qt.PointingHandCursor
          onClicked: root.providerSelected(railItem.pid)
        }

        Keys.onReturnPressed: railItem.focusActivate()
        Keys.onEnterPressed: railItem.focusActivate()
        Keys.onSpacePressed: railItem.focusActivate()

        Accessible.name: entry && entry.name
            ? String(entry.name)
            : Core.providerDisplayName(railItem.pid)
        Accessible.role: Accessible.Button
        Accessible.onPressAction: railItem.focusActivate()
      }
    }

    Item {
      Layout.fillWidth: true
      Layout.fillHeight: true
      Layout.minimumHeight: root.spacerMin
      Layout.preferredWidth: 1
    }

    Item {
      id: settingsItem
      Layout.fillWidth: true
      Layout.preferredHeight: root.slotSize
      Layout.maximumHeight: root.slotSize
      Layout.alignment: Qt.AlignHCenter
      activeFocusOnTab: true
      clip: true

      function focusActivate() {
        root.settingsClicked()
      }

      Rectangle {
        anchors.fill: parent
        radius: Style.cornerRadius
        color: (settingsItem.activeFocus || settingsMouse.containsMouse)
            ? Style.hoverFill
            : "transparent"
        border.width: settingsItem.activeFocus ? 1 : 0
        border.color: Color.accent
      }

      Text {
        anchors.centerIn: parent
        text: "󰒓"
        color: root.foreground
        opacity: 0.85
        font.family: root.fontFamily
        font.pixelSize: Style.font.icon
      }

      MouseArea {
        id: settingsMouse
        anchors.fill: parent
        hoverEnabled: true
        cursorShape: Qt.PointingHandCursor
        onClicked: root.settingsClicked()
      }

      Keys.onReturnPressed: settingsItem.focusActivate()
      Keys.onEnterPressed: settingsItem.focusActivate()
      Keys.onSpacePressed: settingsItem.focusActivate()

      Accessible.name: "Settings"
      Accessible.role: Accessible.Button
      Accessible.onPressAction: settingsItem.focusActivate()

      PanelToolTip {
        visible: settingsMouse.containsMouse
        text: "Settings"
        fontFamily: root.fontFamily
      }
    }
  }
}
