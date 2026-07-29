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

// ---------------------------------------------------------------------------
// Settings state machine (SET-014..): closed → loading → clean → dirty → saving
// ---------------------------------------------------------------------------

function settingsClosed() {
  return {
    phase: "closed",
    generation: 0,
    snapshot: null,
    draft: null,
    busy: false,
    pendingPayload: null
  }
}

// Begin load on open — controls locked until config show completes (SET-014/015).
function settingsBeginLoad(generation) {
  return {
    phase: "loading",
    generation: generation,
    snapshot: null,
    draft: null,
    busy: true,
    pendingPayload: null
  }
}

// Successful config show → clean draft/snapshot. Generation must match.
function settingsFinishLoad(state, generation, doc) {
  if (!state || state.generation !== generation)
    return state
  if (state.phase === "closed")
    return state
  if (state.phase !== "loading" && state.phase !== "clean")
    return state
  var copy = JSON.parse(JSON.stringify(doc))
  return {
    phase: "clean",
    generation: generation,
    snapshot: copy,
    draft: JSON.parse(JSON.stringify(copy)),
    busy: false,
    pendingPayload: null
  }
}

// Legacy alias used by older harnesses: open directly into clean with a doc.
function settingsOpen(state, snapshot, generation) {
  return {
    phase: "clean",
    generation: generation,
    snapshot: snapshot,
    draft: JSON.parse(JSON.stringify(snapshot)),
    busy: false,
    pendingPayload: null
  }
}

function cloneState(state) {
  return {
    phase: state.phase,
    generation: state.generation,
    snapshot: state.snapshot,
    draft: state.draft,
    busy: state.busy,
    pendingPayload: state.pendingPayload
  }
}

function settingsControlsLocked(state) {
  if (!state)
    return true
  return state.phase === "closed" || state.phase === "loading" || state.phase === "saving" || !!state.busy
}

function settingsMarkDirty(state) {
  if (!state || state.phase === "closed" || state.phase === "loading" || state.phase === "saving")
    return state
  var next = cloneState(state)
  next.phase = "dirty"
  next.draft = state.draft
  return next
}

// SET-016/018: save receives a new generation; payload is immutable capture.
function settingsBeginSave(state, generation, payload) {
  if (!state || state.phase === "closed" || state.phase === "loading")
    return state
  if (state.phase === "saving")
    return state
  var next = cloneState(state)
  next.generation = generation
  next.phase = "saving"
  next.busy = true
  next.pendingPayload = payload
  return next
}

// Apply only when generation matches and a save is in flight (SET-017 / SET-021).
function settingsFinishSave(state, generation, ok, canonical) {
  if (!state || state.generation !== generation)
    return state
  if (state.phase !== "saving")
    return state
  var next = cloneState(state)
  next.busy = false
  next.pendingPayload = null
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
  if (!state || state.phase === "closed" || state.phase === "loading")
    return state
  if (state.phase === "saving")
    return state
  if (!state.snapshot)
    return state
  var next = cloneState(state)
  next.draft = JSON.parse(JSON.stringify(state.snapshot))
  next.phase = "clean"
  next.busy = false
  return next
}

// SET-022: restore defaults mutates draft only.
function settingsRestoreDefaults(state) {
  if (!state || state.phase === "closed" || state.phase === "loading" || state.phase === "saving")
    return state
  var next = cloneState(state)
  next.draft = defaultSettings()
  next.phase = "dirty"
  return next
}

// Keep settings machine across popup hide while load/save in flight (SET-019/020).
function settingsShouldRetainOnClose(state) {
  if (!state)
    return false
  return state.phase === "loading" || state.phase === "saving" || !!state.busy
}

// ---------------------------------------------------------------------------
// Settings draft mutations + validation
// ---------------------------------------------------------------------------

function cloneDraft(draft) {
  return JSON.parse(JSON.stringify(draft || defaultSettings()))
}

function setProviderEnabled(draft, providerId, enabled) {
  var next = cloneDraft(draft)
  var id = String(providerId || "")
  if (!Array.isArray(next.providers))
    next.providers = defaultSettings().providers
  for (var i = 0; i < next.providers.length; i++) {
    if (String(next.providers[i].id) === id) {
      next.providers[i].enabled = !!enabled
      break
    }
  }
  return next
}

