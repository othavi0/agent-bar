// Envelope + consts, IPC/probe, pending, lanes, popup ownership.
.pragma library

var CLOSED_PROVIDERS = {
  "claude": true,
  "codex": true,
  "amp": true,
  "grok": true
}

var ACTION_KINDS = {
  "retry": true,
  "login": true,
  "view_installation": true
}

var PROVIDER_STATES = {
  "ready": true,
  "stale": true,
  "cli_missing": true,
  "unauthenticated": true,
  "rate_limited": true,
  "network_error": true,
  "provider_error": true
}

// ---------------------------------------------------------------------------
// Version / health / IPC refresh
// ---------------------------------------------------------------------------

function health(versionReady, versionFailed, helperVersion, manifestVersion, expectedVersion) {
  var expected = String(expectedVersion || "")
  if (!versionReady || versionFailed)
    return "unknown"
  if (String(helperVersion) === expected && String(manifestVersion) === expected)
    return "ok"
  return "unknown"
}

function isClosedProvider(providerId) {
  return !!CLOSED_PROVIDERS[String(providerId || "")]
}

// QML property-var interop: nested arrays become array-like QVariantList where
// Array.isArray is false but .length and numeric keys still work.
function isArrayLike(value) {
  if (Array.isArray(value))
    return true
  return !!(value && typeof value === "object" && typeof value.length === "number")
}

function refreshResult(providerId) {
  if (!isClosedProvider(providerId))
    return "unknown"
  return "ok"
}

function parseVersionStdout(stdout, stderr, exitCode) {
  if (exitCode !== 0)
    return null
  if (stderr && String(stderr).length > 0)
    return null
  var m = String(stdout || "").match(/^(\d+\.\d+\.\d+(?:-[0-9A-Za-z.-]+)?(?:\+[0-9A-Za-z.-]+)?)\n$/)
  return m ? m[1] : null
}

// ---------------------------------------------------------------------------
// Pending forced targets (CACHE-012): set of provider IDs or all
// ---------------------------------------------------------------------------

function emptyPending() {
  return { all: false, ids: {} }
}

function clonePending(pending) {
  var next = emptyPending()
  if (!pending)
    return next
  next.all = !!pending.all
  if (pending.ids) {
    for (var k in pending.ids)
      next.ids[k] = true
  }
  return next
}

function pendingIsEmpty(pending) {
  if (!pending)
    return true
  if (pending.all)
    return false
  for (var k in pending.ids)
    return false
  return true
}

// Union request into pending. forceAll dominates. Returns new pending object.
function unionForced(pending, providerIdOrAll) {
  var next = clonePending(pending)
  if (providerIdOrAll === "all" || providerIdOrAll === true) {
    next.all = true
    next.ids = {}
    return next
  }
  if (next.all)
    return next
  var id = String(providerIdOrAll || "")
  if (!isClosedProvider(id))
    return next
  next.ids[id] = true
  return next
}

// Capture and clear pending for a follow-up run.
function takePending(pending) {
  return {
    captured: clonePending(pending),
    remaining: emptyPending()
  }
}

// Build helper argv for status.
// force / all → cache bypass; otherwise cache use.
// notifications always evaluate for the shared service.
function statusArgv(helperPath, forceOrTargets) {
  var cacheMode = "use"
  if (forceOrTargets === true || forceOrTargets === "all")
    cacheMode = "bypass"
  else if (forceOrTargets && forceOrTargets.all)
    cacheMode = "bypass"
  else if (forceOrTargets && forceOrTargets.ids) {
    for (var k in forceOrTargets.ids) {
      cacheMode = "bypass"
      break
    }
  }
  var argv = [
    helperPath,
    "status",
    "format", "json",
    "cache", cacheMode,
    "notifications", "evaluate"
  ]
  // Single-provider force: include provider clause.
  if (forceOrTargets && forceOrTargets.ids && !forceOrTargets.all) {
    var only = null
    var count = 0
    for (var id in forceOrTargets.ids) {
      only = id
      count++
    }
    if (count === 1) {
      argv.push("provider")
      argv.push(only)
    }
  }
  return argv
}

// ---------------------------------------------------------------------------
// Status envelope validation (before snapshot replace)
// ---------------------------------------------------------------------------

function isFinitePercent(n) {
  return typeof n === "number" && isFinite(n) && n >= 0 && n <= 100
}

