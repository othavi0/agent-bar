import QtQuick
import "../CoreScroll.js" as Core

// Ordered focus ring for popup actions (A11Y-003/010/011).
// Targets are plain Items that may expose:
//   - forceActiveFocus()
//   - focusActivate()  → typed activation callback
//   - enabled / visible
Item {
  id: root

  property var targets: []
  property int index: -1
  property var flickable: null
  property int lineHeight: 18

  readonly property int count: targets && targets.length ? targets.length : 0
  readonly property var current: {
    if (index < 0 || index >= count)
      return null
    return targets[index]
  }

  function isTargetLive(item) {
    if (!item)
      return false
    if (item.visible === false)
      return false
    if (item.enabled === false)
      return false
    if (item.opacity === 0)
      return false
    return true
  }

  function liveTargets() {
    var out = []
    if (!targets)
      return out
    for (var i = 0; i < targets.length; i++) {
      if (isTargetLive(targets[i]))
        out.push(targets[i])
    }
    return out
  }

  function setTargets(list) {
    targets = list || []
    if (count === 0) {
      index = -1
      return
    }
    if (index < 0 || index >= count)
      index = 0
    focusCurrent()
  }

  function move(direction) {
    var live = liveTargets()
    if (!live.length) {
      index = -1
      return null
    }
    // Map current into live list
    var curLive = -1
    if (current) {
      for (var i = 0; i < live.length; i++) {
        if (live[i] === current) {
          curLive = i
          break
        }
      }
    }
    var nextLive = Core.focusNextIndex(curLive, direction, live.length)
    var item = live[nextLive]
    // Sync index into full targets array
    for (var j = 0; j < targets.length; j++) {
      if (targets[j] === item) {
        index = j
        break
      }
    }
    focusCurrent()
    return item
  }

  function focusCurrent() {
    var item = current
    if (!item)
      return
    if (typeof item.forceActiveFocus === "function")
      item.forceActiveFocus()
    ensureVisible(item)
  }

  function activate() {
    var item = current
    if (!item || !isTargetLive(item))
      return false
    if (typeof item.focusActivate === "function") {
      item.focusActivate()
      return true
    }
    if (typeof item.clicked === "function") {
      // Prefer typed callback; Qt Quick Controls often expose clicked as signal.
    }
    if (item.Accessible && typeof item.Accessible.pressAction === "function") {
      item.Accessible.pressAction()
      return true
    }
    // Fall back to Accessible.onPressAction via pressAction if present
    if (typeof item.pressAction === "function") {
      item.pressAction()
      return true
    }
    return false
  }

  function ensureVisible(item) {
    if (!flickable || !item)
      return
    // Prefer mapToItem when available; else item.y relative to content item.
    var y = 0
    var h = item.height || 0
    try {
      if (flickable.contentItem && item.mapToItem) {
        var p = item.mapToItem(flickable.contentItem, 0, 0)
        y = p.y
      } else if (typeof item.y === "number") {
        y = item.y
      }
    } catch (e) {
      return
    }
    flickable.contentY = Core.contentYForItem(
      flickable.contentY,
      flickable.height,
      flickable.contentHeight,
      y,
      h
    )
  }

  function scrollPage(direction) {
    if (!flickable)
      return
    flickable.contentY = Core.applyPageScroll(
      flickable.contentY,
      direction,
      flickable.height,
      flickable.contentHeight,
      root.lineHeight
    )
  }

  function scrollHome() {
    if (!flickable)
      return
    flickable.contentY = Core.scrollHomeY()
  }

  function scrollEnd() {
    if (!flickable)
      return
    flickable.contentY = Core.scrollEndY(flickable.contentHeight, flickable.height)
  }

  function clampScroll() {
    if (!flickable)
      return
    flickable.contentY = Core.clampContentY(
      flickable.contentY,
      flickable.contentHeight,
      flickable.height
    )
  }
}