function moveProvider(draft, providerId, delta) {
  var next = cloneDraft(draft)
  var id = String(providerId || "")
  if (!Array.isArray(next.providers))
    return next
  var idx = -1
  for (var i = 0; i < next.providers.length; i++) {
    if (String(next.providers[i].id) === id) {
      idx = i
      break
    }
  }
  if (idx < 0)
    return next
  var target = idx + (delta > 0 ? 1 : -1)
  if (target < 0 || target >= next.providers.length)
    return next
  var tmp = next.providers[idx]
  next.providers[idx] = next.providers[target]
  next.providers[target] = tmp
  return next
}

function setDisplayMetric(draft, metric) {
  var next = cloneDraft(draft)
  if (!next.display)
    next.display = { metric: "remaining" }
  next.display.metric = metric === "used" ? "used" : "remaining"
  return next
}

function setRefreshInterval(draft, seconds) {
  var next = cloneDraft(draft)
  var n = Math.round(Number(seconds))
  if (!isFinite(n))
    n = 60
  next.refreshIntervalSeconds = n
  return next
}

function setNotificationsEnabled(draft, enabled) {
  var next = cloneDraft(draft)
  if (!next.notifications)
    next.notifications = { enabled: true }
  next.notifications.enabled = !!enabled
  return next
}

function validateSettingsDraft(draft) {
  if (!draft || typeof draft !== "object")
    return { ok: false, reason: "not an object" }
  if (draft.schemaVersion !== 1)
    return { ok: false, reason: "schemaVersion" }
  if (!draft.display || (draft.display.metric !== "used" && draft.display.metric !== "remaining"))
    return { ok: false, reason: "display.metric" }
  var interval = Number(draft.refreshIntervalSeconds)
  if (!isFinite(interval) || interval !== Math.floor(interval) || interval < 30 || interval > 3600)
    return { ok: false, reason: "refreshIntervalSeconds" }
  if (!draft.notifications || typeof draft.notifications.enabled !== "boolean")
    return { ok: false, reason: "notifications" }
  if (!Array.isArray(draft.providers) || draft.providers.length !== 4)
    return { ok: false, reason: "providers length" }
  var seen = {}
  for (var i = 0; i < draft.providers.length; i++) {
    var p = draft.providers[i]
    if (!p || !CLOSED_PROVIDERS[String(p.id)])
      return { ok: false, reason: "provider id" }
    if (seen[p.id])
      return { ok: false, reason: "duplicate provider" }
    seen[p.id] = true
    if (typeof p.enabled !== "boolean")
      return { ok: false, reason: "provider enabled" }
  }
  for (var id in CLOSED_PROVIDERS) {
    if (!seen[id])
      return { ok: false, reason: "missing provider" }
  }
  return { ok: true, reason: null }
}

function settingsCanSave(state, draft) {
  if (!state)
    return false
  if (state.phase !== "dirty" && state.phase !== "clean")
    return false
  if (state.busy || state.phase === "saving" || state.phase === "loading")
    return false
  // Allow save from dirty only (or clean if user re-saves — disable when clean)
  if (state.phase !== "dirty")
    return false
  return validateSettingsDraft(draft).ok
}

function settingsArgvShow(helperPath) {
  return [String(helperPath), "config", "show"]
}

