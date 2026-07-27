import QtQuick
import QtQuick.Controls
import qs.Ui
import qs.Commons
import "ServiceCore.js" as Core
import "components"

// Monitor-local consolidated popup (UX-013..025, A11Y-001..023).
//
// Do NOT redeclare KeyboardPanel's required anchorItem/bar here. Redeclaring
// them as required on this derived type makes Loader/createObject treat the
// base required props as unset (createObject returns null; chip click is a
// no-op). Call sites pass anchorItem + bar like first-party model-usage.
KeyboardPanel {
  id: root

  property var owner: null
  property var agentService: null

  property int maxContentWidth: Style.space(540)
  property int maxContentHeight: Style.space(560)
  // Compact floor: header + one window row + rail stack — not a 280px void.
  property int minContentHeight: Style.space(160)
  property int contentLineHeight: Style.font.body + Style.space(8)
  property int contentMargins: Style.space(14)

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
    // Prefer the open popup's provider so the rail plate matches the pane
    // (selectedProviderId alone could lag / fallback to Claude).
    if (agentService.popupOwner && agentService.popupOwner.providerId
        && String(agentService.popupOwner.providerId).length)
      return String(agentService.popupOwner.providerId)
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

  // Prefer real content height; rail min height prevents icon crush/overlap.
  readonly property int measuredBodyHeight: {
    var col = contentColumn ? contentColumn.implicitHeight : 0
    var margins = contentMargins * 2
    var railMin = rail && rail.minStackHeight
        ? rail.minStackHeight + Style.space(8)
        : Style.space(160)
    return Math.max(col + margins, railMin)
  }

  open: isOpen
  contentWidth: maxContentWidth
  contentHeight: Core.fittedPopupContentHeight(
    measuredBodyHeight + padding * 2,
    minContentHeight,
    maxContentHeight
  )
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

  FocusController {
    id: focusController
    flickable: contentFlick
    lineHeight: root.contentLineHeight
  }

  function rebuildFocusTargets() {
    if (!focusController || typeof focusController.setTargets !== "function")
      return
    var list = []
    if (rail && typeof rail.collectFocusTargets === "function")
      list = list.concat(rail.collectFocusTargets())
    if (contentLoader.item && typeof contentLoader.item.collectFocusTargets === "function")
      list = list.concat(contentLoader.item.collectFocusTargets())
    focusController.setTargets(list)
  }

  onViewChanged: {
    if (contentFlick)
      contentFlick.contentY = 0
    Qt.callLater(function () {
      if (typeof root.rebuildFocusTargets === "function")
        root.rebuildFocusTargets()
    })
  }
  onSelectedIdChanged: Qt.callLater(function () {
    // FocusController may not be ready on the first selectedId emission.
    if (focusController && typeof focusController.clampScroll === "function")
      focusController.clampScroll()
    if (typeof root.rebuildFocusTargets === "function")
      root.rebuildFocusTargets()
  })

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

    // A11Y-023: page/home/end. Nested under an Item (not KeyboardPanel default
    // contentItem) because Shortcut is not a QQuickItem and live Quattro rejects
    // non-Item default children ("Cannot assign QQuickShortcut to contentItem").
    Item {
      id: scrollShortcuts
      width: 0
      height: 0
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
    }

    Row {
      id: panelBody
      anchors.fill: parent
      spacing: 0

      ProviderRail {
        id: rail
        width: rail.railWidth
        // Fill panel height so ColumnLayout spacer can expand; min via measuredBodyHeight.
        height: parent.height
        providers: root.railProviders
        selectedProviderId: root.selectedId
        foreground: Color.foreground
        fontFamily: Style.font.family
        iconBase: Qt.resolvedUrl("icons/")
        onProviderSelected: function (id) { root.selectProvider(id) }
        onSettingsClicked: root.openSettings()
      }

      // Gutter between bordered rail and content (must match content width math).
      Item {
        id: railGutter
        width: Style.space(8)
        height: parent.height
      }

      Item {
        // Exact remaining width — previous `- 1` made content too wide and
        // clipped the first glyphs of titles / body / action labels.
        width: Math.max(0, parent.width - rail.width - railGutter.width)
        height: parent.height
        clip: true

        Flickable {
          id: contentFlick
          anchors.fill: parent
          anchors.leftMargin: root.contentMargins
          anchors.rightMargin: root.contentMargins
          anchors.topMargin: root.contentMargins
          anchors.bottomMargin: root.contentMargins
          contentWidth: width
          contentHeight: contentColumn.implicitHeight
          clip: true
          boundsBehavior: Flickable.StopAtBounds
          flickableDirection: Flickable.VerticalFlick
          interactive: Core.flickableInteractive(contentHeight, height)
          ScrollBar.vertical: ScrollBar {
            policy: contentFlick.interactive ? ScrollBar.AsNeeded : ScrollBar.AlwaysOff
          }

          onContentHeightChanged: {
            if (!Core.flickableInteractive(contentHeight, height))
              contentY = 0
            else if (focusController && typeof focusController.clampScroll === "function")
              focusController.clampScroll()
          }
          onHeightChanged: {
            if (!Core.flickableInteractive(contentHeight, height))
              contentY = 0
            else if (focusController && typeof focusController.clampScroll === "function")
              focusController.clampScroll()
          }

          Column {
            id: contentColumn
            width: contentFlick.width

            Loader {
              id: contentLoader
              width: parent.width
              sourceComponent: root.view === "settings" ? settingsContent : providerContent
              onLoaded: Qt.callLater(function () {
                if (typeof root.rebuildFocusTargets === "function")
                  root.rebuildFocusTargets()
              })
            }
          }
        }
      }
    }
  }

  // Declared as properties (not default contentItem children): KeyboardPanel's
  // default property is a QQuickItem list and rejects QQmlComponent objects.
  property Component providerContent: Component {
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

  property Component settingsContent: Component {
    SettingsView {
      width: contentColumn.width
      agentService: root.agentService
      foreground: Color.foreground
      fontFamily: Style.font.family
      iconBase: Qt.resolvedUrl("icons/")
    }
  }
}
