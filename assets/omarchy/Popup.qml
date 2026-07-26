import QtQuick
import QtQuick.Controls
import qs.Ui
import qs.Commons
import "ServiceCore.js" as Core
import "components"

// Monitor-local consolidated popup (UX-013..025, A11Y-001..023).
KeyboardPanel {
  id: root

  required property Item anchorItem
  required property QtObject bar
  property var owner: null
  property var agentService: null

  property int maxContentWidth: Style.space(540)
  property int maxContentHeight: Style.space(560)
  property int contentLineHeight: Style.font.body + Style.space(8)

  // A11Y-008: Settings/NumberField can raise this while editing.
  property bool editorActive: contentLoader.item && contentLoader.item.editorOwnsFocus
      ? !!contentLoader.item.editorOwnsFocus
      : false

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
    contentFlick.contentY = 0
  }

  function openSettings() {
    if (!agentService)
      return
    agentService.openSettings(owner)
    contentFlick.contentY = 0
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

  function providerIds() {
    var ids = []
    for (var i = 0; i < railProviders.length; i++) {
      if (railProviders[i] && railProviders[i].id)
        ids.push(String(railProviders[i].id))
    }
    return ids
  }

  function stepProvider(delta) {
    var next = Core.routeProviderDelta(providerIds(), root.selectedId, delta)
    if (next)
      root.selectProvider(next)
  }

  function handleTextKey(text) {
    var route = Core.routePanelTextKey(text, keyCatcher.blocked)
    if (route.action === "openSettings") {
      root.openSettings()
      return
    }
    if (route.action === "refresh") {
      if (root.selectedId.length)
        root.onRefresh(root.selectedId)
      return
    }
    if (route.action === "providerDelta")
      root.stepProvider(route.delta)
  }

  function rebuildFocusTargets() {
    var list = []
    if (rail && typeof rail.collectFocusTargets === "function")
      list = list.concat(rail.collectFocusTargets())
    if (contentLoader.item && typeof contentLoader.item.collectFocusTargets === "function")
      list = list.concat(contentLoader.item.collectFocusTargets())
    focusController.setTargets(list)
  }

  onViewChanged: {
    contentFlick.contentY = 0
    Qt.callLater(rebuildFocusTargets)
  }
  onSelectedIdChanged: Qt.callLater(function () {
    focusController.clampScroll()
    rebuildFocusTargets()
  })

  FocusController {
    id: focusController
    flickable: contentFlick
    lineHeight: root.contentLineHeight
  }

  // A11Y-023: page/home/end when panel shortcuts are live
  Shortcut {
    sequences: ["PgDown", "Page Down"]
    enabled: root.isOpen && !keyCatcher.blocked
    onActivated: focusController.scrollPage(1)
  }
  Shortcut {
    sequences: ["PgUp", "Page Up"]
    enabled: root.isOpen && !keyCatcher.blocked
    onActivated: focusController.scrollPage(-1)
  }
  Shortcut {
    sequence: "Home"
    enabled: root.isOpen && !keyCatcher.blocked
    onActivated: focusController.scrollHome()
  }
  Shortcut {
    sequence: "End"
    enabled: root.isOpen && !keyCatcher.blocked
    onActivated: focusController.scrollEnd()
  }

  PanelKeyCatcher {
    id: keyCatcher
    anchors.fill: parent
    blocked: root.editorActive

    onCloseRequested: root.close()
    onMoveRequested: function (dx, dy) {
      if (dy !== 0)
        root.stepProvider(dy)
    }
    onTabRequested: function (direction) {
      focusController.move(direction)
    }
    onActivateRequested: focusController.activate()
    onTextKey: function (text) {
      root.handleTextKey(text)
    }

    Row {
      id: panelBody
      anchors.fill: parent
      spacing: 0

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

      Rectangle {
        width: 1
        height: parent.height
        color: Qt.rgba(Color.foreground.r, Color.foreground.g, Color.foreground.b, 0.12)
      }

      Item {
        width: parent.width - rail.width - 1
        height: parent.height

        Flickable {
          id: contentFlick
          anchors.fill: parent
          anchors.margins: Style.space(14)
          contentWidth: width
          contentHeight: contentColumn.implicitHeight
          clip: true
          boundsBehavior: Flickable.StopAtBounds
          flickableDirection: Flickable.VerticalFlick
          ScrollBar.vertical: ScrollBar { policy: ScrollBar.AsNeeded }

          onContentHeightChanged: focusController.clampScroll()
          onHeightChanged: focusController.clampScroll()

          Column {
            id: contentColumn
            width: contentFlick.width

            Loader {
              id: contentLoader
              width: parent.width
              sourceComponent: root.view === "settings" ? settingsContent : providerContent
              onLoaded: Qt.callLater(rebuildFocusTargets)
            }
          }
        }
      }
    }
  }

  Component {
    id: providerContent
    ProviderView {
      width: contentColumn.width
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
      width: contentColumn.width
      agentService: root.agentService
      foreground: Color.foreground
      fontFamily: Style.font.family
      iconBase: Qt.resolvedUrl("icons/")
    }
  }
}