function settingsArgvApplyStdin(helperPath) {
  return [String(helperPath), "config", "apply", "stdin"]
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
// Login / maintenance UI (Task 13 — UX-040..048)
// ---------------------------------------------------------------------------

function loginDetachedArgv(pluginRoot, providerId) {
  if (!pluginRoot || !String(pluginRoot).length)
    return null
  if (!isClosedProvider(providerId))
    return null
  return [
    String(pluginRoot) + "/scripts/agent-bar-open-terminal",
    "login",
    String(providerId)
  ]
}

// Exact xdg-terminal-exec argv the Bash helper must exec (ARCH login flow).
function terminalHelperXdgArgv(pluginRoot, providerId) {
  if (!pluginRoot || !isClosedProvider(providerId))
    return null
  return [
    "xdg-terminal-exec",
    "--app-id=org.omarchy.terminal",
    "--title=Agent Bar Login",
    "--",
    String(pluginRoot) + "/bin/agent-bar",
    "login",
    String(providerId)
  ]
}

function updateCheckArgv(helperPath) {
  return [String(helperPath), "update", "check"]
}

function updateApplyArgv(helperPath, version) {
  var v = String(version || "")
  if (!v.length)
    return null
  return [String(helperPath), "update", "apply", v]
}

function uninstallArgv(helperPath, purge) {
  if (purge)
    return [String(helperPath), "uninstall", "purge"]
  return [String(helperPath), "uninstall"]
}

// Non-TTY uninstall confirmation document (CLI contract).
function uninstallConfirmation(purge) {
  return {
    schemaVersion: 1,
    operation: "uninstall",
    confirmed: true,
    purgeSettingsAndBackups: !!purge
  }
}

function maintenanceUiIdle(installedVersion) {
  return {
    phase: "idle",
    installedVersion: installedVersion ? String(installedVersion) : "",
    installType: "Plugin bundle",
    targetVersion: "",
    releaseNotesUrl: "",
    purgeSettings: false,
    uninstallArmed: false,
    message: "",
    updateConfirmOpen: false,
    uninstallConfirmOpen: false
  }
}

function maintenanceUiChecking(ui) {
  var next = cloneMaintenanceUi(ui)
  next.phase = "checking"
  next.message = "Checking for updates\u2026"
  next.updateConfirmOpen = false
  return next
}

function cloneMaintenanceUi(ui) {
  return {
    phase: ui && ui.phase ? ui.phase : "idle",
    installedVersion: ui && ui.installedVersion ? String(ui.installedVersion) : "",
    installType: "Plugin bundle",
    targetVersion: ui && ui.targetVersion ? String(ui.targetVersion) : "",
    releaseNotesUrl: ui && ui.releaseNotesUrl ? String(ui.releaseNotesUrl) : "",
    purgeSettings: !!(ui && ui.purgeSettings),
    uninstallArmed: !!(ui && ui.uninstallArmed),
    message: ui && ui.message ? String(ui.message) : "",
    updateConfirmOpen: !!(ui && ui.updateConfirmOpen),
    uninstallConfirmOpen: !!(ui && ui.uninstallConfirmOpen)
  }
}

// Parse update check stdout. Accepts either:
// { "updateAvailable": true, "currentVersion", "targetVersion", "releaseNotesUrl" }
// or empty/up-to-date markers.
function maintenanceUiFromCheck(ui, stdout, exitCode, fallbackVersion) {
  var next = cloneMaintenanceUi(ui)
  next.updateConfirmOpen = false
  if (exitCode !== 0) {
    next.phase = "error"
    next.message = "Update check failed."
    return next
  }
  var text = String(stdout || "").trim()
  if (!text.length || text.indexOf("up to date") >= 0) {
    next.phase = "up_to_date"
    next.targetVersion = ""
    next.releaseNotesUrl = ""
    next.message = "Agent Bar is up to date."
    return next
  }
  try {
    var doc = JSON.parse(text)
    if (doc && doc.updateAvailable === false) {
      next.phase = "up_to_date"
      next.message = "Agent Bar is up to date."
      if (doc.currentVersion)
        next.installedVersion = String(doc.currentVersion)
      return next
    }
    if (doc && (doc.updateAvailable === true || doc.targetVersion)) {
      next.phase = "update_available"
      next.installedVersion = String(doc.currentVersion || next.installedVersion || fallbackVersion || "")
      next.targetVersion = String(doc.targetVersion || doc.version || "")
      next.releaseNotesUrl = doc.releaseNotesUrl ? String(doc.releaseNotesUrl) : ""
      next.message = next.targetVersion.length
          ? ("Update to " + next.targetVersion + " is available.")
          : "An update is available."
      return next
    }
  } catch (e) {
    // fall through
  }
  next.phase = "error"
  next.message = "Update check returned an unusable response."
  return next
}

function maintenanceUiOpenUpdateConfirm(ui) {
  var next = cloneMaintenanceUi(ui)
  if (next.phase !== "update_available" || !next.targetVersion.length)
    return next
  next.updateConfirmOpen = true
  return next
}

function maintenanceUiCloseUpdateConfirm(ui) {
  var next = cloneMaintenanceUi(ui)
  next.updateConfirmOpen = false
  return next
}

function updateConfirmMessage(ui) {
  var current = ui && ui.installedVersion ? String(ui.installedVersion) : "current"
  var target = ui && ui.targetVersion ? String(ui.targetVersion) : "new"
  return "Update Agent Bar from " + current + " to " + target
      + ". This replaces the plugin bundle, preserves settings, and can roll back on failure."
}

function maintenanceUiOpenUninstallConfirm(ui) {
  var next = cloneMaintenanceUi(ui)
  next.uninstallConfirmOpen = true
  next.uninstallArmed = false
  next.purgeSettings = false
  return next
}

function maintenanceUiCloseUninstallConfirm(ui) {
  var next = cloneMaintenanceUi(ui)
  next.uninstallConfirmOpen = false
  next.uninstallArmed = false
  return next
}

function maintenanceUiSetPurge(ui, purge) {
  var next = cloneMaintenanceUi(ui)
  next.purgeSettings = !!purge
  // Changing purge resets the second destructive arm (UX-047 safety).
  next.uninstallArmed = false
  return next
}

// First destructive click arms; second confirms (UX-047).
function maintenanceUiArmOrConfirmUninstall(ui) {
  var next = cloneMaintenanceUi(ui)
  if (!next.uninstallConfirmOpen)
    return { ui: next, confirmed: false }
  if (!next.uninstallArmed) {
    next.uninstallArmed = true
    next.message = "Click Uninstall again to confirm."
    return { ui: next, confirmed: false }
  }
  return { ui: next, confirmed: true }
}

function maintenanceUiApplying(ui) {
  var next = cloneMaintenanceUi(ui)
  next.phase = "applying"
  next.updateConfirmOpen = false
  next.message = "Applying update\u2026"
  return next
}

function maintenanceUiUninstalling(ui) {
  var next = cloneMaintenanceUi(ui)
  next.phase = "uninstalling"
  next.uninstallConfirmOpen = false
  next.message = "Uninstalling\u2026"
  return next
}

function maintenanceIntention(kind, ui) {
  if (kind === "update_apply") {
    return {
      kind: "update_apply",
      version: ui && ui.targetVersion ? String(ui.targetVersion) : "",
      payload: null
    }
  }
  if (kind === "uninstall") {
    return {
      kind: "uninstall",
      purge: !!(ui && ui.purgeSettings),
      payload: uninstallConfirmation(!!(ui && ui.purgeSettings))
    }
  }
  return null
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
  if (!provider || !isArrayLike(provider.windows) || provider.windows.length === 0)
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

function windowDisplayLines(provider, metric) {
  var lines = []
  if (!provider || !isArrayLike(provider.windows))
    return lines
  var mode = metric === "used" ? "used" : "remaining"
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

// ---------------------------------------------------------------------------
// Focus / keyboard / scroll (A11Y-002..023)
// ---------------------------------------------------------------------------

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

// Screenshot inventory required by TEST/CP2 (exact basenames).
function requiredScreenshotNames() {
  return [
    "ready-light.png",
    "ready-dark.png",
    "loading-dark.png",
    "refreshing-with-data-dark.png",
    "stale-dark.png",
    "cli-missing-dark.png",
    "unauthenticated-dark.png",
    "rate-limited-dark.png",
    "network-error-dark.png",
    "provider-error-dark.png",
    "settings-clean-dark.png",
    "settings-dirty-dark.png",
    "settings-invalid-dark.png",
    "maintenance-update-dark.png",
    "uninstall-confirmation-dark.png"
  ]
}

function themePalette(mode) {
  if (mode === "light") {
    return {
      mode: "light",
      background: "#f4f4f5",
      foreground: "#18181b",
      muted: "#52525b",
      border: "#d4d4d8",
      urgent: "#b91c1c"
    }
  }
  return {
    mode: "dark",
    background: "#18181b",
    foreground: "#e4e4e7",
    muted: "#a1a1aa",
    border: "#3f3f46",
    urgent: "#f87171"
  }
}
