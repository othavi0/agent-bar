import QtQuick
import qs.Commons
import qs.Ui
import "CoreView.js" as Core
import "components"

// Race-safe Settings UI (SET-014..022, UX-033..039). Mutations go through Service.
Item {
  id: root

  property var agentService: null
  property color foreground: Color.foreground
  property string fontFamily: Style.font.family
  property url iconBase: Qt.resolvedUrl("icons/")
  // A11Y-008: true while NumberField (or other editor) owns focus.
  property bool editorOwnsFocus: intervalField && intervalField.field
      ? !!intervalField.field.activeFocus
      : false

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

  readonly property int reminderMinutes: {
    if (draft && draft.notifications
        && isFinite(Number(draft.notifications.reminderMinutes)))
      return Number(draft.notifications.reminderMinutes)
    return 120
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
      text: "Loading\u2026"
      color: Util.alpha(root.foreground, 0.72)
      font.family: root.fontFamily
      font.pixelSize: Style.font.caption
      textFormat: Text.PlainText
    }

    Text {
      visible: root.saving
      width: parent.width
      text: "Saving\u2026"
      color: Util.alpha(root.foreground, 0.72)
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
        color: Util.alpha(root.foreground, 0.55)
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

    PanelSeparator {
      width: parent.width
      foreground: root.foreground
    }

    // Display metric
    Column {
      width: parent.width
      spacing: Style.space(6)
      opacity: root.locked ? 0.55 : 1.0
      enabled: !root.locked

      Text {
        text: "Bar shows"
        color: Util.alpha(root.foreground, 0.55)
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

      Row {
        spacing: Style.spacing.lg

        NumberField {
          id: intervalField
          label: "Refresh every"
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

        // The host NumberField exposes no suffix property (measured), so the
        // unit is a sibling label, and it cannot use anchors, because the
        // spin box is a child of a sibling, which QML refuses to anchor to.
        // The y binding composes intervalField's own offset within this Row
        // with the spin box's offset inside intervalField's Column, so the
        // label tracks the spin box from whatever frame the Row places the
        // field in.
        Text {
          y: intervalField.y + intervalField.field.y
             + (intervalField.field.height - height) / 2
          text: "seconds"
          color: Util.alpha(root.foreground, 0.72)
          font.family: root.fontFamily
          font.pixelSize: Style.font.bodySmall
          textFormat: Text.PlainText
          Accessible.ignored: true
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
        description: "Warn me before a quota runs out."
        checked: root.notificationsOn
        foreground: root.foreground
        fontFamily: root.fontFamily
        onClicked: {
          if (root.agentService)
            root.agentService.setNotificationsEnabled(!root.notificationsOn)
        }
      }

      Row {
        spacing: Style.spacing.lg

        NumberField {
          id: reminderField
          label: "Remind me every"
          value: root.reminderMinutes
          from: 15
          to: 1440
          stepSize: 15
          foreground: root.foreground
          fontFamily: root.fontFamily
          onModified: function (v) {
            if (root.agentService)
              root.agentService.setReminderMinutes(v)
          }
        }

        // Same sibling-label technique as the refresh interval above: the
        // host NumberField exposes no suffix property, and the spin box is a
        // child of a sibling, which QML refuses to anchor to.
        Text {
          y: reminderField.y + reminderField.field.y
             + (reminderField.field.height - height) / 2
          text: "minutes"
          color: Util.alpha(root.foreground, 0.72)
          font.family: root.fontFamily
          font.pixelSize: Style.font.bodySmall
          textFormat: Text.PlainText
          Accessible.ignored: true
        }
      }
    }

    PanelSeparator {
      width: parent.width
      foreground: root.foreground
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

    PanelSeparator {
      width: parent.width
      foreground: root.foreground
    }

    MaintenanceView {
      width: parent.width
      agentService: root.agentService
      foreground: root.foreground
      fontFamily: root.fontFamily
    }
  }
}
