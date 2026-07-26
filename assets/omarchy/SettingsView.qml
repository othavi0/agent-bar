import QtQuick
import qs.Commons
import qs.Ui
import "ServiceCore.js" as Core
import "components"

// Race-safe Settings UI (SET-014..022, UX-033..039). Mutations go through Service.
Item {
  id: root

  property var agentService: null
  property color foreground: Color.foreground
  property string fontFamily: Style.font.family
  property url iconBase: Qt.resolvedUrl("icons/")

  readonly property var state: agentService ? agentService.settingsState : null
  readonly property var draft: agentService ? agentService.settingsDraft : null
  readonly property string phase: state && state.phase ? String(state.phase) : "closed"
  readonly property bool locked: agentService
      ? agentService.settingsLocked()
      : true
  readonly property bool canSave: agentService ? agentService.canSaveSettings() : false
  readonly property bool loading: phase === "loading"
  readonly property bool saving: phase === "saving"

  readonly property var providers: {
    if (!draft || !Array.isArray(draft.providers))
      return []
    return draft.providers
  }

  readonly property string metric: {
    if (draft && draft.display && draft.display.metric === "used")
      return "used"
    return "remaining"
  }

  readonly property int intervalSec: {
    if (draft && isFinite(Number(draft.refreshIntervalSeconds)))
      return Number(draft.refreshIntervalSeconds)
    return 60
  }

  readonly property bool notificationsOn: {
    if (draft && draft.notifications)
      return !!draft.notifications.enabled
    return true
  }

  width: parent ? parent.width : implicitWidth
  implicitHeight: col.implicitHeight

  function iconUrl(id) {
    var name = Core.iconFileName(id)
    if (!name.length)
      return ""
    return String(root.iconBase) + name
  }

  Column {
    id: col
    width: parent.width
    spacing: Style.space(12)

    Text {
      text: "Settings"
      color: root.foreground
      font.family: root.fontFamily
      font.pixelSize: Style.font.body
      font.bold: true
      textFormat: Text.PlainText
      Accessible.role: Accessible.Heading
    }

    Text {
      visible: root.loading
      width: parent.width
      text: "Loading settings\u2026"
      color: Qt.darker(root.foreground, 1.3)
      font.family: root.fontFamily
      font.pixelSize: Style.font.caption
      textFormat: Text.PlainText
    }

    Text {
      visible: root.saving
      width: parent.width
      text: "Saving\u2026"
      color: Qt.darker(root.foreground, 1.3)
      font.family: root.fontFamily
      font.pixelSize: Style.font.caption
      textFormat: Text.PlainText
    }

    // Providers
    Column {
      width: parent.width
      spacing: Style.space(4)
      opacity: root.locked ? 0.55 : 1.0
      enabled: !root.locked

      Text {
        text: "Providers"
        color: Qt.darker(root.foreground, 1.35)
        font.family: root.fontFamily
        font.pixelSize: Style.font.caption
        font.bold: true
        textFormat: Text.PlainText
      }

      Repeater {
        model: root.providers

        SettingsProviderRow {
          required property var modelData
          required property int index
          width: parent.width
          providerId: String(modelData.id || "")
          displayName: Core.providerDisplayName(providerId)
          iconSource: root.iconUrl(providerId)
          enabled: !!modelData.enabled
          locked: root.locked
          canMoveUp: index > 0
          canMoveDown: index < root.providers.length - 1
          foreground: root.foreground
          fontFamily: root.fontFamily
          onEnableToggled: {
            if (root.agentService)
              root.agentService.setProviderEnabled(providerId, !modelData.enabled)
          }
          onMoveUp: {
            if (root.agentService)
              root.agentService.moveProvider(providerId, -1)
          }
          onMoveDown: {
            if (root.agentService)
              root.agentService.moveProvider(providerId, 1)
          }
        }
      }
    }

    Rectangle {
      width: parent.width
      height: 1
      color: Qt.rgba(root.foreground.r, root.foreground.g, root.foreground.b, 0.12)
    }

    // Display metric
    Column {
      width: parent.width
      spacing: Style.space(6)
      opacity: root.locked ? 0.55 : 1.0
      enabled: !root.locked

      Text {
        text: "Chip number"
        color: Qt.darker(root.foreground, 1.35)
        font.family: root.fontFamily
        font.pixelSize: Style.font.caption
        font.bold: true
        textFormat: Text.PlainText
      }

      Row {
        spacing: Style.space(8)

        Button {
          text: "Remaining"
          selected: root.metric === "remaining"
          bordered: true
          focusable: true
          enabled: !root.locked
          foreground: root.foreground
          fontFamily: root.fontFamily
          onClicked: {
            if (root.agentService)
              root.agentService.setDisplayMetric("remaining")
          }
        }

        Button {
          text: "Used"
          selected: root.metric === "used"
          bordered: true
          focusable: true
          enabled: !root.locked
          foreground: root.foreground
          fontFamily: root.fontFamily
          onClicked: {
            if (root.agentService)
              root.agentService.setDisplayMetric("used")
          }
        }
      }
    }

    // Refresh interval — native NumberField (UX-035)
    Column {
      width: parent.width
      spacing: Style.space(4)
      opacity: root.locked ? 0.55 : 1.0
      enabled: !root.locked

      NumberField {
        label: "Refresh interval (seconds)"
        value: root.intervalSec
        from: 30
        to: 3600
        stepSize: 5
        foreground: root.foreground
        fontFamily: root.fontFamily
        onModified: function (v) {
          if (root.agentService)
            root.agentService.setRefreshInterval(v)
        }
      }
    }

    // Notifications
    Column {
      width: parent.width
      spacing: Style.space(4)
      opacity: root.locked ? 0.55 : 1.0
      enabled: !root.locked

      Toggle {
        label: "Notifications"
        description: "Usage threshold alerts for enabled providers"
        checked: root.notificationsOn
        foreground: root.foreground
        fontFamily: root.fontFamily
        onClicked: {
          if (root.agentService)
            root.agentService.setNotificationsEnabled(!root.notificationsOn)
        }
      }
    }

    Rectangle {
      width: parent.width
      height: 1
      color: Qt.rgba(root.foreground.r, root.foreground.g, root.foreground.b, 0.12)
    }

    // Actions — English text labels (UX-036..038)
    Flow {
      width: parent.width
      spacing: Style.space(8)

      Button {
        text: "Restore defaults"
        bordered: true
        focusable: true
        enabled: !root.locked
        foreground: root.foreground
        fontFamily: root.fontFamily
        Accessible.name: "Restore defaults"
        onClicked: {
          if (root.agentService)
            root.agentService.restoreSettingsDefaults()
        }
      }

      Button {
        text: "Cancel"
        bordered: true
        focusable: true
        enabled: !root.locked && root.phase === "dirty"
        foreground: root.foreground
        fontFamily: root.fontFamily
        Accessible.name: "Cancel"
        onClicked: {
          if (root.agentService)
            root.agentService.cancelSettings()
        }
      }

      Button {
        text: root.saving ? "Saving\u2026" : "Save changes"
        bordered: true
        focusable: true
        enabled: root.canSave
        foreground: root.foreground
        fontFamily: root.fontFamily
        Accessible.name: "Save changes"
        onClicked: {
          if (root.agentService)
            root.agentService.saveSettings()
        }
      }
    }

    // Maintenance section anchor — Task 13 fills content
    Column {
      width: parent.width
      spacing: Style.space(6)

      Rectangle {
        width: parent.width
        height: 1
        color: Qt.rgba(root.foreground.r, root.foreground.g, root.foreground.b, 0.12)
      }

      Text {
        text: "Maintenance"
        color: Qt.darker(root.foreground, 1.35)
        font.family: root.fontFamily
        font.pixelSize: Style.font.caption
        font.bold: true
        textFormat: Text.PlainText
      }

      Text {
        width: parent.width
        text: "Update and uninstall controls land in the next task."
        color: Qt.darker(root.foreground, 1.4)
        font.family: root.fontFamily
        font.pixelSize: Style.font.caption
        wrapMode: Text.WordWrap
        textFormat: Text.PlainText
      }
    }
  }
}
