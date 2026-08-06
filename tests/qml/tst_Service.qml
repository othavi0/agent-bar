import QtQuick
import QtTest
import "../../assets/omarchy/CoreService.js" as Core
import "../../assets/omarchy/CoreMaintenance.js" as Maintenance
import "../../assets/omarchy/CoreSettings.js" as Settings

TestCase {
  id: testCase
  name: "AgentBarService"
  when: windowShown

  property string repoRoot: {
    var u = Qt.resolvedUrl(".")
    var path = String(u).replace("file://", "")
    if (path.endsWith("/"))
      path = path.slice(0, -1)
    var parts = path.split("/")
    parts.pop(); parts.pop()
    return parts.join("/")
  }
  property string serviceUrl: "file://" + repoRoot + "/assets/omarchy/Service.qml"
  property string fakeHelper: repoRoot + "/tests/qml/fixtures/fake-agent-bar"
  property string manifestPath: repoRoot + "/assets/omarchy/manifest.json"

  // Harness mirrors Service.qml state machine without Quickshell.Io Process.
  Item {
    id: h
    property string helperVersion: ""
    property bool versionReady: false
    property bool versionFailed: false
    property bool collectionStarted: false
    property int collectionDelayMs: 0
    property var snapshot: null
    property bool refreshing: false
    property string selectedProviderId: ""
    property var popupOwner: null
    property var settingsState: Settings.settingsClosed()
    property var settingsDraft: null
    property var maintenanceState: Maintenance.maintenanceIdle()
    property var pendingForcedTargets: Core.emptyPending()
    property bool statusBusy: false
    property bool settingsReadBusy: false
    property bool settingsWriteBusy: false
    property int statusGeneration: 0
    property int activeStatusGeneration: 0
    property int statusStartCount: 0
    property int refreshRequestCount: 0
    property string lastRefreshProviderId: ""
    property bool pollEnabled: true
    property var manifest: ({ version: "10.0.0" })
    readonly property string manifestVersion: "10.0.0"

    function health(v) {
      return Core.health(versionReady, versionFailed, helperVersion, manifestVersion, v)
    }
    function applyVersion(stdout) {
      var v = Core.parseVersionStdout(stdout, "", 0)
      if (!v) { versionFailed = true; return }
      helperVersion = v; versionReady = true; versionFailed = false
      if (collectionDelayMs > 0) delay.restart()
      else beginCollection()
    }
    function beginCollection() {
      collectionStarted = true
      kickStatus()
    }
    function refreshAll(force) {
      if (maintenanceState.blocked) return
      if (force) pendingForcedTargets = Core.unionForced(pendingForcedTargets, "all")
      kickStatus()
    }
    function refreshProvider(id, force) {
      if (maintenanceState.blocked) return
      if (!Core.isClosedProvider(id)) return
      if (force) pendingForcedTargets = Core.unionForced(pendingForcedTargets, id)
      kickStatus()
    }
    function refresh(id) {
      if (Core.refreshResult(id) !== "ok") return "unknown"
      lastRefreshProviderId = String(id)
      refreshRequestCount++
      refreshProvider(id, true)
      return "ok"
    }
    function kickStatus() {
      if (!versionReady || versionFailed || maintenanceState.blocked) return
      if (!Core.canStartLane(statusBusy)) return
      statusGeneration++
      activeStatusGeneration = statusGeneration
      var taken = Core.takePending(pendingForcedTargets)
      pendingForcedTargets = taken.remaining
      statusBusy = true
      refreshing = true
      statusStartCount++
      // argv available for assertions
      lastArgv = Core.statusArgv("/helper", taken.captured)
    }
    property var lastArgv: []
    function applyStatus(gen, stdout, code) {
      if (!Core.shouldApplyGeneration(activeStatusGeneration, gen)) return
      statusBusy = false
      refreshing = false
      if (code !== 0) { maybeFollowUp(); return }
      var parsed = Core.parseStatusEnvelope(stdout, helperVersion)
      if (!parsed.ok) { maybeFollowUp(); return }
      snapshot = parsed.envelope
      maybeFollowUp()
    }
    function maybeFollowUp() {
      if (!Core.pendingIsEmpty(pendingForcedTargets))
        kickStatus()
    }
    function requestPopup(owner, providerId, view) {
      popupOwner = Core.requestPopup(popupOwner, owner, providerId, view)
      if (providerId) selectedProviderId = String(providerId)
    }
    function closePopup(owner) {
      popupOwner = Core.closePopup(popupOwner, owner)
    }
    function dismissPopup() {
      popupOwner = Core.dismissPopup(popupOwner)
    }
    function openSettings(owner) {
      if (maintenanceState.blocked) return
      requestPopup(owner, selectedProviderId || null, "settings")
      if (!settingsState || settingsState.phase === "closed") {
        settingsGen++
        settingsState = Settings.settingsBeginLoad(settingsGen)
        settingsDraft = null
        // Harness completes load immediately with defaults (production uses config show).
        settingsState = Settings.settingsFinishLoad(settingsState, settingsGen, Core.defaultSettings())
        settingsDraft = settingsState.draft
      }
    }
    property int settingsGen: 0
    function beginMaintenance() {
      maintenanceState = Maintenance.maintenanceBeginHandoff(maintenanceState)
      pollEnabled = false
    }
    function tryDetach() {
      return Maintenance.maintenanceCanDetach(maintenanceState, statusBusy, settingsWriteBusy)
    }
    Timer {
      id: delay
      interval: h.collectionDelayMs
      onTriggered: h.beginCollection()
    }
  }

  function reset() {
    h.helperVersion = ""
    h.versionReady = false
    h.versionFailed = false
    h.collectionStarted = false
    h.collectionDelayMs = 0
    h.snapshot = null
    h.refreshing = false
    h.selectedProviderId = ""
    h.popupOwner = null
    h.settingsState = Settings.settingsClosed()
    h.settingsDraft = null
    h.maintenanceState = Maintenance.maintenanceIdle()
    h.pendingForcedTargets = Core.emptyPending()
    h.statusBusy = false
    h.settingsWriteBusy = false
    h.statusGeneration = 0
    h.activeStatusGeneration = 0
    h.statusStartCount = 0
    h.refreshRequestCount = 0
    h.lastArgv = []
    h.pollEnabled = true
  }

  function validEnvelope(version) {
    return JSON.stringify({
      schemaVersion: 2,
      helperVersion: version || "10.0.0",
      generatedAt: "2026-07-26T18:42:00Z",
      request: { provider: null, cache: "use" },
      providers: [{
        id: "claude",
        name: "Claude",
        state: "ready",
        source: "live",
        plan: null,
        account: null,
        windows: [{
          id: "session",
          label: "Session",
          usedPercent: 10,
          remainingPercent: 90,
          resetsAt: null
        }],
        lastSuccessAt: "2026-07-26T18:42:00Z",
        error: null,
        action: null
      }]
    })
  }

  function loadManifest() {
    var xhr = new XMLHttpRequest()
    xhr.open("GET", "file://" + manifestPath, false)
    xhr.send()
    compare(xhr.status === 200 || xhr.status === 0, true)
    return JSON.parse(xhr.responseText)
  }

  // ---- Task 8 carry-over ----

  function test_manifest_shape() {
    var m = loadManifest()
    compare(m.id, "othavi0.agent-bar")
    verify(m.kinds.indexOf("service") >= 0)
    verify(m.kinds.indexOf("bar-widget") >= 0)
    compare(m.barWidget.schema.length, 0)
    verify(!("activation" in m))
  }

  function test_version_and_health() {
    reset()
    h.applyVersion("10.0.0\n")
    compare(h.health("10.0.0"), "ok")
    compare(h.health("9.0.0"), "unknown")
    compare(h.collectionStarted, true)
  }

  function test_refresh_closed_providers() {
    reset()
    h.applyVersion("10.0.0\n")
    h.statusBusy = false // clear auto kick
    h.statusStartCount = 0
    compare(h.refresh("claude"), "ok")
    compare(h.refresh("nope"), "unknown")
    compare(h.refreshRequestCount, 1)
  }

  // ---- Task 9 ----

  function test_status_argv_shape_cache_use() {
    reset()
    h.applyVersion("10.0.0\n")
    // First kick uses empty pending → cache use
    verify(h.lastArgv.indexOf("status") >= 0)
    verify(h.lastArgv.indexOf("format") >= 0)
    verify(h.lastArgv.indexOf("json") >= 0)
    verify(h.lastArgv.indexOf("cache") >= 0)
    verify(h.lastArgv.indexOf("use") >= 0 || h.lastArgv.indexOf("bypass") >= 0)
    verify(h.lastArgv.indexOf("notifications") >= 0)
    verify(h.lastArgv.indexOf("evaluate") >= 0)
  }

  function test_force_refresh_uses_bypass() {
    reset()
    h.applyVersion("10.0.0\n")
    // complete in-flight
    h.applyStatus(h.activeStatusGeneration, validEnvelope("10.0.0"), 0)
    h.statusStartCount = 0
    h.refreshAll(true)
    compare(h.statusStartCount, 1)
    verify(h.lastArgv.indexOf("bypass") >= 0)
  }

  function test_immutable_snapshot_replacement() {
    reset()
    h.applyVersion("10.0.0\n")
    var gen = h.activeStatusGeneration
    h.applyStatus(gen, validEnvelope("10.0.0"), 0)
    verify(h.snapshot !== null)
    compare(h.snapshot.schemaVersion, 2)
    compare(h.snapshot.providers[0].id, "claude")
    var first = h.snapshot
    h.statusBusy = false
    h.kickStatus()
    h.applyStatus(h.activeStatusGeneration, validEnvelope("10.0.0"), 0)
    verify(h.snapshot !== first)
    compare(h.snapshot.providers[0].id, "claude")
  }

  function test_malformed_envelope_retains_snapshot() {
    reset()
    h.applyVersion("10.0.0\n")
    h.applyStatus(h.activeStatusGeneration, validEnvelope("10.0.0"), 0)
    var kept = h.snapshot
    h.statusBusy = false
    h.kickStatus()
    h.applyStatus(h.activeStatusGeneration, "{not-json", 0)
    compare(h.snapshot, kept)
  }

  function test_stale_generation_ignored() {
    reset()
    h.applyVersion("10.0.0\n")
    var oldGen = h.activeStatusGeneration
    h.statusBusy = false
    h.kickStatus()
    var newGen = h.activeStatusGeneration
    verify(newGen !== oldGen)
    // Late callback from old generation must not clobber.
    h.applyStatus(oldGen, validEnvelope("10.0.0"), 0)
    // Still refreshing / busy from new gen until applied
    h.applyStatus(newGen, validEnvelope("10.0.0"), 0)
    compare(h.snapshot.schemaVersion, 2)
  }

  function test_one_status_lane_no_reentry() {
    reset()
    h.applyVersion("10.0.0\n")
    var starts = h.statusStartCount
    // While busy, kick must not start another.
    h.kickStatus()
    compare(h.statusStartCount, starts)
  }

  function test_pending_forced_union_all_dominates() {
    var p = Core.emptyPending()
    p = Core.unionForced(p, "claude")
    p = Core.unionForced(p, "amp")
    p = Core.unionForced(p, "all")
    compare(p.all, true)
    p = Core.unionForced(p, "grok")
    compare(p.all, true)
  }

  function test_status_argv_single_provider_force() {
    var argv = Core.statusArgv("/h", { all: false, ids: { "claude": true } })
    verify(argv.indexOf("provider") >= 0)
    verify(argv.indexOf("claude") >= 0)
    verify(argv.indexOf("bypass") >= 0)
  }

  function test_popup_same_owner_close() {
    reset()
    h.requestPopup("mon-a", "claude", "usage")
    compare(h.popupOwner.owner, "mon-a")
    compare(h.selectedProviderId, "claude")
    h.closePopup("mon-b")
    verify(h.popupOwner !== null)
    h.closePopup("mon-a")
    compare(h.popupOwner, null)
  }

  function test_popup_dismiss_clears_any_owner() {
    reset()
    h.requestPopup("mon-a", "claude", "usage")
    verify(h.popupOwner !== null)
    // Foreign monitor cannot same-owner close, but dismiss always clears.
    h.closePopup("mon-b")
    verify(h.popupOwner !== null)
    h.dismissPopup()
    compare(h.popupOwner, null)
    verify(Core.foreignPopupOpen({ owner: "mon-a" }, "mon-b"))
    verify(!Core.foreignPopupOpen({ owner: "mon-a" }, "mon-a"))
    verify(!Core.foreignPopupOpen(null, "mon-b"))
  }

  function test_popup_cross_monitor_transfer() {
    reset()
    h.requestPopup("mon-a", "claude", "usage")
    h.requestPopup("mon-b", "grok", "usage")
    compare(h.popupOwner.owner, "mon-b")
    compare(h.popupOwner.providerId, "grok")
    compare(h.selectedProviderId, "grok")
  }

  function test_settings_open_captures_snapshot() {
    reset()
    h.applyVersion("10.0.0\n")
    h.applyStatus(h.activeStatusGeneration, validEnvelope("10.0.0"), 0)
    h.openSettings("mon-a")
    compare(h.settingsState.phase, "clean")
    verify(h.settingsState.snapshot !== null)
    verify(h.settingsDraft !== null)
    compare(h.popupOwner.view, "settings")
  }

  function test_maintenance_blocks_poll_and_waits_drain() {
    reset()
    h.applyVersion("10.0.0\n")
    h.statusBusy = true
    h.beginMaintenance()
    compare(h.maintenanceState.blocked, true)
    compare(h.pollEnabled, false)
    compare(h.tryDetach(), false)
    h.statusBusy = false
    compare(h.tryDetach(), true)
  }

  function test_canStartLane() {
    compare(Core.canStartLane(false), true)
    compare(Core.canStartLane(true), false)
  }

  function test_service_qml_declares_six_process_lanes() {
    var xhr = new XMLHttpRequest()
    xhr.open("GET", serviceUrl, false)
    xhr.send()
    var src = String(xhr.responseText)
    verify(src.indexOf("id: versionProbe") >= 0)
    verify(src.indexOf("id: statusProcess") >= 0)
    verify(src.indexOf("id: settingsReadProcess") >= 0)
    verify(src.indexOf("id: settingsWriteProcess") >= 0)
    verify(src.indexOf("id: maintenanceCheckProcess") >= 0)
    verify(src.indexOf("id: maintenanceHandoffProcess") >= 0)
    verify(src.indexOf("id: pollTimer") >= 0)
    verify(src.indexOf('target: "othavi0.agent-bar"') >= 0)
  }

  // Live Quattro: duplicate Component.onCompleted → "Property value set multiple times"
  // and the service never loads (bar chips disappear).
  function test_service_qml_has_single_component_on_completed() {
    var xhr = new XMLHttpRequest()
    xhr.open("GET", serviceUrl, false)
    xhr.send()
    var src = String(xhr.responseText)
    var re = /Component\.onCompleted/g
    var count = 0
    while (re.exec(src) !== null)
      count++
    compare(count, 1)
    verify(src.indexOf("startVersionProbe") >= 0)
    verify(src.indexOf("syncMaintenanceVersion") >= 0)
  }

  // Quickshell StdioCollector.text is read-only; assigning throws TypeError and
  // aborts the version probe before Process.running is set (live bar stuck loading).
  function test_service_qml_does_not_assign_stdio_collector_text() {
    var xhr = new XMLHttpRequest()
    xhr.open("GET", serviceUrl, false)
    xhr.send()
    var src = String(xhr.responseText)
    verify(src.indexOf("versionOut.text =") < 0)
    verify(src.indexOf("versionErr.text =") < 0)
    verify(src.indexOf("statusOut.text =") < 0)
    verify(src.indexOf("statusErr.text =") < 0)
  }

  // Live Quattro createObject sets manifest after construction completes.
  // Service must retry production start when helper path appears, and must not
  // mark versionFailed merely because the path was empty on first attempt.
  function test_service_qml_defers_probe_until_helper_path() {
    var xhr = new XMLHttpRequest()
    xhr.open("GET", serviceUrl, false)
    xhr.send()
    var src = String(xhr.responseText)
    verify(src.indexOf("function tryStartProduction") >= 0)
    verify(src.indexOf("onManifestChanged") >= 0)
    verify(src.indexOf("onHelperPathChanged") >= 0)
    verify(src.indexOf("tryStartProduction()") >= 0)
    verify(src.indexOf("manifest.__sourceDir") >= 0)
    // Empty helper path must wait, not finishVersionProbeFailure.
    var emptyBranch = src.indexOf("if (!helper.length)")
    verify(emptyBranch >= 0)
    var nextFail = src.indexOf("finishVersionProbeFailure", emptyBranch)
    var nextReturn = src.indexOf("return", emptyBranch)
    verify(nextReturn >= 0)
    // The immediate empty-path branch must return without permanent failure.
    if (nextFail >= 0)
      verify(nextReturn < nextFail)
  }
}
