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

// Typed error/collection-failure states shared by the chip cue and the
// tooltip/copy qualifiers (plan 03 §5.1-5.3).
var ERROR_STATES = {
  "cli_missing": true,
  "unauthenticated": true,
  "rate_limited": true,
  "network_error": true,
  "provider_error": true
}

// UX-012: text cue beyond color for stale/error. No leading space — the
// chip separates cue from numeral with layout spacing (visual design §5).
// 󰅐 is U+F0150 from the bar's Nerd Font family, replacing the old
// emoji-font hourglass glyph that broke the monospace surface.
function chipStateCue(provider) {
  if (!provider)
    return ""
  var state = String(provider.state || "")
  if (state === "stale")
    return "󰅐"
  if (ERROR_STATES[state])
    return "!"
  return ""
}

var STATE_QUALIFIERS = {
  "stale": "stale",
  "loading": "loading",
  "cli_missing": "no CLI",
  "unauthenticated": "signed out",
  "rate_limited": "rate limited",
  "network_error": "offline",
  "provider_error": "failed"
}

// Copy design §5.4: lowercase trailing qualifier for the chip tooltip.
// Plan 03 deletes connectionLabel with the meta footer; this function is
// the humaniser that replaces it.
function stateQualifier(state) {
  var s = String(state || "")
  if (s === "ready")
    return ""
  if (STATE_QUALIFIERS[s] !== undefined)
    return STATE_QUALIFIERS[s]
  return "unknown"
}

// Numeral box content: loading renders the loading cue in place of the
// number so the cue is visually distinct from the — no-data glyph (§5).
function chipNumeralText(provider, metric) {
  if (provider && String(provider.state || "") === "loading")
    return "···"
  return chipPercentText(provider, metric)
}

function chipDimmed(provider) {
  if (!provider)
    return true
  var state = String(provider.state || "")
  return state !== "ready"
}

// UX-011 (amended by this plan): provider, displayed percentage when one
// exists, and a plain-language qualifier when not ready. The raw enum
// value never renders (copy design §5.4).
function chipTooltip(provider, metric) {
  if (!provider)
    return ""
  var name = provider.name ? String(provider.name) : providerDisplayName(provider.id)
  var parts = [name]
  var state = provider.state ? String(provider.state) : "unknown"
  var pct = chipPercentText(provider, metric)
  if (pct !== "—" || state === "ready")
    parts.push(pct)
  var qualifier = stateQualifier(state)
  if (qualifier.length)
    parts.push(qualifier)
  return parts.join(" \u00b7 ")
}

// Visual design §5/§10: per-provider optical scale on the shared 16px
// canvas. Grok's mark is a thin ring that fills its own box edge to edge.
function iconOpticalScale(id) {
  if (String(id || "") === "grok")
    return 0.875
  return 1.0
}

// §10: monochrome brands inherit the theme ink at render time; polychrome
// brands keep their official color and are never tinted.
function iconTinted(id) {
  var key = String(id || "")
  return key === "codex" || key === "grok"
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
  return "This account is billed another way."
}

function stateTitle(provider) {
  if (!provider)
    return ""
  var name = plainText(provider.name || providerDisplayName(provider.id))
  var s = String(provider.state || "")
  if (s === "loading" || s === "stale")
    return ""
  if (s === "ready" && (!provider.windows || provider.windows.length === 0))
    return name + " reports no quota"
  if (s === "ready")
    return ""
  if (s === "cli_missing")
    return name + " CLI is not installed"
  if (s === "unauthenticated")
    return "Not signed in to " + name
  if (s === "rate_limited")
    return name + " hit a rate limit"
  if (s === "network_error")
    return "Cannot reach " + name
  if (s === "provider_error")
    return name + " returned no limits"
  return name + " state is unknown"
}

function stateBody(provider) {
  if (!provider)
    return ""
  var name = plainText(provider.name || providerDisplayName(provider.id))
  var s = String(provider.state || "")
  if (s === "loading" || s === "stale")
    return ""
  if (s === "ready" && (!provider.windows || provider.windows.length === 0))
    return emptyWindowsMessage()
  var err = errorMessage(provider)
  if (err.length)
    return err
  if (s === "cli_missing")
    return "Agent Bar reads the quota through it."
  if (s === "unauthenticated")
    return "Signing in opens the official " + name + " CLI."
  if (s === "rate_limited")
    return "Try again in a few minutes."
  if (s === "network_error")
    return "Check your connection."
  return ""
}

function defaultActionLabel(kind) {
  if (kind === "retry")
    return "Retry"
  if (kind === "login")
    return "Sign in"
  if (kind === "view_installation")
    return "Install guide"
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
    pushAction("login", "Sign in", null)

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
      lastSuccessAt: null,
      refreshing: !!refreshing,
      showStale: false
    }
  }
  return {
    name: plainText(provider.name || providerDisplayName(provider.id)),
    plan: plainText(planBadge(provider)),
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
