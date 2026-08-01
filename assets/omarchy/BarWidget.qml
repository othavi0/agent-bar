import QtQuick
import Quickshell
import Quickshell.Wayland
import qs.Ui
import qs.Commons
import "CoreView.js" as Core
import "CoreService.js" as Service
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
    return Service.defaultSettings()
  }

  readonly property string displayMetric: Core.displayMetric(appliedSettings)

  readonly property var chipProviders: Core.visibleProviders(
    agentService ? agentService.snapshot : null,
    appliedSettings
  )

  readonly property color chipForeground: bar ? bar.foreground : Color.foreground
  readonly property string chipFontFamily: bar ? bar.fontFamily : "monospace"

  // Popup is open on another monitor → this instance hosts dismiss-only overlay.
  readonly property bool foreignDismissActive: Service.foreignPopupOpen(
    agentService ? agentService.popupOwner : null,
    root
  )

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
    columnSpacing: Style.spacing.md
    rowSpacing: Style.spacing.md

    Repeater {
      model: root.chipProviders

      ProviderChip {
        id: chip
        required property var modelData

        bar: root.bar
        providerId: String(modelData.id || "")
        displayName: modelData.name ? String(modelData.name) : Core.providerDisplayName(providerId)
        numeralText: Core.chipNumeralText(modelData, root.displayMetric)
        stateCue: Core.chipStateCue(modelData)
        cueLabel: Core.chipCueLabel(modelData)
        severityUrgent: Core.chipSeverityUrgent(modelData)
        tooltipText: Core.chipTooltip(modelData, root.displayMetric)
        iconSource: root.iconUrl(providerId)
        tinted: Core.iconTinted(providerId)
        iconScale: Core.iconOpticalScale(providerId)
        foreground: root.chipForeground
        fontFamily: root.chipFontFamily
        dimmed: Core.chipDimmed(modelData)

        onPressed: function (button) {
          root.handleChipPress(chip.providerId, button)
        }
      }
    }
  }

  // Monitor-local popup; only the owning bar instance opens (UX-021/022).
  // Direct child + property initializers (model-usage / media pattern).
  // Do not wrap in Loader+Component: under Quattro that path leaves
  // KeyboardPanel required anchorItem/bar unset (Loader Error, no panel).
  Popup {
    anchorItem: root
    bar: root.bar
    owner: root
    agentService: root.agentService
  }

  // Cross-monitor outside-click: KeyboardPanel only maps on the owner
  // monitor. Non-owner bars host a transparent full-screen dismiss layer so
  // a click on the other desktop always closes (design D1). Bar strip
  // forwards to clickTargets so chips can still transfer (D2/D3).
  PanelWindow {
    id: foreignDismiss
    property var anchorWindow: root.QsWindow ? root.QsWindow.window : null
    screen: anchorWindow ? anchorWindow.screen : null
    visible: root.foreignDismissActive && root.bar !== null
    color: "transparent"
    exclusionMode: ExclusionMode.Ignore
    WlrLayershell.namespace: "agent-bar-foreign-dismiss"
    WlrLayershell.layer: WlrLayer.Overlay
    WlrLayershell.keyboardFocus: WlrKeyboardFocus.None
    anchors {
      top: true
      bottom: true
      left: true
      right: true
    }

    readonly property string barPos: root.bar ? root.bar.position : "top"
    readonly property real barH: anchorWindow ? anchorWindow.height : (root.bar ? root.bar.barSize : 26)
    readonly property real barW: anchorWindow ? anchorWindow.width : 0
    readonly property real screenW: screen ? screen.width : 0
    readonly property real screenH: screen ? screen.height : 0
    readonly property real barStripSize: {
      if (!root.bar)
        return 0
      var actual = (barPos === "top" || barPos === "bottom") ? barH : barW
      return Math.max(root.bar.barSize || 0, actual)
    }

    MouseArea {
      id: foreignDismissArea
      anchors.fill: parent
      enabled: root.foreignDismissActive
      hoverEnabled: true
      acceptedButtons: Qt.LeftButton | Qt.RightButton | Qt.MiddleButton

      function inBarRegion(px, py) {
        if (foreignDismiss.barPos === "bottom")
          return py >= foreignDismiss.screenH - foreignDismiss.barStripSize
        if (foreignDismiss.barPos === "left")
          return px <= foreignDismiss.barStripSize
        if (foreignDismiss.barPos === "right")
          return px >= foreignDismiss.screenW - foreignDismiss.barStripSize
        return py <= foreignDismiss.barStripSize
      }

      function barPoint(px, py) {
        if (foreignDismiss.barPos === "bottom")
          return Qt.point(px, py - (foreignDismiss.screenH - foreignDismiss.barH))
        if (foreignDismiss.barPos === "right")
          return Qt.point(px - (foreignDismiss.screenW - foreignDismiss.barW), py)
        return Qt.point(px, py)
      }

      function forwardBarClick(px, py, button) {
        if (!foreignDismiss.anchorWindow || !root.bar || !root.bar.clickTargets)
          return false
        var p = barPoint(px, py)
        var targets = root.bar.clickTargets
        for (var i = targets.length - 1; i >= 0; i--) {
          var target = targets[i]
          if (!target || typeof target.triggerPress !== "function")
            continue
          if (target.visible === false || target.opacity === 0)
            continue
          if (root.bar.targetBelongsToWindow
              && !root.bar.targetBelongsToWindow(target, foreignDismiss.anchorWindow))
            continue
          if (!foreignDismiss.anchorWindow.itemPosition)
            continue
          var pos = foreignDismiss.anchorWindow.itemPosition(target)
          if (!pos)
            continue
          if (p.x >= pos.x && p.x <= pos.x + target.width
              && p.y >= pos.y && p.y <= pos.y + target.height) {
            target.triggerPress(button)
            return true
          }
        }
        return false
      }

      function runDismiss() {
        if (!root.agentService)
          return
        root.agentService.dismissPopup()
      }

      onPressed: function (mouse) {
        // Prefer pressed over clicked: some layer-shell surfaces drop click
        // composition when focus stays exclusive on the owner monitor panel.
        if (inBarRegion(mouse.x, mouse.y)
            && forwardBarClick(mouse.x, mouse.y, mouse.button)) {
          mouse.accepted = true
          return
        }
        runDismiss()
        mouse.accepted = true
      }
    }
  }
}
