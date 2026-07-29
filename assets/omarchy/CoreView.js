// Chip + popup presentation and time formatting.
.pragma library
.import "CoreService.js" as Kernel

function displayMetric(settings) {
  if (settings && settings.display && settings.display.metric === "used")
    return "used"
  return "remaining"
}

function providerDisplayName(id) {
  var key = String(id || "")
  if (key === "claude")
    return "Claude"
  if (key === "codex")
    return "Codex"
  if (key === "amp")
    return "Amp"
  if (key === "grok")
    return "Grok"
  return key
}

function iconFileName(id) {
  var key = String(id || "")
  if (key === "claude")
    return "claude.png"
  if (key === "codex")
    return "codex.png"
  if (key === "amp")
    return "amp.svg"
  if (key === "grok")
    return "grok.svg"
  return ""
}

function placeholderProvider(id) {
  var key = String(id || "")
  return {
    id: key,
    name: providerDisplayName(key),
    state: "loading",
    source: null,
    plan: null,
    account: null,
    windows: [],
    lastSuccessAt: null,
    error: null,
    action: null
  }
}

// Enabled providers in settings order, joined with snapshot data.
// Without settings, uses snapshot order (helper already filters/orders).
function visibleProviders(snapshot, settings) {
  var byId = {}
  if (snapshot && Array.isArray(snapshot.providers)) {
    for (var i = 0; i < snapshot.providers.length; i++) {
      var p = snapshot.providers[i]
      if (p && p.id)
        byId[String(p.id)] = p
    }
  }

  var cfg = settings && Array.isArray(settings.providers) ? settings.providers : null
  if (!cfg) {
    var fromSnap = []
    if (snapshot && Array.isArray(snapshot.providers)) {
      for (var s = 0; s < snapshot.providers.length; s++)
        fromSnap.push(snapshot.providers[s])
    }
    return fromSnap
  }

  var out = []
  for (var j = 0; j < cfg.length; j++) {
    var item = cfg[j]
    if (!item || !item.enabled)
      continue
    var id = String(item.id || "")
    if (!Kernel.CLOSED_PROVIDERS[id])
      continue
    if (byId[id])
      out.push(byId[id])
    else
      out.push(placeholderProvider(id))
  }
  return out
}

function primaryWindow(provider) {
  if (!provider || !Kernel.isArrayLike(provider.windows) || provider.windows.length === 0)
    return null
  return provider.windows[0]
}

// UX-002 / UX-032A: used|remaining percent, or em-dash when empty.
function chipPercentText(provider, metric) {
  var w = primaryWindow(provider)
  if (!w)
    return "\u2014"
  var mode = metric === "used" ? "used" : "remaining"
  var v = mode === "used" ? Number(w.usedPercent) : Number(w.remainingPercent)
  if (!isFinite(v))
    return "\u2014"
  return Math.round(v) + "%"
}

// UX-012: text cue beyond color for stale/error/loading.
function chipStateCue(provider) {
  if (!provider)
    return ""
  var state = String(provider.state || "")
  if (state === "stale")
    return " ⌛"
  if (state === "loading")
    return "\u2026"
  if (state === "cli_missing" || state === "unauthenticated" || state === "rate_limited"
      || state === "network_error" || state === "provider_error")
    return " !"
  return ""
}

function chipDimmed(provider) {
  if (!provider)
    return true
  var state = String(provider.state || "")
  return state !== "ready"
}

// UX-011: provider, displayed percentage, state, reset summary.
function chipTooltip(provider, metric, nowMs) {
  if (!provider)
    return ""
  var name = provider.name ? String(provider.name) : providerDisplayName(provider.id)
  var pct = chipPercentText(provider, metric)
  var state = provider.state ? String(provider.state) : "unknown"
  var parts = [name, pct, state]
  var w = primaryWindow(provider)
  if (w && w.resetsAt) {
    var resetText = formatResetText(String(w.resetsAt), nowMs === undefined ? Date.now() : nowMs)
    if (resetText) {
      var label = w.label ? String(w.label) : String(w.id || "window")
      parts.push("resets " + label + " " + resetText)
    }
  }
  return parts.join(" \u00b7 ")
}

