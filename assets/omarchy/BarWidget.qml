import QtQuick
import qs.Ui
import qs.Commons
import "ServiceCore.js" as Core
import "components"

// Monitor-local provider chips. Resolves the shared service via shell.serviceFor.
// Presentation only — Service owns I/O and polling (ARCH-023).
BarWidget {
  id: root
  moduleName: "agent-bar.usage"

  readonly property var agentService: bar && bar.shell
      ? bar.shell.serviceFor(moduleName)
      : null

  readonly property var appliedSettings: {
    if (agentService && agentService.appliedSettings)
      return agentService.appliedSettings
    return Core.defaultSettings()
  }

  readonly property string displayMetric: Core.displayMetric(appliedSettings)

  readonly property var chipProviders: Core.visibleProviders(
    agentService ? agentService.snapshot : null,
    appliedSettings
  )

  readonly property color chipForeground: bar ? bar.foreground : Color.foreground
  readonly property string chipFontFamily: bar ? bar.fontFamily : "monospace"

  function iconUrl(providerId) {
    var name = Core.iconFileName(providerId)
    if (!name.length)
      return ""
    return Qt.resolvedUrl("icons/" + name)
  }

  function handleChipPress(providerId, button) {
    if (!agentService)
      return
    var route = Core.routeChipClick(button, root, providerId, agentService.popupOwner)
    if (route.action === "refreshAll") {
      agentService.refreshAll(!!route.force)
      return
    }
    if (route.action === "openSettings") {
      agentService.openSettings(root)
      return
    }
    if (route.action === "closePopup") {
      agentService.closePopup(root)
      return
    }
    if (route.action === "requestPopup") {
      agentService.requestPopup(root, route.providerId, route.view || "usage")
    }
  }

  implicitWidth: root.vertical
      ? root.barSize
      : Math.max(12, chips.implicitWidth + Style.space(12))
  implicitHeight: root.vertical
      ? Math.max(root.barSize, chips.implicitHeight + Style.space(12))
      : root.barSize

  Grid {
    id: chips
    anchors.centerIn: parent
    columns: root.vertical ? 1 : Math.max(1, root.chipProviders.length)
    columnSpacing: Style.space(10)
    rowSpacing: Style.space(6)

    Repeater {
      model: root.chipProviders

      ProviderChip {
        id: chip
        required property var modelData

        bar: root.bar
        providerId: String(modelData.id || "")
        displayName: modelData.name ? String(modelData.name) : Core.providerDisplayName(providerId)
        percentText: Core.chipPercentText(modelData, root.displayMetric)
        stateCue: Core.chipStateCue(modelData)
        tooltipText: Core.chipTooltip(modelData, root.displayMetric)
        iconSource: root.iconUrl(providerId)
        foreground: root.chipForeground
        fontFamily: root.chipFontFamily
        fontPixelSize: Style.font.body
        vertical: root.vertical
        barSize: root.barSize
        dimmed: Core.chipDimmed(modelData)

        onPressed: function (button) {
          root.handleChipPress(chip.providerId, button)
        }
      }
    }
  }

  // Monitor-local popup; only the owning bar instance opens (UX-021/022).
  Loader {
    active: root.bar !== null
    sourceComponent: popupComponent
  }

  Component {
    id: popupComponent
    Popup {
      anchorItem: root
      bar: root.bar
      owner: root
      agentService: root.agentService
    }
  }
}