function validateProvider(p) {
  if (!p || typeof p !== "object")
    return "provider not an object"
  if (!isClosedProvider(p.id))
    return "invalid provider id"
  if (!PROVIDER_STATES[p.state])
    return "invalid provider state"
  if (!Array.isArray(p.windows))
    return "windows not array"
  for (var i = 0; i < p.windows.length; i++) {
    var w = p.windows[i]
    if (!w || typeof w !== "object")
      return "window not object"
    if (!isFinitePercent(w.usedPercent) || !isFinitePercent(w.remainingPercent))
      return "invalid window percent"
    if (w.action && w.action.kind && !ACTION_KINDS[w.action.kind])
      return "invalid window action" // windows shouldn't have action; provider does
  }
  if (p.action && p.action.kind && !ACTION_KINDS[p.action.kind])
    return "invalid action kind"
  return null
}

// Returns { ok: true, envelope } or { ok: false, reason }
// expectedHelperVersion: when non-empty, must match envelope.helperVersion
function parseStatusEnvelope(stdout, expectedHelperVersion) {
  var text = String(stdout || "").trim()
  if (!text.length)
    return { ok: false, reason: "empty stdout" }
  var env
  try {
    env = JSON.parse(text)
  } catch (e) {
    return { ok: false, reason: "json parse failed" }
  }
  if (!env || typeof env !== "object")
    return { ok: false, reason: "not an object" }
  if (env.schemaVersion !== 2)
    return { ok: false, reason: "schemaVersion !== 2" }
  if (expectedHelperVersion && String(env.helperVersion) !== String(expectedHelperVersion))
    return { ok: false, reason: "helperVersion mismatch" }
  if (!Array.isArray(env.providers))
    return { ok: false, reason: "providers not array" }
  for (var i = 0; i < env.providers.length; i++) {
    var err = validateProvider(env.providers[i])
    if (err)
      return { ok: false, reason: err }
  }
  return { ok: true, envelope: env }
}

// Stale-callback rule: accept only if generation still matches.
function shouldApplyGeneration(activeGeneration, callbackGeneration) {
  return activeGeneration === callbackGeneration
}

// ---------------------------------------------------------------------------
// Popup ownership
// ---------------------------------------------------------------------------

// owner: opaque id (monitor/widget instance). Returns new popup state object.
// { owner, providerId, view }
function requestPopup(current, owner, providerId, view) {
  var o = owner
  if (o === null || o === undefined)
    return current
  return {
    owner: o,
    providerId: providerId === null || providerId === undefined ? null : String(providerId),
    view: view ? String(view) : "usage"
  }
}

// Same-owner close only; cross-owner close is ignored.
function closePopup(current, owner) {
  if (!current || current.owner === null || current.owner === undefined)
    return null
  if (current.owner !== owner)
    return current
  return null
}

// Outside-click / foreign-monitor dismiss: clear ownership unconditionally.
function dismissPopup(_current) {
  return null
}

// True when this monitor/widget is not the popup owner but a popup is open.
function foreignPopupOpen(popupOwner, selfOwner) {
  if (!popupOwner || popupOwner.owner === null || popupOwner.owner === undefined)
    return false
  if (selfOwner === null || selfOwner === undefined)
    return false
  return popupOwner.owner !== selfOwner
}

// Cross-monitor transfer: new owner takes popup (requestPopup always transfers).
function popupOwnerId(popup) {
  return popup ? popup.owner : null
}

// Popup open for this owner only (UX-021 / UX-022).
function popupOpenForOwner(popupOwner, owner) {
  if (!popupOwner || owner === null || owner === undefined)
    return false
  return popupOwner.owner === owner
}

function popupView(popupOwner) {
  if (!popupOwner || !popupOwner.view)
    return "usage"
  return String(popupOwner.view)
}

// ---------------------------------------------------------------------------
// Lane guards: never start same-lane exec while running
// ---------------------------------------------------------------------------

function canStartLane(laneBusy) {
  return !laneBusy
}

// ---------------------------------------------------------------------------
// Default settings (shared primitive: consumed by CoreSettings and QML)
// ---------------------------------------------------------------------------

// Poll timer interval from applied settings. The SET-005 range (30..3600) is
// enforced by validation before anything reaches appliedSettings; null means
// no settings applied yet and falls back to the 60 s default.
function pollIntervalMs(settings) {
  if (settings && settings.refreshIntervalSeconds)
    return Number(settings.refreshIntervalSeconds) * 1000
  return 60000
}

function defaultSettings() {
  return {
    schemaVersion: 1,
    providers: [
      { id: "claude", enabled: true },
      { id: "codex", enabled: true },
      { id: "amp", enabled: true },
      { id: "grok", enabled: true }
    ],
    display: { metric: "remaining" },
    refreshIntervalSeconds: 60,
    notifications: { enabled: true, reminderMinutes: 120 }
  }
}
