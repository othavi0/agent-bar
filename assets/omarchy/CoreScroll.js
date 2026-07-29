// A11y / scroll: focus, viewport math, keyboard routing.
.pragma library

function focusNextIndex(current, direction, count) {
  var n = Math.floor(Number(count))
  if (!isFinite(n) || n <= 0)
    return -1
  var cur = Math.floor(Number(current))
  var dir = direction < 0 ? -1 : 1
  if (!isFinite(cur) || cur < 0 || cur >= n)
    return dir > 0 ? 0 : n - 1
  return (cur + dir + n * 100) % n
}

function maxContentY(contentHeight, viewportHeight) {
  var ch = Number(contentHeight)
  var vh = Number(viewportHeight)
  if (!isFinite(ch) || !isFinite(vh))
    return 0
  return Math.max(0, ch - vh)
}

// A11Y short-content: Flickable must not accept wheel/drag when no overflow.
function flickableInteractive(contentHeight, viewportHeight) {
  return maxContentY(contentHeight, viewportHeight) > 0
}

// Card height from real body (not a large empty floor). minCompact is a
// small floor for header+one row; maxCap is maxContentHeight.
function fittedPopupContentHeight(bodyHeight, minCompact, maxCap) {
  var body = Number(bodyHeight)
  var minH = Number(minCompact)
  var maxH = Number(maxCap)
  if (!isFinite(body) || body < 0)
    body = 0
  if (!isFinite(minH) || minH < 0)
    minH = 0
  if (!isFinite(maxH) || maxH <= 0)
    maxH = body
  return Math.min(maxH, Math.max(minH, body))
}

function clampContentY(y, contentHeight, viewportHeight) {
  var max = maxContentY(contentHeight, viewportHeight)
  var n = Number(y)
  if (!isFinite(n) || n < 0)
    return 0
  if (n > max)
    return max
  return n
}

// A11Y-023: PageUp/PageDown move by one viewport minus one content line.
function pageScrollDelta(viewportHeight, lineHeight) {
  var line = Math.max(1, Number(lineHeight) || 1)
  var view = Math.max(line, Number(viewportHeight) || line)
  return Math.max(line, view - line)
}

function applyPageScroll(contentY, direction, viewportHeight, contentHeight, lineHeight) {
  var delta = pageScrollDelta(viewportHeight, lineHeight)
  var next = Number(contentY) + (direction > 0 ? delta : -delta)
  return clampContentY(next, contentHeight, viewportHeight)
}

function scrollHomeY() {
  return 0
}

function scrollEndY(contentHeight, viewportHeight) {
  return maxContentY(contentHeight, viewportHeight)
}

// Panel shortcuts suspended while a native editor owns focus (A11Y-008).
function panelShortcutsBlocked(editorActive) {
  return !!editorActive
}

function routePanelTextKey(key, editorActive) {
  if (panelShortcutsBlocked(editorActive))
    return { action: "noop" }
  var k = String(key || "")
  if (k === "s" || k === "S")
    return { action: "openSettings" }
  if (k === "r" || k === "R")
    return { action: "refresh" }
  if (k === "j" || k === "J")
    return { action: "providerDelta", delta: 1 }
  if (k === "k" || k === "K")
    return { action: "providerDelta", delta: -1 }
  return { action: "noop" }
}

function routeProviderDelta(providerIds, selectedId, delta) {
  if (!Array.isArray(providerIds) || providerIds.length === 0)
    return null
  var ids = []
  for (var i = 0; i < providerIds.length; i++)
    ids.push(String(providerIds[i]))
  var cur = -1
  var want = String(selectedId || "")
  for (var j = 0; j < ids.length; j++) {
    if (ids[j] === want) {
      cur = j
      break
    }
  }
  var next = focusNextIndex(cur, delta, ids.length)
  if (next < 0)
    return null
  return ids[next]
}

// Map item geometry into flickable content and return a contentY that fully
// reveals the item when possible (A11Y-011).
function contentYForItem(contentY, viewportHeight, contentHeight, itemY, itemHeight) {
  var y = Number(itemY)
  var h = Math.max(0, Number(itemHeight))
  var vh = Math.max(0, Number(viewportHeight))
  var cur = Number(contentY)
  if (!isFinite(y) || !isFinite(vh))
    return clampContentY(cur, contentHeight, viewportHeight)
  if (y < cur)
    return clampContentY(y, contentHeight, viewportHeight)
  if (y + h > cur + vh)
    return clampContentY(y + h - vh, contentHeight, viewportHeight)
  return clampContentY(cur, contentHeight, viewportHeight)
}