// Map mouse button to typed service intention (UX-004..009).
// button: Qt.LeftButton(1) | RightButton(2) | MiddleButton(4) or string.
function routeChipClick(button, owner, providerId, popupOwner) {
  var b = button
  var isLeft = b === 1 || b === "left" || b === "LeftButton"
  var isRight = b === 2 || b === "right" || b === "RightButton"
  var isMiddle = b === 4 || b === "middle" || b === "MiddleButton"

  if (isMiddle)
    return { action: "refreshAll", force: true }
  if (isRight)
    return { action: "openSettings", owner: owner }
  if (isLeft) {
    var pid = String(providerId || "")
    if (popupOwner
        && popupOwner.owner === owner
        && String(popupOwner.providerId || "") === pid
        && (!popupOwner.view || popupOwner.view === "usage")) {
      return { action: "closePopup", owner: owner }
    }
    return {
      action: "requestPopup",
      owner: owner,
      providerId: pid,
      view: "usage"
    }
  }
  return { action: "noop" }
}

// ---------------------------------------------------------------------------
// Popup / provider presentation (UX-013..032A, JSON-023..028)
// ---------------------------------------------------------------------------

var ACTION_INTENTS = {
  "retry": true,
  "login": true,
  "view_installation": true
}

var MONEY_COPY_RE = /(?:\bBRL\b|\$|USD|EUR|GBP|\bspend\b|\bbalance\b|\bcredits?\b|\bcost\b|\bprice\b|\bcurrency\b)/i

function findProvider(snapshot, providerId) {
  if (!snapshot || !Array.isArray(snapshot.providers))
    return null
  var want = String(providerId || "")
  for (var i = 0; i < snapshot.providers.length; i++) {
    var p = snapshot.providers[i]
    if (p && String(p.id) === want)
      return p
  }
  return null
}

function resolveSelectedProvider(snapshot, selectedProviderId, settings) {
  var chips = visibleProviders(snapshot, settings)
  if (!chips.length)
    return null
  var want = String(selectedProviderId || "")
  if (want) {
    for (var i = 0; i < chips.length; i++) {
      if (String(chips[i].id) === want)
        return chips[i]
    }
  }
  return chips[0]
}

function connectionLabel(state) {
  var s = String(state || "")
  if (s === "ready")
    return "Connected"
  if (s === "stale")
    return "Stale"
  if (s === "loading")
    return "Loading"
  if (s === "cli_missing")
    return "CLI missing"
  if (s === "unauthenticated")
    return "Not connected"
  if (s === "rate_limited")
    return "Rate limited"
  if (s === "network_error")
    return "Network error"
  if (s === "provider_error")
    return "Provider error"
  return "Unknown"
}

function planBadge(provider) {
  if (!provider || !provider.plan)
    return ""
  if (provider.plan.label)
    return String(provider.plan.label)
  if (provider.plan.id)
    return String(provider.plan.id)
  return ""
}

// Render only the safe English message from the typed error object.
function errorMessage(provider) {
  if (!provider || !provider.error)
    return ""
  var msg = provider.error.message
  if (msg === null || msg === undefined)
    return ""
  return plainText(String(msg))
}

