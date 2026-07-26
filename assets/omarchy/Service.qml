import QtQuick
import Quickshell.Io
import "ServiceCore.js" as Core

// Shared Agent Bar service — one instance per shell (ARCH-023 / ARCH-024).
Item {
  id: root

  // --- Quattro injection ---
  property string omarchyPath: ""
  property var shell: null
  property var manifest: null
  property var barWidgetRegistry: null
  property var pluginRegistry: null

  readonly property string pluginRoot: manifest && manifest.__sourceDir
      ? String(manifest.__sourceDir)
      : ""

  // Test harness: absolute helper path. Production uses pluginRoot/bin/agent-bar.
  property string helperPath: ""
  // Skip auto Process start; tests drive apply* methods.
  property bool testMode: false
  property int versionProbeTimeoutMs: 2000
  property int statusTimeoutMs: 60000
  property int pollIntervalMs: 60000
  property int collectionDelayMs: 0

  // --- Public service surface (Task 9) ---
  property var snapshot: null
  property bool refreshing: false
  property string selectedProviderId: ""
  property var popupOwner: null // { owner, providerId, view } or null
  property var settingsState: Core.settingsClosed()
  property var settingsDraft: null
  property var maintenanceState: Core.maintenanceIdle()
  property var pendingForcedTargets: Core.emptyPending()

  // Version probe
  property string helperVersion: ""
  property bool versionReady: false
  property bool versionFailed: false
  property bool versionProbeRunning: false
  property bool collectionStarted: false

  // Lane busy flags (one Process per lane; never re-exec while running)
  property bool statusBusy: false
  property bool settingsReadBusy: false
  property bool settingsWriteBusy: false
  property bool maintenanceCheckBusy: false
  property bool maintenanceHandoffBusy: false

  // Generation counters for stale-callback rejection
  property int statusGeneration: 0
  property int settingsGeneration: 0
  property int activeStatusGeneration: 0
  property int activeSettingsWriteGeneration: 0

  // Bookkeeping
  property int refreshRequestCount: 0
  property string lastRefreshProviderId: ""
  property int statusStartCount: 0
  property bool pollEnabled: true

  readonly property string manifestVersion: manifest && manifest.version
      ? String(manifest.version)
      : ""

  // -------------------------------------------------------------------------
  // Paths / IPC
  // -------------------------------------------------------------------------

  function resolvedHelperPath() {
    if (helperPath && helperPath.length > 0)
      return helperPath
    if (pluginRoot && pluginRoot.length > 0)
      return pluginRoot + "/bin/agent-bar"
    return ""
  }

  function health(expectedVersion) {
    return Core.health(versionReady, versionFailed, helperVersion, manifestVersion, expectedVersion)
  }

  // IPC refresh(providerId) — queue one cache-bypass provider refresh.
  function refresh(providerId) {
    var result = Core.refreshResult(providerId)
    if (result !== "ok")
      return result
    lastRefreshProviderId = String(providerId)
    refreshRequestCount++
    refreshProvider(String(providerId), true)
    return "ok"
  }

  // -------------------------------------------------------------------------
  // Public methods
  // -------------------------------------------------------------------------

  function refreshAll(force) {
    if (maintenanceState.blocked)
      return
    if (force)
      pendingForcedTargets = Core.unionForced(pendingForcedTargets, "all")
    kickStatus()
  }

  function refreshProvider(providerId, force) {
    if (maintenanceState.blocked)
      return
    if (!Core.isClosedProvider(providerId))
      return
    if (force)
      pendingForcedTargets = Core.unionForced(pendingForcedTargets, providerId)
    kickStatus()
  }

  function requestPopup(owner, providerId, view) {
    popupOwner = Core.requestPopup(popupOwner, owner, providerId, view)
    if (providerId)
      selectedProviderId = String(providerId)
  }

  function closePopup(owner) {
    popupOwner = Core.closePopup(popupOwner, owner)
  }

  function openSettings(owner) {
    if (maintenanceState.blocked)
      return
    requestPopup(owner, selectedProviderId || null, "settings")
    // Capture immutable snapshot when settings open (Task 12 expands apply).
    if (settingsState.phase === "closed") {
      settingsGeneration++
      var snap = snapshot ? JSON.parse(JSON.stringify(snapshot)) : ({
        schemaVersion: 1,
        providers: []
      })
      settingsState = Core.settingsOpen(settingsState, snap, settingsGeneration)
      settingsDraft = settingsState.draft
      kickSettingsRead()
    }
  }

  // -------------------------------------------------------------------------
  // Version probe lane
  // -------------------------------------------------------------------------

  function applyVersionProbeResult(stdout, stderr, exitCode) {
    var version = Core.parseVersionStdout(stdout, stderr, exitCode)
    if (version)
      finishVersionProbeSuccess(version)
    else
      finishVersionProbeFailure()
  }

  function startVersionProbe() {
    if (versionProbeRunning || versionReady)
      return
    if (testMode)
      return
    if (!Core.canStartLane(versionProbeRunning))
      return
    var helper = resolvedHelperPath()
    if (!helper.length) {
      finishVersionProbeFailure()
      return
    }
    versionProbeRunning = true
    versionFailed = false
    versionOut.text = ""
    versionErr.text = ""
    versionProbe.command = [helper, "version"]
    versionProbe.running = true
    versionTimeout.restart()
  }

  function finishVersionProbeSuccess(versionText) {
    versionTimeout.stop()
    versionProbeRunning = false
    helperVersion = versionText
    versionReady = true
    versionFailed = false
    if (collectionDelayMs > 0) {
      collectionDelay.interval = collectionDelayMs
      collectionDelay.start()
    } else {
      beginCollection()
    }
  }

  function finishVersionProbeFailure() {
    versionTimeout.stop()
    versionProbeRunning = false
    versionReady = false
    versionFailed = true
    helperVersion = ""
  }

  function beginCollection() {
    collectionStarted = true
    pollTimer.restart()
    kickStatus()
  }

  // -------------------------------------------------------------------------
  // Status lane
  // -------------------------------------------------------------------------

  function kickStatus() {
    if (!versionReady || versionFailed)
      return
    if (maintenanceState.blocked)
      return
    if (!Core.canStartLane(statusBusy))
      return
    var helper = resolvedHelperPath()
    if (!helper.length)
      return

    statusGeneration++
    var gen = statusGeneration
    activeStatusGeneration = gen
    var targets = Core.takePending(pendingForcedTargets)
    pendingForcedTargets = targets.remaining
    var argv = Core.statusArgv(helper, targets.captured)
    var request = {
      generation: gen,
      argv: argv.slice(),
      forced: targets.captured
    }

    statusBusy = true
    refreshing = true
    statusStartCount++
    statusOut.text = ""
    statusErr.text = ""
    if (testMode) {
      // Tests call applyStatusResult(gen, stdout, stderr, code)
      return
    }
    statusProcess.command = argv
    statusProcess.running = true
    statusTimeout.restart()
  }

  function applyStatusResult(generation, stdout, stderr, exitCode) {
    if (!Core.shouldApplyGeneration(activeStatusGeneration, generation))
      return
    statusTimeout.stop()
    statusBusy = false
    refreshing = false

    if (exitCode !== 0) {
      // Keep last snapshot (CACHE-021 / malformed retention).
      maybeFollowUpStatus()
      return
    }
    var parsed = Core.parseStatusEnvelope(stdout, helperVersion)
    if (!parsed.ok) {
      // Malformed envelope: retain previous snapshot.
      maybeFollowUpStatus()
      return
    }
    // Immutable replacement.
    snapshot = parsed.envelope
    maybeFollowUpStatus()
  }

  function maybeFollowUpStatus() {
    if (!pendingIsEmptySafe())
      kickStatus()
  }

  function pendingIsEmptySafe() {
    return Core.pendingIsEmpty(pendingForcedTargets)
  }

  // -------------------------------------------------------------------------
  // Settings lanes (read / write) — skeleton; full store in Task 12
  // -------------------------------------------------------------------------

  function kickSettingsRead() {
    // ARCH-024: maintenance rejects new writes/polling; reads may still start.
    if (!Core.canStartLane(settingsReadBusy))
      return
    var helper = resolvedHelperPath()
    if (!helper.length)
      return
    settingsReadBusy = true
    if (testMode)
      return
    settingsReadProcess.command = [helper, "config", "show"]
    settingsReadProcess.running = true
  }

  function applySettingsReadResult(stdout, exitCode) {
    settingsReadBusy = false
    if (exitCode !== 0 || !settingsState || settingsState.phase === "closed")
      return
    try {
      var doc = JSON.parse(String(stdout || "").trim())
      settingsState = Core.settingsOpen(settingsState, doc, settingsState.generation)
      settingsDraft = settingsState.draft
    } catch (e) {
      // keep existing draft
    }
  }

  function applySettingsWriteResult(generation, ok, canonical) {
    settingsWriteBusy = false
    settingsState = Core.settingsFinishSave(settingsState, generation, ok, canonical)
    settingsDraft = settingsState ? settingsState.draft : null
    tryMaintenanceDetach()
  }

  // -------------------------------------------------------------------------
  // Maintenance handoff
  // -------------------------------------------------------------------------

  function beginMaintenanceHandoff() {
    maintenanceState = Core.maintenanceBeginHandoff(maintenanceState)
    pollEnabled = false
    pollTimer.stop()
    tryMaintenanceDetach()
  }

  function tryMaintenanceDetach() {
    if (!Core.maintenanceCanDetach(maintenanceState, statusBusy, settingsWriteBusy))
      return
    if (!Core.canStartLane(maintenanceHandoffBusy))
      return
    maintenanceHandoffBusy = true
    if (testMode)
      return
    // Detached worker placeholder — Task 16/17 wires real maintenance argv.
    maintenanceHandoffProcess.command = [resolvedHelperPath(), "doctor", "scan"]
    maintenanceHandoffProcess.running = true
  }

  function applyMaintenanceHandoffDone() {
    maintenanceHandoffBusy = false
    maintenanceState = Core.maintenanceIdle()
    pollEnabled = true
    if (versionReady)
      pollTimer.restart()
  }

  // -------------------------------------------------------------------------
  // Processes (isolated lanes)
  // -------------------------------------------------------------------------

  Process {
    id: versionProbe
    stdout: StdioCollector { id: versionOut; waitForEnd: true }
    stderr: StdioCollector { id: versionErr; waitForEnd: true }
    onExited: function (exitCode) {
      if (!root.versionProbeRunning)
        return
      root.applyVersionProbeResult(versionOut.text || "", versionErr.text || "", exitCode)
    }
  }

  Process {
    id: statusProcess
    stdout: StdioCollector { id: statusOut; waitForEnd: true }
    stderr: StdioCollector { id: statusErr; waitForEnd: true }
    onExited: function (exitCode) {
      root.applyStatusResult(root.activeStatusGeneration, statusOut.text || "", statusErr.text || "", exitCode)
    }
  }

  Process {
    id: settingsReadProcess
    stdout: StdioCollector { id: settingsReadOut; waitForEnd: true }
    stderr: StdioCollector { id: settingsReadErr; waitForEnd: true }
    onExited: function (exitCode) {
      root.applySettingsReadResult(settingsReadOut.text || "", exitCode)
    }
  }

  Process {
    id: settingsWriteProcess
    stdout: StdioCollector { id: settingsWriteOut; waitForEnd: true }
    stderr: StdioCollector { id: settingsWriteErr; waitForEnd: true }
    onExited: function (exitCode) {
      var ok = exitCode === 0
      var canonical = null
      if (ok) {
        try { canonical = JSON.parse(String(settingsWriteOut.text || "").trim()) } catch (e) { ok = false }
      }
      root.applySettingsWriteResult(root.activeSettingsWriteGeneration, ok, canonical)
    }
  }

  Process {
    id: maintenanceCheckProcess
    stdout: StdioCollector { id: maintenanceCheckOut; waitForEnd: true }
    stderr: StdioCollector { id: maintenanceCheckErr; waitForEnd: true }
    onExited: function () {
      root.maintenanceCheckBusy = false
    }
  }

  Process {
    id: maintenanceHandoffProcess
    stdout: StdioCollector { id: maintenanceHandoffOut; waitForEnd: true }
    stderr: StdioCollector { id: maintenanceHandoffErr; waitForEnd: true }
    onExited: function () {
      root.applyMaintenanceHandoffDone()
    }
  }

  Timer {
    id: versionTimeout
    interval: root.versionProbeTimeoutMs
    repeat: false
    onTriggered: {
      if (!root.versionProbeRunning)
        return
      if (versionProbe.running)
        versionProbe.running = false
      root.finishVersionProbeFailure()
    }
  }

  Timer {
    id: statusTimeout
    interval: root.statusTimeoutMs
    repeat: false
    onTriggered: {
      if (!root.statusBusy)
        return
      if (statusProcess.running)
        statusProcess.running = false
      root.applyStatusResult(root.activeStatusGeneration, "", "timeout", 1)
    }
  }

  Timer {
    id: collectionDelay
    repeat: false
    onTriggered: root.beginCollection()
  }

  // One automatic poll timer (CACHE-005).
  Timer {
    id: pollTimer
    interval: root.pollIntervalMs
    repeat: true
    running: false
    onTriggered: {
      if (!root.pollEnabled || root.maintenanceState.blocked)
        return
      root.kickStatus()
    }
  }

  IpcHandler {
    target: "agent-bar.usage"
    function health(expectedVersion): string { return root.health(expectedVersion) }
    function refresh(providerId): string { return root.refresh(providerId) }
  }

  Component.onCompleted: startVersionProbe()
}
