import QtQuick
import Quickshell
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

  // --- Public service surface (Task 9 / Task 10) ---
  property var snapshot: null
  property bool refreshing: false
  property string selectedProviderId: ""
  property var popupOwner: null // { owner, providerId, view } or null
  property var settingsState: Core.settingsClosed()
  property var settingsDraft: null
  // Applied product settings for bar chips (order / metric). Null → defaults.
  property var appliedSettings: null
  property var maintenanceState: Core.maintenanceIdle()
  property var maintenanceUi: Core.maintenanceUiIdle("")
  property var pendingForcedTargets: Core.emptyPending()
  // Login bookkeeping + last detached argv (testable without exec).
  property int loginRequestCount: 0
  property string lastLoginProviderId: ""
  property var lastLoginArgv: null
  property string lastViewInstallationUrl: ""
  // Pending maintenance intention after confirm (update apply / uninstall).
  property var pendingMaintenanceIntention: null
  property string pendingMaintenancePayload: ""

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
  property int activeSettingsReadGeneration: 0
  property int activeSettingsWriteGeneration: 0
  // Immutable captured JSON for the in-flight config apply (SET-018).
  property string pendingSettingsPayload: ""

  // Bookkeeping
  property int refreshRequestCount: 0
  property string lastRefreshProviderId: ""
  property int statusStartCount: 0
  property int settingsSaveCount: 0
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
    // Prefer live manifest.__sourceDir over pluginRoot: Quattro sets `manifest`
    // after createObject, and onManifestChanged can run before the pluginRoot
    // binding re-evaluates (nested JS field access is not a QML property).
    var root = ""
    if (manifest && manifest.__sourceDir)
      root = String(manifest.__sourceDir)
    else if (pluginRoot && pluginRoot.length > 0)
      root = pluginRoot
    if (root && root.length > 0)
      return root + "/bin/agent-bar"
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
    // SET-019: closing never fabricates load/save completion.
    if (!popupOwner && !Core.settingsShouldRetainOnClose(settingsState)) {
      settingsState = Core.settingsClosed()
      settingsDraft = null
    }
  }

  function openSettings(owner) {
    if (maintenanceState.blocked)
      return
    requestPopup(owner, selectedProviderId || null, "settings")
    // SET-014/020: only start a new load when closed; reopen during save keeps busy.
    if (!settingsState || settingsState.phase === "closed") {
      settingsGeneration++
      activeSettingsReadGeneration = settingsGeneration
      settingsState = Core.settingsBeginLoad(settingsGeneration)
      settingsDraft = null
      kickSettingsRead()
    }
  }

  // ---- Draft mutations (dirty only when unlocked) ----

  function settingsLocked() {
    return Core.settingsControlsLocked(settingsState)
  }

  function mutateSettingsDraft(mutator) {
    if (settingsLocked())
      return
    if (!settingsDraft)
      return
    settingsDraft = mutator(settingsDraft)
    settingsState = Core.settingsMarkDirty(settingsState)
    // Keep draft pointer on state for consumers that read settingsState.draft.
    if (settingsState && settingsState.phase !== "closed") {
      var next = Core.cloneState(settingsState)
      next.draft = settingsDraft
      settingsState = next
    }
  }

  function setProviderEnabled(providerId, enabled) {
    mutateSettingsDraft(function (d) {
      return Core.setProviderEnabled(d, providerId, enabled)
    })
  }

  function moveProvider(providerId, delta) {
    mutateSettingsDraft(function (d) {
      return Core.moveProvider(d, providerId, delta)
    })
  }

  function setDisplayMetric(metric) {
    mutateSettingsDraft(function (d) {
      return Core.setDisplayMetric(d, metric)
    })
  }

  function setRefreshInterval(seconds) {
    mutateSettingsDraft(function (d) {
      return Core.setRefreshInterval(d, seconds)
    })
  }

  function setNotificationsEnabled(enabled) {
    mutateSettingsDraft(function (d) {
      return Core.setNotificationsEnabled(d, enabled)
    })
  }

  function restoreSettingsDefaults() {
    if (settingsLocked())
      return
    settingsState = Core.settingsRestoreDefaults(settingsState)
    settingsDraft = settingsState ? settingsState.draft : null
  }

  function cancelSettings() {
    if (settingsLocked() && settingsState && settingsState.phase === "saving")
      return
    settingsState = Core.settingsCancel(settingsState)
    settingsDraft = settingsState ? settingsState.draft : null
  }

  function canSaveSettings() {
    return Core.settingsCanSave(settingsState, settingsDraft)
  }

  function saveSettings() {
    if (maintenanceState.blocked)
      return false
    if (!canSaveSettings())
      return false
    if (!Core.canStartLane(settingsWriteBusy))
      return false
    // SET-018: capture immutable payload before process start.
    var payloadObj = JSON.parse(JSON.stringify(settingsDraft))
    var validation = Core.validateSettingsDraft(payloadObj)
    if (!validation.ok)
      return false
    // SET-016: every save gets a new generation id.
    settingsGeneration++
    var gen = settingsGeneration
    pendingSettingsPayload = JSON.stringify(payloadObj)
    settingsState = Core.settingsBeginSave(settingsState, gen, payloadObj)
    settingsDraft = settingsState.draft
    activeSettingsWriteGeneration = gen
    settingsSaveCount++
    kickSettingsWrite()
    return true
  }

  // JSON-025: map closed action kinds to typed service methods.
  function retryProvider(providerId) {
    refreshProvider(providerId, true)
  }

  function loginProvider(providerId) {
    if (maintenanceState.blocked)
      return
    if (!Core.isClosedProvider(providerId))
      return
    var rootPath = pluginRoot
    if (!rootPath || !rootPath.length)
      return
    var argv = Core.loginDetachedArgv(rootPath, providerId)
    if (!argv)
      return
    lastLoginProviderId = String(providerId)
    lastLoginArgv = argv.slice()
    loginRequestCount++
    if (testMode)
      return
    // Exact argv array — never shell-string construction (UX-048).
    Quickshell.execDetached(argv)
  }

  // ---- Maintenance UI (update / uninstall) ----

  function syncMaintenanceVersion() {
    var ver = helperVersion || manifestVersion || ""
    if (!maintenanceUi || !maintenanceUi.phase || maintenanceUi.phase === "idle") {
      maintenanceUi = Core.maintenanceUiIdle(ver)
      return
    }
    if (ver && (!maintenanceUi.installedVersion || !String(maintenanceUi.installedVersion).length)) {
      var next = Core.cloneMaintenanceUi(maintenanceUi)
      next.installedVersion = ver
      maintenanceUi = next
    }
  }

  function checkForUpdates() {
    if (maintenanceState.blocked)
      return
    if (!Core.canStartLane(maintenanceCheckBusy))
      return
    syncMaintenanceVersion()
    maintenanceUi = Core.maintenanceUiChecking(maintenanceUi)
    maintenanceCheckBusy = true
    if (testMode)
      return
    var helper = resolvedHelperPath()
    if (!helper.length) {
      maintenanceCheckBusy = false
      maintenanceUi = Core.maintenanceUiFromCheck(maintenanceUi, "", 1, helperVersion)
      return
    }
    maintenanceCheckProcess.command = Core.updateCheckArgv(helper)
    maintenanceCheckProcess.running = true
  }

  function applyUpdateCheckResult(stdout, exitCode) {
    maintenanceCheckBusy = false
    maintenanceUi = Core.maintenanceUiFromCheck(
      maintenanceUi,
      stdout,
      exitCode,
      helperVersion || manifestVersion
    )
  }

  function openUpdateConfirm() {
    maintenanceUi = Core.maintenanceUiOpenUpdateConfirm(maintenanceUi)
  }

  function closeUpdateConfirm() {
    maintenanceUi = Core.maintenanceUiCloseUpdateConfirm(maintenanceUi)
  }

  function confirmUpdateApply() {
    if (!maintenanceUi || !maintenanceUi.targetVersion)
      return false
    var intention = Core.maintenanceIntention("update_apply", maintenanceUi)
    if (!intention || !intention.version.length)
      return false
    pendingMaintenanceIntention = intention
    pendingMaintenancePayload = ""
    maintenanceUi = Core.maintenanceUiApplying(maintenanceUi)
    beginMaintenanceHandoff()
    return true
  }

  function openUninstallConfirm() {
    maintenanceUi = Core.maintenanceUiOpenUninstallConfirm(maintenanceUi)
  }

  function closeUninstallConfirm() {
    maintenanceUi = Core.maintenanceUiCloseUninstallConfirm(maintenanceUi)
  }

  function setUninstallPurge(purge) {
    maintenanceUi = Core.maintenanceUiSetPurge(maintenanceUi, purge)
  }

  // UX-047: first click arms, second confirms.
  function armOrConfirmUninstall() {
    var result = Core.maintenanceUiArmOrConfirmUninstall(maintenanceUi)
    maintenanceUi = result.ui
    if (!result.confirmed)
      return false
    var intention = Core.maintenanceIntention("uninstall", maintenanceUi)
    pendingMaintenanceIntention = intention
    pendingMaintenancePayload = JSON.stringify(intention.payload)
    maintenanceUi = Core.maintenanceUiUninstalling(maintenanceUi)
    beginMaintenanceHandoff()
    return true
  }

  function openReleaseNotes() {
    var url = maintenanceUi && maintenanceUi.releaseNotesUrl
        ? String(maintenanceUi.releaseNotesUrl)
        : ""
    if (url.indexOf("https://") !== 0)
      return
    Qt.openUrlExternally(url)
  }

  function viewInstallation(providerId, url) {
    if (!Core.isClosedProvider(providerId))
      return
    var target = String(url || "")
    if (target.indexOf("https://") !== 0)
      return
    lastViewInstallationUrl = target
    Qt.openUrlExternally(target)
  }

  function dispatchAction(providerId, action) {
    if (!action)
      return
    var kind = Core.mapActionKind(action.kind)
    if (!kind)
      return
    if (kind === "retry") {
      retryProvider(providerId)
      return
    }
    if (kind === "login") {
      loginProvider(providerId)
      return
    }
    if (kind === "view_installation") {
      viewInstallation(providerId, action.target)
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

  // Quattro injects `manifest` (and thus pluginRoot) after createObject returns.
  // The completed handler therefore often runs with an empty helper path; do not
  // treat that as a permanent probe failure — retry when the path appears.
  function tryStartProduction() {
    if (testMode)
      return
    if (versionReady || versionProbeRunning)
      return
    if (!resolvedHelperPath().length)
      return
    startVersionProbe()
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
      // Empty path is expected before Quattro property injection; wait for
      // onManifestChanged / onHelperPathChanged rather than locking out.
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
    syncMaintenanceVersion()
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
  // Settings lanes (read / write)
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
    settingsReadProcess.command = Core.settingsArgvShow(helper)
    settingsReadProcess.running = true
  }

  function applySettingsReadResult(generation, stdout, exitCode) {
    if (!Core.shouldApplyGeneration(activeSettingsReadGeneration, generation))
      return
    settingsReadBusy = false
    if (exitCode !== 0 || !settingsState || settingsState.phase === "closed")
      return
    try {
      var doc = JSON.parse(String(stdout || "").trim())
      if (!Core.validateSettingsDraft(doc).ok)
        return
      settingsState = Core.settingsFinishLoad(settingsState, generation, doc)
      settingsDraft = settingsState.draft
      appliedSettings = doc
    } catch (e) {
      // keep loading/locked; user can close and reopen
    }
  }

  function kickSettingsWrite() {
    if (!Core.canStartLane(settingsWriteBusy))
      return
    if (maintenanceState.blocked)
      return
    var helper = resolvedHelperPath()
    if (!helper.length)
      return
    settingsWriteBusy = true
    if (testMode)
      return
    settingsWriteProcess.command = Core.settingsArgvApplyStdin(helper)
    settingsWriteProcess.running = true
  }

  function applySettingsWriteResult(generation, ok, canonical) {
    if (!Core.shouldApplyGeneration(activeSettingsWriteGeneration, generation))
      return
    settingsWriteBusy = false
    pendingSettingsPayload = ""
    settingsState = Core.settingsFinishSave(settingsState, generation, ok, canonical)
    settingsDraft = settingsState ? settingsState.draft : null
    if (ok && canonical)
      appliedSettings = canonical
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
    var helper = resolvedHelperPath()
    var intention = pendingMaintenanceIntention
    var argv = null
    if (intention && intention.kind === "update_apply")
      argv = Core.updateApplyArgv(helper, intention.version)
    else if (intention && intention.kind === "uninstall")
      argv = Core.uninstallArgv(helper, intention.purge)
    else
      argv = helper && helper.length ? [helper, "doctor", "scan"] : null

    if (testMode)
      return
    if (!argv) {
      maintenanceHandoffBusy = false
      return
    }
    // Uninstall confirmation document is written on stdin (BUNDLE-036).
    if (intention && intention.kind === "uninstall" && pendingMaintenancePayload.length) {
      maintenanceHandoffProcess.stdinEnabled = true
    } else {
      maintenanceHandoffProcess.stdinEnabled = false
    }
    maintenanceHandoffProcess.command = argv
    maintenanceHandoffProcess.running = true
  }

  function applyMaintenanceHandoffDone(exitCode) {
    maintenanceHandoffBusy = false
    var intention = pendingMaintenanceIntention
    pendingMaintenanceIntention = null
    pendingMaintenancePayload = ""
    maintenanceState = Core.maintenanceIdle()
    pollEnabled = true
    if (versionReady)
      pollTimer.restart()
    if (intention && intention.kind === "update_apply") {
      if (exitCode === 0) {
        maintenanceUi = Core.maintenanceUiIdle(helperVersion || intention.version)
        maintenanceUi.message = "Update applied."
      } else {
        maintenanceUi = Core.cloneMaintenanceUi(maintenanceUi)
        maintenanceUi.phase = "error"
        maintenanceUi.message = "Update failed."
      }
    } else if (intention && intention.kind === "uninstall") {
      if (exitCode === 0) {
        maintenanceUi = Core.maintenanceUiIdle(helperVersion)
        maintenanceUi.message = "Uninstall completed."
      } else {
        maintenanceUi = Core.cloneMaintenanceUi(maintenanceUi)
        maintenanceUi.phase = "error"
        maintenanceUi.message = "Uninstall failed."
      }
    }
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
      root.applySettingsReadResult(
        root.activeSettingsReadGeneration,
        settingsReadOut.text || "",
        exitCode
      )
    }
  }

  Process {
    id: settingsWriteProcess
    stdinEnabled: true
    stdout: StdioCollector { id: settingsWriteOut; waitForEnd: true }
    stderr: StdioCollector { id: settingsWriteErr; waitForEnd: true }
    onStarted: {
      // Write the immutable captured payload; never re-read live draft (SET-018).
      if (root.pendingSettingsPayload && root.pendingSettingsPayload.length)
        write(root.pendingSettingsPayload + "\n")
    }
    onExited: function (exitCode) {
      var ok = exitCode === 0
      var canonical = null
      if (ok) {
        try {
          canonical = JSON.parse(String(settingsWriteOut.text || "").trim())
          if (!Core.validateSettingsDraft(canonical).ok)
            ok = false
        } catch (e) {
          ok = false
        }
      }
      root.applySettingsWriteResult(root.activeSettingsWriteGeneration, ok, canonical)
    }
  }

  Process {
    id: maintenanceCheckProcess
    stdout: StdioCollector { id: maintenanceCheckOut; waitForEnd: true }
    stderr: StdioCollector { id: maintenanceCheckErr; waitForEnd: true }
    onExited: function (exitCode) {
      root.applyUpdateCheckResult(maintenanceCheckOut.text || "", exitCode)
    }
  }

  Process {
    id: maintenanceHandoffProcess
    stdinEnabled: false
    stdout: StdioCollector { id: maintenanceHandoffOut; waitForEnd: true }
    stderr: StdioCollector { id: maintenanceHandoffErr; waitForEnd: true }
    onStarted: {
      // BUNDLE-036 / UX-048: non-TTY uninstall confirmation uses read_to_end.
      // write() alone does not deliver EOF; stdinEnabled=false closes the write channel.
      if (root.pendingMaintenancePayload && root.pendingMaintenancePayload.length
          && maintenanceHandoffProcess.stdinEnabled) {
        write(root.pendingMaintenancePayload + "\n")
        maintenanceHandoffProcess.stdinEnabled = false
      }
    }
    onExited: function (exitCode) {
      root.applyMaintenanceHandoffDone(exitCode)
    }
  }

  // Single completed handler — QML rejects duplicate assignments of this
  // handler (live Quattro: "Property value set multiple times" at service load).

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

  onHelperPathChanged: root.tryStartProduction()

  onManifestChanged: {
    root.syncMaintenanceVersion()
    root.tryStartProduction()
  }

  Component.onCompleted: {
    root.syncMaintenanceVersion()
    root.tryStartProduction()
  }
}