function plainText(value) {
  var s = String(value === null || value === undefined ? "" : value)
  // Drop control chars / ANSI; never treat as HTML.
  s = s.replace(/\u001b\[[0-9;]*[A-Za-z]/g, "")
  s = s.replace(/[\u0000-\u0008\u000B\u000C\u000E-\u001F\u007F]/g, "")
  return s
}

function containsMoneyCopy(text) {
  return MONEY_COPY_RE.test(String(text || ""))
}

function emptyWindowsMessage() {
  return "Percentage usage is not available for this account"
}

function stateTitle(provider) {
  if (!provider)
    return "Loading"
  var s = String(provider.state || "")
  if (s === "loading")
    return "Loading"
  if (s === "ready" && (!provider.windows || provider.windows.length === 0))
    return "No percentage usage"
  if (s === "stale")
    return "Stale"
  if (s === "cli_missing")
    return "CLI not found"
  if (s === "unauthenticated")
    return "Authentication required"
  if (s === "rate_limited")
    return "Rate limited"
  if (s === "network_error")
    return "Network error"
  if (s === "provider_error")
    return "Provider error"
  return connectionLabel(s)
}

function stateBody(provider) {
  if (!provider)
    return "Collecting provider status\u2026"
  var s = String(provider.state || "")
  if (s === "loading")
    return "Collecting provider status\u2026"
  if (s === "ready" && (!provider.windows || provider.windows.length === 0))
    return emptyWindowsMessage()
  if (s === "stale") {
    var base = "Showing the last successful result."
    var err = errorMessage(provider)
    if (err.length)
      return base + " " + err
    return base
  }
  if (s === "cli_missing") {
    var cli = errorMessage(provider)
    return cli.length ? cli : "Required CLI was not found."
  }
  if (s === "unauthenticated") {
    var auth = errorMessage(provider)
    return auth.length ? auth : "Sign in to collect usage."
  }
  if (s === "rate_limited") {
    var rl = errorMessage(provider)
    return rl.length ? rl : "The provider rate-limited this request. Try again shortly."
  }
  if (s === "network_error") {
    var net = errorMessage(provider)
    return net.length ? net : "A temporary network error prevented collection."
  }
  if (s === "provider_error") {
    var pe = errorMessage(provider)
    return pe.length ? pe : "The provider returned an unusable response."
  }
  return ""
}

function defaultActionLabel(kind) {
  if (kind === "retry")
    return "Retry"
  if (kind === "login")
    return "Connect"
  if (kind === "view_installation")
    return "View installation"
  return String(kind || "")
}

// Closed action list for the current provider state (JSON-025).
function stateActions(provider) {
  var out = []
  if (!provider)
    return out
  var state = String(provider.state || "")
  var seen = {}

  function pushAction(kind, label, target) {
    var k = String(kind || "")
    if (!ACTION_INTENTS[k] || seen[k])
      return
    seen[k] = true
    out.push({
      kind: k,
      label: plainText(label || defaultActionLabel(k)),
      target: target === undefined ? null : target
    })
  }

  if (provider.action && provider.action.kind)
    pushAction(provider.action.kind, provider.action.label, provider.action.target)

  if (state === "cli_missing")
    pushAction("retry", "Check again", null)
  if (state === "stale" || state === "rate_limited" || state === "network_error" || state === "provider_error")
    pushAction("retry", "Retry", null)
  if (state === "unauthenticated" && !seen.login && !seen.view_installation)
    pushAction("login", "Connect", null)

  // JSON-025 addendum: a typed non-retryable error must never offer Retry,
  // regardless of which branch above produced it.
  var retryAllowed = !provider || !provider.error
      || provider.error.retryable === undefined
      || provider.error.retryable === true
  if (!retryAllowed)
    out = out.filter(function (a) { return a.kind !== "retry" })

  return out
}

function mapActionKind(kind) {
  var k = String(kind || "")
  if (!ACTION_INTENTS[k])
    return null
  return k
}

// ---------------------------------------------------------------------------
// Humanized time (UX Fase 2: countdown + absolute local time)
// ---------------------------------------------------------------------------


function parseIsoMs(iso) {
  if (iso === null || iso === undefined)
    return NaN
  var s = String(iso)
  if (!s.length)
    return NaN
  var ms = Date.parse(s)
  return isFinite(ms) ? ms : NaN
}

var WEEKDAYS = ["Sun", "Mon", "Tue", "Wed", "Thu", "Fri", "Sat"]

function countdownText(diffMs) {
  var totalMinutes = Math.floor(diffMs / 60000)
  var days = Math.floor(totalMinutes / 1440)
  var hours = Math.floor((totalMinutes % 1440) / 60)
  var minutes = totalMinutes % 60
  if (days > 0)
    return days + "d " + hours + "h"
  if (hours > 0)
    return hours + "h " + minutes + "m"
  return minutes + "m"
}

// "2h 30m · 14:59" (<24h) | "2d 18h · Fri 09:00" (>=24h) | "now" | "".
function formatResetText(iso, nowMs) {
  var ms = parseIsoMs(iso)
  if (!isFinite(ms))
    return ""
  var diff = ms - nowMs
  if (diff <= 0)
    return "now"
  var date = new Date(ms)
  var absolute = diff >= 86400000
      ? WEEKDAYS[date.getDay()] + " " + Qt.formatDateTime(date, "hh:mm")
      : Qt.formatDateTime(date, "hh:mm")
  return countdownText(diff) + " · " + absolute
}

// "just now" | "5m ago" | "3h ago" | "2d ago" | "".
function formatAgoText(iso, nowMs) {
  var ms = parseIsoMs(iso)
  if (!isFinite(ms))
    return ""
  var diff = Math.max(0, nowMs - ms)
  if (diff < 60000)
    return "just now"
  var minutes = Math.floor(diff / 60000)
  if (minutes < 60)
    return minutes + "m ago"
  var hours = Math.floor(minutes / 60)
  if (hours < 24)
    return hours + "h ago"
  return Math.floor(hours / 24) + "d ago"
}

function windowDisplayLines(provider, metric, nowMs) {
  var lines = []
  if (!provider || !Kernel.isArrayLike(provider.windows))
    return lines
  var mode = metric === "used" ? "used" : "remaining"
  var effectiveNowMs = nowMs === undefined ? Date.now() : nowMs
  for (var i = 0; i < provider.windows.length; i++) {
    var w = provider.windows[i]
    if (!w)
      continue
    var pct = mode === "used" ? Number(w.usedPercent) : Number(w.remainingPercent)
    var finite = isFinite(pct)
    var rounded = finite ? Math.round(pct) : null
    var pctText = finite ? (rounded + "%") : "\u2014"
    lines.push({
      id: String(w.id || ("w" + i)),
      label: plainText(w.label || w.id || "Window"),
      percentText: pctText,
      // 0–100 for progress track; -1 when unavailable.
      percent: finite ? Math.max(0, Math.min(100, rounded)) : -1,
      resetsAt: w.resetsAt ? String(w.resetsAt) : null,
      resetText: w.resetsAt ? formatResetText(String(w.resetsAt), effectiveNowMs) : ""
    })
  }
  return lines
}

var PRIMARY_WINDOW_IDS = { "session": true, "weekly": true, "daily": true }

function windowGroups(provider, metric, nowMs) {
  var lines = windowDisplayLines(provider, metric, nowMs)
  var groups = { primary: [], secondary: [] }
  for (var i = 0; i < lines.length; i++) {
    if (PRIMARY_WINDOW_IDS[lines[i].id])
      groups.primary.push(lines[i])
    else
      groups.secondary.push(lines[i])
  }
  return groups
}

function headerModel(provider, refreshing) {
  if (!provider) {
    return {
      name: "",
      plan: "",
      connection: "Loading",
      lastSuccessAt: null,
      refreshing: !!refreshing,
      showStale: false
    }
  }
  return {
    name: plainText(provider.name || providerDisplayName(provider.id)),
    plan: plainText(planBadge(provider)),
    connection: connectionLabel(provider.state),
    lastSuccessAt: provider.lastSuccessAt ? String(provider.lastSuccessAt) : null,
    refreshing: !!refreshing,
    showStale: String(provider.state) === "stale"
  }
}

function contentMode(provider) {
  if (!provider)
    return "skeleton"
  var s = String(provider.state || "")
  if (s === "loading")
    return "skeleton"
  if (s === "ready") {
    if (!provider.windows || provider.windows.length === 0)
      return "empty_windows"
    return "windows"
  }
  if (s === "stale") {
    if (provider.windows && provider.windows.length > 0)
      return "stale_windows"
    return "state"
  }
  return "state"
}
