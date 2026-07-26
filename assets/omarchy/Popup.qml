import QtQuick
import qs.Ui
import qs.Commons
import "ServiceCore.js" as Core

// Monitor-local consolidated popup (UX-013..025). Hosted by BarWidget.
KeyboardPanel {
  id: root

  required property Item anchorItem
  required property QtObject bar
  property var owner: null
  property var agentService: null

  // Maximum intentions; KeyboardPanel fitting clamps to screen (UX-020).
  property int maxContentWidth: Style.space(540)
  property int maxContentHeight: Style.space(560)

  readonly property var appliedSettings: {
    if (agentService && agentService.appliedSettings)
      return agentService.appliedSettings
    return Core.defaultSettings()
  }

  readonly property var railProviders: Core.visibleProviders(
    agentService ? agentService.snapshot : null,
    appliedSettings
  )

  readonly property string displayMetric: Core.displayMetric(appliedSettings)

  readonly property string selectedId: {
    if (!agentService)
      return ""
    if (agentService.selectedProviderId && String(agentService.selectedProviderId).length)
      return String(agentService.selectedProviderId)
    if (railProviders.length)
      return String(railProviders[0].id)
    return ""
  }

  readonly property var selectedProvider: Core.resolveSelectedProvider(
    agentService ? agentService.snapshot : null,
    selectedId,
    appliedSettings
  )

  readonly property string view: Core.popupView(agentService ? agentService.popupOwner : null)

  readonly property bool isOpen: Core.popupOpenForOwner(
    agentService ? agentService.popupOwner : null,
    owner
  )

  open: isOpen
  contentWidth: maxContentWidth
  contentHeight: Math.min(maxContentHeight, Math.max(Style.space(280), panelBody.implicitHeight + padding * 2))
  focusTarget: keyCatcher

  function close() {
    if (agentService && owner)
      agentService.closePopup(owner)
    else
      root.open = false
  }

  function selectProvider(providerId) {
    if (!agentService)
      return
    agentService.requestPopup(owner, providerId, "usage")
  }

  function openSettings() {
    if (!agentService)
      return
    agentService.openSettings(owner)
  }

  function onRefresh(providerId) {
    if (!agentService)
      return
    agentService.refreshProvider(providerId, true)
  }

  function onAction(providerId, kind, target) {
    if (!agentService)
      return
    agentService.dispatchAction(providerId, {
      kind: kind,
      label: "",
      target: target
    })
  }

  PanelKeyCatcher {
    id: keyCatcher
    anchors.fill: parent

    // A11Y-007 Escape closes
    Keys.onEscapePressed: root.close()

    // A11Y-006 s opens Settings (suspended later when editors own focus)
    Keys.onPressed: function (event) {
      if (event.key === Qt.Key_S && !(event.modifiers & Qt.ControlModifier)) {
        root.openSettings()
        event.accepted = true
      } else if (event.key === Qt.Key_R && !(event.modifiers & Qt.ControlModifier)) {
        if (root.selectedId.length)
          root.onRefresh(root.selectedId)
        event.accepted = true
      }
    }

    Row {
      id: panelBody
      anchors.fill: parent
      anchors.margins: 0
      spacing: 0

      // Icon-only rail
      ProviderRail {
        id: rail
        width: Style.space(48)
        height: parent.height
        providers: root.railProviders
        selectedProviderId: root.selectedId
        foreground: Color.foreground
        fontFamily: Style.font.family
        iconBase: Qt.resolvedUrl("icons/")
        onProviderSelected: function (id) { root.selectProvider(id) }
        onSettingsClicked: root.openSettings()
      }

      // Visual separation for the rail
      Rectangle {
        width: 1
        height: parent.height
        color: Qt.rgba(Color.foreground.r, Color.foreground.g, Color.foreground.b, 0.12)
      }

      // Content column
      Item {
        width: parent.width - rail.width - 1
        height: parent.height

        // Usage view (Task 11). Settings replaced in Task 12.
        Flickable {
          id: contentFlick
          anchors.fill: parent
          anchors.margins: Style.space(14)
          contentWidth: width
          contentHeight: contentLoader.item ? contentLoader.item.implicitHeight : 0
          clip: true
          boundsBehavior: Flickable.StopAtBounds
          // UX-024/025: scroll position resets when selection changes on new open.
          // Selection persistence while open is service-owned selectedProviderId.

          Loader {
            id: contentLoader
            width: contentFlick.width
            sourceComponent: root.view === "settings" ? settingsContent : providerContent
          }
        }
      }
    }
  }

  Component {
    id: providerContent
    ProviderView {
      width: contentFlick.width
      provider: root.selectedProvider
      displayMetric: root.displayMetric
      refreshing: agentService ? !!agentService.refreshing : false
      foreground: Color.foreground
      fontFamily: Style.font.family
      onRefreshRequested: function (id) { root.onRefresh(id) }
      onActionRequested: function (id, kind, target) { root.onAction(id, kind, target) }
    }
  }

  Component {
    id: settingsContent
    SettingsView {
      width: contentFlick.width
      agentService: root.agentService
      foreground: Color.foreground
      fontFamily: Style.font.family
      iconBase: Qt.resolvedUrl("icons/")
    }
  }
}
