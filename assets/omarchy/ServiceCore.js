// Pure Agent Bar service logic — free of Quickshell.Io for qmltestrunner.
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

// Cross-monitor transfer: new owner takes popup (requestPopup always transfers).
function popupOwnerId(popup) {
  return popup ? popup.owner : null
}

// ---------------------------------------------------------------------------
// Settings state machine (SET-014..): closed → loading → clean → dirty → saving
// ---------------------------------------------------------------------------

function settingsClosed() {
  return { phase: "closed", generation: 0, snapshot: null, draft: null, busy: false }
}

function settingsOpen(state, snapshot, generation) {
  return {
    phase: "clean",
    generation: generation,
    snapshot: snapshot,
    draft: JSON.parse(JSON.stringify(snapshot)),
    busy: false
  }
}

function cloneState(state) {
  return {
    phase: state.phase,
    generation: state.generation,
    snapshot: state.snapshot,
    draft: state.draft,
    busy: state.busy
  }
}

function settingsMarkDirty(state) {
  if (!state || state.phase === "closed" || state.phase === "loading")
    return state
  var next = cloneState(state)
  next.phase = "dirty"
  return next
}

function settingsBeginSave(state, generation) {
  if (!state || state.phase === "closed")
    return state
  if (state.generation !== generation)
    return state
  var next = cloneState(state)
  next.phase = "saving"
  next.busy = true
  return next
}

// Apply only when generation still matches.
function settingsFinishSave(state, generation, ok, canonical) {
  if (!state || state.generation !== generation)
    return state
  var next = cloneState(state)
  next.busy = false
  if (ok) {
    next.snapshot = canonical
    next.draft = JSON.parse(JSON.stringify(canonical))
    next.phase = "clean"
  } else {
    next.phase = "dirty"
  }
  return next
}

function settingsCancel(state) {
  if (!state || state.phase === "closed")
    return state
  var next = cloneState(state)
  next.draft = JSON.parse(JSON.stringify(state.snapshot))
  next.phase = "clean"
  next.busy = false
  return next
}

// ---------------------------------------------------------------------------
// Maintenance state
// ---------------------------------------------------------------------------

function maintenanceIdle() {
  return { phase: "idle", blocked: false }
}

function maintenanceBeginHandoff(state) {
  return { phase: "handoff", blocked: true }
}

function maintenanceCanStartWrite(maint) {
  return !maint || !maint.blocked
}

// Drain rule: handoff waits while status or settingsWrite busy.
function maintenanceCanDetach(maint, statusBusy, settingsWriteBusy) {
  if (!maint || maint.phase !== "handoff")
    return false
  return !statusBusy && !settingsWriteBusy
}

// ---------------------------------------------------------------------------
// Lane guards: never start same-lane exec while running
// ---------------------------------------------------------------------------

function canStartLane(laneBusy) {
  return !laneBusy
}

// ---------------------------------------------------------------------------
// Bar chips (UX-001..012) — pure model helpers for BarWidget / ProviderChip
// ---------------------------------------------------------------------------

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
    notifications: { enabled: true }
  }
}

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
    if (!CLOSED_PROVIDERS[id])
      continue
    if (byId[id])
      out.push(byId[id])
    else
      out.push(placeholderProvider(id))
  }
  return out
}

function primaryWindow(provider) {
  if (!provider || !Array.isArray(provider.windows) || provider.windows.length === 0)
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
    return " stale"
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
function chipTooltip(provider, metric) {
  if (!provider)
    return ""
  var name = provider.name ? String(provider.name) : providerDisplayName(provider.id)
  var pct = chipPercentText(provider, metric)
  var state = provider.state ? String(provider.state) : "unknown"
  var parts = [name, pct, state]
  var w = primaryWindow(provider)
  if (w && w.resetsAt) {
    var label = w.label ? String(w.label) : String(w.id || "window")
    parts.push("resets " + label + " " + String(w.resetsAt))
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

  return out
}

function mapActionKind(kind) {
  var k = String(kind || "")
  if (!ACTION_INTENTS[k])
    return null
  return k
}

function windowDisplayLines(provider, metric) {
  var lines = []
  if (!provider || !Array.isArray(provider.windows))
    return lines
  var mode = metric === "used" ? "used" : "remaining"
  for (var i = 0; i < provider.windows.length; i++) {
    var w = provider.windows[i]
    if (!w)
      continue
    var pct = mode === "used" ? Number(w.usedPercent) : Number(w.remainingPercent)
    var pctText = isFinite(pct) ? (Math.round(pct) + "%") : "\u2014"
    lines.push({
      id: String(w.id || ("w" + i)),
      label: plainText(w.label || w.id || "Window"),
      percentText: pctText,
      resetsAt: w.resetsAt ? String(w.resetsAt) : null
    })
  }
  return lines
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
