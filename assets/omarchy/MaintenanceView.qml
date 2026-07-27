import QtQuick
import qs.Commons
import qs.Ui
import "ServiceCore.js" as Core
import "components"

// Maintenance section: version, update check/apply, uninstall (UX-040..047).
Item {
  id: root

  property var agentService: null
  property color foreground: Color.foreground
  property string fontFamily: Style.font.family

  readonly property var ui: agentService && agentService.maintenanceUi
      ? agentService.maintenanceUi
      : Core.maintenanceUiIdle("")

  readonly property bool blocked: agentService && agentService.maintenanceState
      ? !!agentService.maintenanceState.blocked
      : false

  readonly property bool checking: ui.phase === "checking"
  readonly property bool updateAvailable: ui.phase === "update_available"
  readonly property bool applying: ui.phase === "applying" || ui.phase === "uninstalling"

  width: parent ? parent.width : implicitWidth
  implicitHeight: body.implicitHeight

  Column {
    id: body
    width: parent.width
    spacing: Style.space(8)

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
      text: "Installed version: "
          + (ui.installedVersion && ui.installedVersion.length
              ? ui.installedVersion
              : "—")
      color: root.foreground
      font.family: root.fontFamily
      font.pixelSize: Style.font.body
      textFormat: Text.PlainText
      Accessible.name: text
    }

    Text {
      width: parent.width
      text: "Installation type: " + (ui.installType || "Plugin bundle")
      color: Qt.darker(root.foreground, 1.25)
      font.family: root.fontFamily
      font.pixelSize: Style.font.caption
      textFormat: Text.PlainText
    }

    Text {
      width: parent.width
      visible: ui.message && ui.message.length > 0
      text: ui.message
      color: ui.phase === "error" ? Color.urgent : Qt.darker(root.foreground, 1.2)
      font.family: root.fontFamily
      font.pixelSize: Style.font.caption
      wrapMode: Text.WordWrap
      textFormat: Text.PlainText
    }

    Flow {
      width: parent.width
      spacing: Style.space(8)

      Button {
        text: root.checking ? "Checking\u2026" : "Check for updates"
        bordered: true
        focusable: true
        enabled: !root.blocked && !root.checking && !root.applying
        foreground: root.foreground
        fontFamily: root.fontFamily
        Accessible.name: "Check for updates"
        onClicked: {
          if (root.agentService)
            root.agentService.checkForUpdates()
        }
      }

      Button {
        visible: root.updateAvailable && ui.targetVersion && ui.targetVersion.length > 0
        text: "Update to " + ui.targetVersion
        bordered: true
        focusable: true
        enabled: !root.blocked && !root.applying
        foreground: root.foreground
        fontFamily: root.fontFamily
        Accessible.name: text
        onClicked: {
          if (root.agentService)
            root.agentService.openUpdateConfirm()
        }
      }

      Button {
        visible: ui.releaseNotesUrl && String(ui.releaseNotesUrl).indexOf("https://") === 0
        text: "Release notes"
        bordered: true
        focusable: true
        enabled: !root.applying
        foreground: root.foreground
        fontFamily: root.fontFamily
        Accessible.name: "Release notes"
        onClicked: {
          if (root.agentService)
            root.agentService.openReleaseNotes()
        }
      }
    }

    Rectangle {
      width: parent.width
      height: 1
      color: Qt.rgba(root.foreground.r, root.foreground.g, root.foreground.b, 0.12)
    }

    // Danger zone — visually separated (UX-044)
    Column {
      width: parent.width
      spacing: Style.space(6)

      Text {
        text: "Danger zone"
        color: Color.urgent
        font.family: root.fontFamily
        font.pixelSize: Style.font.caption
        font.bold: true
        textFormat: Text.PlainText
      }

      Button {
        text: "Uninstall agent-bar"
        bordered: true
        focusable: true
        enabled: !root.blocked && !root.applying
        foreground: Color.urgent
        fontFamily: root.fontFamily
        Accessible.name: "Uninstall agent-bar"
        onClicked: {
          if (root.agentService)
            root.agentService.openUninstallConfirm()
        }
      }
    }
  }

  // Update confirmation (UX-043)
  ConfirmDialog {
    opened: !!ui.updateConfirmOpen
    title: "Confirm update"
    message: Core.updateConfirmMessage(ui)
    cancelText: "Cancel"
    confirmText: "Update"
    destructive: false
    foreground: root.foreground
    fontFamily: root.fontFamily
    onCanceled: {
      if (root.agentService)
        root.agentService.closeUpdateConfirm()
    }
    onConfirmed: {
      if (root.agentService)
        root.agentService.confirmUpdateApply()
    }
  }

  // Uninstall confirmation (UX-045..047)
  ConfirmDialog {
    opened: !!ui.uninstallConfirmOpen
    title: "Uninstall agent-bar"
    message: ui.uninstallArmed
        ? (ui.purgeSettings
            ? "Final confirmation: uninstall will remove the plugin, settings, and backups. Click Uninstall again."
            : "Final confirmation: uninstall will remove the plugin bundle. Settings stay. Click Uninstall again.")
        : "This removes the Agent Bar plugin bundle. Settings are preserved by default."
    cancelText: "Cancel"
    confirmText: ui.uninstallArmed ? "Uninstall now" : "Uninstall"
    destructive: true
    foreground: root.foreground
    fontFamily: root.fontFamily
    onCanceled: {
      if (root.agentService)
        root.agentService.closeUninstallConfirm()
    }
    onConfirmed: {
      if (root.agentService)
        root.agentService.armOrConfirmUninstall()
    }

    // UX-046: purge checkbox default unchecked
    Toggle {
      label: "Also delete saved settings and backups"
      description: "Unchecked by default. Standard uninstall preserves settings."
      checked: !!ui.purgeSettings
      foreground: root.foreground
      fontFamily: root.fontFamily
      onClicked: {
        if (root.agentService)
          root.agentService.setUninstallPurge(!ui.purgeSettings)
      }
    }
  }
}
