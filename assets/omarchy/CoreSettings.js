// Settings state machine + draft mutations/validation.
.pragma library
.import "CoreService.js" as Kernel

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
  next.draft = Kernel.defaultSettings()
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
  return JSON.parse(JSON.stringify(draft || Kernel.defaultSettings()))
}

function setProviderEnabled(draft, providerId, enabled) {
  var next = cloneDraft(draft)
  var id = String(providerId || "")
  if (!Array.isArray(next.providers))
    next.providers = Kernel.defaultSettings().providers
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
    if (!p || !Kernel.CLOSED_PROVIDERS[String(p.id)])
      return { ok: false, reason: "provider id" }
    if (seen[p.id])
      return { ok: false, reason: "duplicate provider" }
    seen[p.id] = true
    if (typeof p.enabled !== "boolean")
      return { ok: false, reason: "provider enabled" }
  }
  for (var id in Kernel.CLOSED_PROVIDERS) {
    if (!seen[id])
      return { ok: false, reason: "missing provider" }
  }
  return { ok: true, reason: null }
}

// Startup bootstrap (SET-023): decide what appliedSettings becomes after the
// boot-time `config show`. A dialog read/save that finished first is newer
// than the boot read and wins; any failure keeps in-memory defaults (SET-008)
// and never touches the dialog state machine (SET-014..022 own their own
// snapshot and generations).
function settingsBootstrapResult(currentApplied, stdout, exitCode) {
  if (currentApplied)
    return currentApplied
  if (exitCode !== 0)
    return null
  try {
    var doc = JSON.parse(String(stdout || "").trim())
    if (!validateSettingsDraft(doc).ok)
      return null
    return doc
  } catch (e) {
    return null
  }
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
